use crate::entities;
use crate::events::Event;
use anyhow::Result;
use sea_orm::{Database, DatabaseConnection, EntityTrait, TransactionTrait};
use std::time::Instant;
use tracing::{debug, error, info};

// Result structs for query responses
#[derive(Debug, Clone)]
pub struct AgentTransactionEventResult {
    pub id: i64,
    pub coordinator_id: String,
    pub tx_type: String,
    pub developer: String,
    pub agent: String,
    pub app: String,
    pub job_id: String,
    pub sequences: Vec<u64>,
    pub event_timestamp: u64,
    pub tx_hash: String,
    pub chain: String,
    pub network: String,
    pub memo: String,
    pub metadata: String,
    pub created_at_timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct AgentMessageEventResult {
    pub id: i64,
    pub coordinator_id: String,
    pub developer: String,
    pub agent: String,
    pub app: String,
    pub job_id: String,
    pub sequences: Vec<u64>,
    pub event_timestamp: u64,
    pub level: u32,
    pub message: String,
    pub created_at_timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct CoordinatorMessageEventResult {
    pub id: i64,
    pub coordinator_id: String,
    pub event_timestamp: u64,
    pub level: i32,
    pub message: String,
    pub created_at_timestamp: i64,
    pub relevance_score: f64,
}

pub struct EventDatabase {
    connection: DatabaseConnection,
}

impl EventDatabase {
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to TiDB at: {}", database_url);

        let connection = Database::connect(database_url).await?;

        info!("Successfully connected to TiDB");

        Ok(Self { connection })
    }

    pub async fn insert_events_batch(&self, events: &[Event]) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        let start_time = Instant::now();

        debug!(
            "Inserting batch of {} events using parallel batch insertion with child table support",
            events.len()
        );

        let txn = self.connection.begin().await?;
        let mut total_inserted = 0;

        // Group events by type for batch insertion
        let mut coordinator_started_events = Vec::new();
        let mut agent_started_job_events = Vec::new();
        let mut agent_finished_job_events = Vec::new();
        let mut coordination_tx_events = Vec::new();
        let mut coordinator_message_events = Vec::new();
        let mut client_transaction_events = Vec::new();
        let mut agent_message_events = Vec::new();
        let mut agent_transaction_events = Vec::new();

        // Store sequences separately for child table insertion
        let mut agent_message_sequences = Vec::new();
        let mut agent_transaction_sequences = Vec::new();

        // Categorize events by type
        for event in events {
            if let Some(event_type) = &event.event_type {
                match event_type {
                    crate::events::event::EventType::Coordinator(coordinator_event) => {
                        if let Some(coord_event) = &coordinator_event.event {
                            use crate::events::coordinator_event::Event as CoordEvent;
                            match coord_event {
                                CoordEvent::CoordinatorStarted(event) => {
                                    coordinator_started_events
                                        .push(convert_coordinator_started_event(event));
                                }
                                CoordEvent::AgentStartedJob(event) => {
                                    agent_started_job_events
                                        .push(convert_agent_started_job_event(event));
                                }
                                CoordEvent::AgentFinishedJob(event) => {
                                    agent_finished_job_events
                                        .push(convert_agent_finished_job_event(event));
                                }
                                CoordEvent::CoordinationTx(event) => {
                                    coordination_tx_events
                                        .push(convert_coordination_tx_event(event));
                                }
                                CoordEvent::CoordinatorError(event) => {
                                    coordinator_message_events
                                        .push(convert_coordinator_message_event(event));
                                }
                                CoordEvent::ClientTransaction(event) => {
                                    client_transaction_events
                                        .push(convert_client_transaction_event(event));
                                }
                            }
                        }
                    }
                    crate::events::event::EventType::Agent(agent_event) => {
                        if let Some(agent_event_type) = &agent_event.event {
                            use crate::events::agent_event::Event as AgentEventType;
                            match agent_event_type {
                                AgentEventType::Message(event) => {
                                    let (main_event, sequences) =
                                        convert_agent_message_event(event);
                                    agent_message_events.push(main_event);
                                    agent_message_sequences.push(sequences);
                                }
                                AgentEventType::Transaction(event) => {
                                    let (main_event, sequences) =
                                        convert_agent_transaction_event(event);
                                    agent_transaction_events.push(main_event);
                                    agent_transaction_sequences.push(sequences);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Phase 1: Run all independent main table insertions in parallel
        info!("Phase 1: Running main table insertions in parallel");
        let independent_results = tokio::try_join!(
            self.insert_coordinator_started_events(&txn, coordinator_started_events),
            self.insert_agent_started_job_events(&txn, agent_started_job_events),
            self.insert_agent_finished_job_events(&txn, agent_finished_job_events),
            self.insert_coordination_tx_events(&txn, coordination_tx_events),
            self.insert_coordinator_message_events(&txn, coordinator_message_events),
            self.insert_client_transaction_events(&txn, client_transaction_events),
        )?;

        // Sum up results from independent insertions
        total_inserted += independent_results.0
            + independent_results.1
            + independent_results.2
            + independent_results.3
            + independent_results.4
            + independent_results.5;

        // Phase 2: Handle parent-child relationships for events with sequences
        info!("Phase 2: Running parent-child table insertions");
        let parent_child_results = tokio::try_join!(
            self.insert_agent_message_events_with_sequences(
                &txn,
                agent_message_events,
                &agent_message_sequences
            ),
            self.insert_agent_transaction_events_with_sequences(
                &txn,
                agent_transaction_events,
                &agent_transaction_sequences
            ),
        )?;

        total_inserted += parent_child_results.0 + parent_child_results.1;

        txn.commit().await?;

        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis();
        let events_per_second = events.len() as f64 / duration.as_secs_f64();

        info!(
            "Successfully parallel batch inserted {} records for {} events in {}ms ({:.2}s) - {:.0} events/second",
            total_inserted,
            events.len(),
            duration_ms,
            duration.as_secs_f64(),
            events_per_second
        );

        Ok(events.len())
    }

    // Independent table insertion methods - can run in parallel
    async fn insert_coordinator_started_events(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        events: Vec<entities::coordinator_started_event::ActiveModel>,
    ) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!(
            "Parallel inserting {} coordinator_started_events",
            events.len()
        );
        match entities::coordinator_started_event::Entity::insert_many(events)
            .exec(txn)
            .await
        {
            Ok(result) => {
                debug!("Successfully inserted coordinator_started_events");
                Ok(result.last_insert_id as usize)
            }
            Err(e) => {
                error!("Failed to batch insert coordinator_started_events: {}", e);
                Err(e.into())
            }
        }
    }

    async fn insert_agent_started_job_events(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        events: Vec<entities::agent_started_job_event::ActiveModel>,
    ) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!(
            "Parallel inserting {} agent_started_job_events",
            events.len()
        );
        match entities::agent_started_job_event::Entity::insert_many(events)
            .exec(txn)
            .await
        {
            Ok(result) => {
                debug!("Successfully inserted agent_started_job_events");
                Ok(result.last_insert_id as usize)
            }
            Err(e) => {
                error!("Failed to batch insert agent_started_job_events: {}", e);
                Err(e.into())
            }
        }
    }

    async fn insert_agent_finished_job_events(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        events: Vec<entities::agent_finished_job_event::ActiveModel>,
    ) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!(
            "Parallel inserting {} agent_finished_job_events",
            events.len()
        );
        match entities::agent_finished_job_event::Entity::insert_many(events)
            .exec(txn)
            .await
        {
            Ok(result) => {
                debug!("Successfully inserted agent_finished_job_events");
                Ok(result.last_insert_id as usize)
            }
            Err(e) => {
                error!("Failed to batch insert agent_finished_job_events: {}", e);
                Err(e.into())
            }
        }
    }

    async fn insert_coordination_tx_events(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        events: Vec<entities::coordination_tx_event::ActiveModel>,
    ) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!("Parallel inserting {} coordination_tx_events", events.len());
        match entities::coordination_tx_event::Entity::insert_many(events)
            .exec(txn)
            .await
        {
            Ok(result) => {
                debug!("Successfully inserted coordination_tx_events");
                Ok(result.last_insert_id as usize)
            }
            Err(e) => {
                error!("Failed to batch insert coordination_tx_events: {}", e);
                Err(e.into())
            }
        }
    }

    async fn insert_coordinator_message_events(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        events: Vec<entities::coordinator_message_event::ActiveModel>,
    ) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!(
            "Parallel inserting {} coordinator_message_events",
            events.len()
        );
        match entities::coordinator_message_event::Entity::insert_many(events)
            .exec(txn)
            .await
        {
            Ok(result) => {
                debug!("Successfully inserted coordinator_message_events");
                Ok(result.last_insert_id as usize)
            }
            Err(e) => {
                error!("Failed to batch insert coordinator_message_events: {}", e);
                Err(e.into())
            }
        }
    }

    async fn insert_client_transaction_events(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        events: Vec<entities::client_transaction_event::ActiveModel>,
    ) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!(
            "Parallel inserting {} client_transaction_events",
            events.len()
        );
        match entities::client_transaction_event::Entity::insert_many(events)
            .exec(txn)
            .await
        {
            Ok(result) => {
                debug!("Successfully inserted client_transaction_events");
                Ok(result.last_insert_id as usize)
            }
            Err(e) => {
                error!("Failed to batch insert client_transaction_events: {}", e);
                Err(e.into())
            }
        }
    }

    // Parent-child table insertion methods - handle sequences
    async fn insert_agent_message_events_with_sequences(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        events: Vec<entities::agent_message_event::ActiveModel>,
        sequences_per_event: &[Vec<u64>],
    ) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!(
            "Parallel inserting {} agent_message_events with sequences",
            events.len()
        );

        // Insert parent records first
        let parent_result = entities::agent_message_event::Entity::insert_many(events)
            .exec(txn)
            .await?;

        let base_id = parent_result.last_insert_id;
        let sequence_records =
            self.create_agent_message_sequence_records(base_id, sequences_per_event);

        // Insert child records if any
        if !sequence_records.is_empty() {
            debug!(
                "Inserting {} agent_message_event_sequences records",
                sequence_records.len()
            );
            entities::agent_message_event_sequences::Entity::insert_many(sequence_records)
                .exec(txn)
                .await?;
            debug!("Successfully inserted agent message sequences");
        }

        Ok(parent_result.last_insert_id as usize)
    }

    async fn insert_agent_transaction_events_with_sequences(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        events: Vec<entities::agent_transaction_event::ActiveModel>,
        sequences_per_event: &[Vec<u64>],
    ) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!(
            "Parallel inserting {} agent_transaction_events with sequences",
            events.len()
        );

        // Insert parent records first
        let parent_result = entities::agent_transaction_event::Entity::insert_many(events)
            .exec(txn)
            .await?;

        let base_id = parent_result.last_insert_id;
        let sequence_records =
            self.create_agent_transaction_sequence_records(base_id, sequences_per_event);

        // Insert child records if any
        if !sequence_records.is_empty() {
            debug!(
                "Inserting {} agent_transaction_event_sequences records",
                sequence_records.len()
            );
            entities::agent_transaction_event_sequences::Entity::insert_many(sequence_records)
                .exec(txn)
                .await?;
            debug!("Successfully inserted agent transaction sequences");
        }

        Ok(parent_result.last_insert_id as usize)
    }

    fn create_agent_message_sequence_records(
        &self,
        base_id: i64,
        sequences_per_event: &[Vec<u64>],
    ) -> Vec<entities::agent_message_event_sequences::ActiveModel> {
        use entities::agent_message_event_sequences::*;
        use sea_orm::ActiveValue;

        let mut records = Vec::new();
        let mut current_id = base_id;

        for sequences in sequences_per_event {
            for &sequence in sequences {
                records.push(ActiveModel {
                    id: ActiveValue::NotSet,
                    agent_message_event_id: ActiveValue::Set(current_id),
                    sequence: ActiveValue::Set(sequence as i64),
                    created_at: ActiveValue::NotSet,
                    updated_at: ActiveValue::NotSet,
                });
            }
            current_id += 1; // Move to next parent event ID
        }

        records
    }

    fn create_agent_transaction_sequence_records(
        &self,
        base_id: i64,
        sequences_per_event: &[Vec<u64>],
    ) -> Vec<entities::agent_transaction_event_sequences::ActiveModel> {
        use entities::agent_transaction_event_sequences::*;
        use sea_orm::ActiveValue;

        let mut records = Vec::new();
        let mut current_id = base_id;

        for sequences in sequences_per_event {
            for &sequence in sequences {
                records.push(ActiveModel {
                    id: ActiveValue::NotSet,
                    agent_transaction_event_id: ActiveValue::Set(current_id),
                    sequence: ActiveValue::Set(sequence as i64),
                    created_at: ActiveValue::NotSet,
                    updated_at: ActiveValue::NotSet,
                });
            }
            current_id += 1; // Move to next parent event ID
        }

        records
    }

    #[allow(dead_code)]
    pub async fn get_connection(&self) -> &DatabaseConnection {
        &self.connection
    }

    // Query methods for retrieving events by sequence
    pub async fn get_agent_transaction_events_by_sequence(
        &self,
        sequence: u64,
        limit: Option<u32>,
        offset: Option<u32>,
        coordinator_id: Option<String>,
        developer: Option<String>,
        agent: Option<String>,
        app: Option<String>,
    ) -> Result<(Vec<AgentTransactionEventResult>, u32)> {
        use sea_orm::{ConnectionTrait, Statement};

        // Build the WHERE clause for optional filters
        let mut where_conditions = vec!["seqs.sequence = ?".to_string()];
        let mut params: Vec<sea_orm::Value> = vec![(sequence as i64).into()];

        if let Some(coord_id) = &coordinator_id {
            where_conditions.push("e.coordinator_id = ?".to_string());
            params.push(coord_id.clone().into());
        }
        if let Some(dev) = &developer {
            where_conditions.push("e.developer = ?".to_string());
            params.push(dev.clone().into());
        }
        if let Some(agent_filter) = &agent {
            where_conditions.push("e.agent = ?".to_string());
            params.push(agent_filter.clone().into());
        }
        if let Some(app_filter) = &app {
            where_conditions.push("e.app = ?".to_string());
            params.push(app_filter.clone().into());
        }

        let where_clause = where_conditions.join(" AND ");

        // First, get the count for pagination
        let count_query = format!(
            "SELECT COUNT(DISTINCT e.id) as count 
             FROM agent_transaction_event e 
             INNER JOIN agent_transaction_event_sequences seqs ON e.id = seqs.agent_transaction_event_id 
             WHERE {}",
            where_clause
        );

        let count_stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::MySql,
            &count_query,
            params.clone(),
        );

        let count_result = self.connection.query_one(count_stmt).await?;
        let total_count: u32 = count_result
            .map(|row| row.try_get::<i64>("", "count").unwrap_or(0) as u32)
            .unwrap_or(0);

        if total_count == 0 {
            return Ok((Vec::new(), 0));
        }

        // Build the main query with all event data and sequences in one go
        let mut main_query = format!(
            "SELECT e.id, e.coordinator_id, e.tx_type, e.developer, e.agent, e.app, e.job_id, 
                    e.event_timestamp, e.tx_hash, e.chain, e.network, e.memo, e.metadata, 
                    e.created_at, GROUP_CONCAT(all_seqs.sequence) as sequences
             FROM agent_transaction_event e 
             INNER JOIN agent_transaction_event_sequences seqs ON e.id = seqs.agent_transaction_event_id
             LEFT JOIN agent_transaction_event_sequences all_seqs ON e.id = all_seqs.agent_transaction_event_id
             WHERE {}
             GROUP BY e.id, e.coordinator_id, e.tx_type, e.developer, e.agent, e.app, e.job_id, 
                      e.event_timestamp, e.tx_hash, e.chain, e.network, e.memo, e.metadata, e.created_at
             ORDER BY e.created_at DESC",
            where_clause
        );

        // Add pagination
        if let Some(limit_val) = limit {
            main_query.push_str(&format!(" LIMIT {}", limit_val));
            if let Some(offset_val) = offset {
                main_query.push_str(&format!(" OFFSET {}", offset_val));
            }
        }

        let main_stmt =
            Statement::from_sql_and_values(sea_orm::DatabaseBackend::MySql, &main_query, params);

        let rows = self.connection.query_all(main_stmt).await?;

        let mut results = Vec::new();
        for row in rows {
            let sequences_str: String = row.try_get("", "sequences").unwrap_or_default();
            let sequences: Vec<u64> = if sequences_str.is_empty() {
                Vec::new()
            } else {
                sequences_str
                    .split(',')
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect()
            };

            let created_at_timestamp: i64 = row
                .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            results.push(AgentTransactionEventResult {
                id: row.try_get("", "id").unwrap_or(0),
                coordinator_id: row.try_get("", "coordinator_id").unwrap_or_default(),
                tx_type: row.try_get("", "tx_type").unwrap_or_default(),
                developer: row.try_get("", "developer").unwrap_or_default(),
                agent: row.try_get("", "agent").unwrap_or_default(),
                app: row.try_get("", "app").unwrap_or_default(),
                job_id: row.try_get("", "job_id").unwrap_or_default(),
                sequences,
                event_timestamp: row.try_get::<i64>("", "event_timestamp").unwrap_or(0) as u64,
                tx_hash: row.try_get("", "tx_hash").unwrap_or_default(),
                chain: row.try_get("", "chain").unwrap_or_default(),
                network: row.try_get("", "network").unwrap_or_default(),
                memo: row.try_get("", "memo").unwrap_or_default(),
                metadata: row.try_get("", "metadata").unwrap_or_default(),
                created_at_timestamp,
            });
        }

        Ok((results, total_count))
    }

    pub async fn get_agent_message_events_by_sequence(
        &self,
        sequence: u64,
        limit: Option<u32>,
        offset: Option<u32>,
        coordinator_id: Option<String>,
        developer: Option<String>,
        agent: Option<String>,
        app: Option<String>,
    ) -> Result<(Vec<AgentMessageEventResult>, u32)> {
        use sea_orm::{ConnectionTrait, Statement};

        // Build the WHERE clause for optional filters
        let mut where_conditions = vec!["seqs.sequence = ?".to_string()];
        let mut params: Vec<sea_orm::Value> = vec![(sequence as i64).into()];

        if let Some(coord_id) = &coordinator_id {
            where_conditions.push("e.coordinator_id = ?".to_string());
            params.push(coord_id.clone().into());
        }
        if let Some(dev) = &developer {
            where_conditions.push("e.developer = ?".to_string());
            params.push(dev.clone().into());
        }
        if let Some(agent_filter) = &agent {
            where_conditions.push("e.agent = ?".to_string());
            params.push(agent_filter.clone().into());
        }
        if let Some(app_filter) = &app {
            where_conditions.push("e.app = ?".to_string());
            params.push(app_filter.clone().into());
        }

        let where_clause = where_conditions.join(" AND ");

        // First, get the count for pagination
        let count_query = format!(
            "SELECT COUNT(DISTINCT e.id) as count 
             FROM agent_message_event e 
             INNER JOIN agent_message_event_sequences seqs ON e.id = seqs.agent_message_event_id 
             WHERE {}",
            where_clause
        );

        let count_stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::MySql,
            &count_query,
            params.clone(),
        );

        let count_result = self.connection.query_one(count_stmt).await?;
        let total_count: u32 = count_result
            .map(|row| row.try_get::<i64>("", "count").unwrap_or(0) as u32)
            .unwrap_or(0);

        if total_count == 0 {
            return Ok((Vec::new(), 0));
        }

        // Build the main query with all event data and sequences in one go
        let mut main_query = format!(
            "SELECT e.id, e.coordinator_id, e.developer, e.agent, e.app, e.job_id, 
                    e.event_timestamp, e.level, e.message, e.created_at, 
                    GROUP_CONCAT(all_seqs.sequence) as sequences
             FROM agent_message_event e 
             INNER JOIN agent_message_event_sequences seqs ON e.id = seqs.agent_message_event_id
             LEFT JOIN agent_message_event_sequences all_seqs ON e.id = all_seqs.agent_message_event_id
             WHERE {}
             GROUP BY e.id, e.coordinator_id, e.developer, e.agent, e.app, e.job_id, 
                      e.event_timestamp, e.level, e.message, e.created_at
             ORDER BY e.created_at DESC",
            where_clause
        );

        // Add pagination
        if let Some(limit_val) = limit {
            main_query.push_str(&format!(" LIMIT {}", limit_val));
            if let Some(offset_val) = offset {
                main_query.push_str(&format!(" OFFSET {}", offset_val));
            }
        }

        let main_stmt =
            Statement::from_sql_and_values(sea_orm::DatabaseBackend::MySql, &main_query, params);

        let rows = self.connection.query_all(main_stmt).await?;

        let mut results = Vec::new();
        for row in rows {
            let sequences_str: String = row.try_get("", "sequences").unwrap_or_default();
            let sequences: Vec<u64> = if sequences_str.is_empty() {
                Vec::new()
            } else {
                sequences_str
                    .split(',')
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect()
            };

            let created_at_timestamp: i64 = row
                .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            // Parse level as integer (TINYINT in database)
            let level: u32 = row.try_get::<i32>("", "level").unwrap_or(0) as u32;

            results.push(AgentMessageEventResult {
                id: row.try_get("", "id").unwrap_or(0),
                coordinator_id: row.try_get("", "coordinator_id").unwrap_or_default(),
                developer: row.try_get("", "developer").unwrap_or_default(),
                agent: row.try_get("", "agent").unwrap_or_default(),
                app: row.try_get("", "app").unwrap_or_default(),
                job_id: row.try_get("", "job_id").unwrap_or_default(),
                sequences,
                event_timestamp: row.try_get::<i64>("", "event_timestamp").unwrap_or(0) as u64,
                level,
                message: row.try_get("", "message").unwrap_or_default(),
                created_at_timestamp,
            });
        }

        Ok((results, total_count))
    }

    /// Search CoordinatorMessageEvent using full-text search on message content
    /// Uses TiDB's FTS_MATCH_WORD function with automatic language detection and BM25 relevance ranking
    pub async fn search_coordinator_message_events(
        &self,
        search_query: &str,
        limit: Option<u32>,
        offset: Option<u32>,
        coordinator_id: Option<String>,
    ) -> Result<(Vec<CoordinatorMessageEventResult>, u32)> {
        use sea_orm::{ConnectionTrait, Statement};

        if search_query.trim().is_empty() {
            return Ok((Vec::new(), 0));
        }

        // Escape single quotes in search query to prevent SQL injection
        let escaped_query = search_query.replace("'", "''");

        // Build the WHERE clause - TiDB requires literal strings for FTS_MATCH_WORD, not parameters
        let mut where_conditions = vec![format!("fts_match_word('{}', message)", escaped_query)];
        let mut params: Vec<sea_orm::Value> = Vec::new();

        if let Some(coord_id) = &coordinator_id {
            where_conditions.push("coordinator_id = ?".to_string());
            params.push(coord_id.clone().into());
        }

        let where_clause = where_conditions.join(" AND ");

        // First, get the count for pagination
        let count_query = format!(
            "SELECT COUNT(*) as count 
             FROM coordinator_message_event 
             WHERE {}",
            where_clause
        );

        let count_stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::MySql,
            &count_query,
            params.clone(),
        );

        let count_result = self.connection.query_one(count_stmt).await?;
        let total_count: u32 = count_result
            .map(|row| row.try_get::<i64>("", "count").unwrap_or(0) as u32)
            .unwrap_or(0);

        if total_count == 0 {
            return Ok((Vec::new(), 0));
        }

        // Build the main query with full-text search and relevance scoring
        let mut main_query = format!(
            "SELECT id, coordinator_id, event_timestamp, level, message, created_at,
                    fts_match_word('{}', message) as relevance_score
             FROM coordinator_message_event 
             WHERE {}
             ORDER BY fts_match_word('{}', message) DESC, created_at DESC",
            escaped_query, where_clause, escaped_query
        );

        // Add pagination - TiDB requires LIMIT when using FTS_MATCH_WORD in ORDER BY
        let limit_val = limit.unwrap_or(1000); // Use large default limit if none specified
        main_query.push_str(&format!(" LIMIT {}", limit_val));
        if let Some(offset_val) = offset {
            main_query.push_str(&format!(" OFFSET {}", offset_val));
        }

        let main_stmt =
            Statement::from_sql_and_values(sea_orm::DatabaseBackend::MySql, &main_query, params);

        let rows = self.connection.query_all(main_stmt).await?;

        let mut results = Vec::new();
        for row in rows {
            let created_at_timestamp: i64 = row
                .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            let level: i32 = row.try_get::<i32>("", "level").unwrap_or(0);

            let relevance_score: f64 = row.try_get::<f64>("", "relevance_score").unwrap_or(0.0);

            results.push(CoordinatorMessageEventResult {
                id: row.try_get("", "id").unwrap_or(0),
                coordinator_id: row.try_get("", "coordinator_id").unwrap_or_default(),
                event_timestamp: row.try_get::<i64>("", "event_timestamp").unwrap_or(0) as u64,
                level,
                message: row.try_get("", "message").unwrap_or_default(),
                created_at_timestamp,
                relevance_score,
            });
        }

        Ok((results, total_count))
    }
}

// Conversion functions from protobuf messages to Sea-ORM entities

fn convert_coordinator_started_event(
    event: &crate::events::CoordinatorStartedEvent,
) -> entities::coordinator_started_event::ActiveModel {
    use entities::coordinator_started_event::*;
    use sea_orm::ActiveValue;

    ActiveModel {
        id: ActiveValue::NotSet,
        coordinator_id: ActiveValue::Set(event.coordinator_id.clone()),
        ethereum_address: ActiveValue::Set(event.ethereum_address.clone()),
        sui_ed_25519_address: ActiveValue::Set(event.sui_ed25519_address.clone()),
        event_timestamp: ActiveValue::Set(event.event_timestamp as i64),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    }
}

fn convert_agent_started_job_event(
    event: &crate::events::AgentStartedJobEvent,
) -> entities::agent_started_job_event::ActiveModel {
    use entities::agent_started_job_event::*;
    use sea_orm::ActiveValue;

    ActiveModel {
        id: ActiveValue::NotSet,
        coordinator_id: ActiveValue::Set(event.coordinator_id.clone()),
        developer: ActiveValue::Set(event.developer.clone()),
        agent: ActiveValue::Set(event.agent.clone()),
        app: ActiveValue::Set(event.app.clone()),
        job_id: ActiveValue::Set(event.job_id.clone()),
        event_timestamp: ActiveValue::Set(event.event_timestamp as i64),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    }
}

fn convert_agent_finished_job_event(
    event: &crate::events::AgentFinishedJobEvent,
) -> entities::agent_finished_job_event::ActiveModel {
    use entities::agent_finished_job_event::*;
    use sea_orm::ActiveValue;

    ActiveModel {
        id: ActiveValue::NotSet,
        coordinator_id: ActiveValue::Set(event.coordinator_id.clone()),
        developer: ActiveValue::Set(event.developer.clone()),
        agent: ActiveValue::Set(event.agent.clone()),
        app: ActiveValue::Set(event.app.clone()),
        job_id: ActiveValue::Set(event.job_id.clone()),
        duration: ActiveValue::Set(event.duration as i64),
        event_timestamp: ActiveValue::Set(event.event_timestamp as i64),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    }
}

fn convert_coordination_tx_event(
    event: &crate::events::CoordinationTxEvent,
) -> entities::coordination_tx_event::ActiveModel {
    use entities::coordination_tx_event::*;
    use sea_orm::ActiveValue;

    ActiveModel {
        id: ActiveValue::NotSet,
        coordinator_id: ActiveValue::Set(event.coordinator_id.clone()),
        developer: ActiveValue::Set(event.developer.clone()),
        agent: ActiveValue::Set(event.agent.clone()),
        app: ActiveValue::Set(event.app.clone()),
        job_id: ActiveValue::Set(event.job_id.clone()),
        memo: ActiveValue::Set(event.memo.clone()),
        tx_hash: ActiveValue::Set(event.tx_hash.clone()),
        event_timestamp: ActiveValue::Set(event.event_timestamp as i64),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    }
}

fn convert_coordinator_message_event(
    event: &crate::events::CoordinatorMessageEvent,
) -> entities::coordinator_message_event::ActiveModel {
    use entities::coordinator_message_event::*;
    use sea_orm::ActiveValue;

    ActiveModel {
        id: ActiveValue::NotSet,
        coordinator_id: ActiveValue::Set(event.coordinator_id.clone()),
        event_timestamp: ActiveValue::Set(event.event_timestamp as i64),
        level: ActiveValue::Set(event.level().into()),
        message: ActiveValue::Set(event.message.clone()),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    }
}

fn convert_client_transaction_event(
    event: &crate::events::ClientTransactionEvent,
) -> entities::client_transaction_event::ActiveModel {
    use entities::client_transaction_event::*;
    use sea_orm::ActiveValue;

    ActiveModel {
        id: ActiveValue::NotSet,
        coordinator_id: ActiveValue::Set(event.coordinator_id.clone()),
        developer: ActiveValue::Set(event.developer.clone()),
        agent: ActiveValue::Set(event.agent.clone()),
        app: ActiveValue::Set(event.app.clone()),
        client_ip_address: ActiveValue::Set(event.client_ip_address.clone()),
        method: ActiveValue::Set(event.method.clone()),
        data: ActiveValue::Set(event.data.clone()),
        tx_hash: ActiveValue::Set(event.tx_hash.clone()),
        sequence: ActiveValue::Set(event.sequence as i64),
        event_timestamp: ActiveValue::Set(event.event_timestamp as i64),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    }
}

fn convert_agent_message_event(
    event: &crate::events::AgentMessageEvent,
) -> (entities::agent_message_event::ActiveModel, Vec<u64>) {
    use entities::agent_message_event::*;
    use sea_orm::ActiveValue;

    let sequences = event.sequences.clone();

    let active_model = ActiveModel {
        id: ActiveValue::NotSet,
        coordinator_id: ActiveValue::Set(event.coordinator_id.clone()),
        developer: ActiveValue::Set(event.developer.clone()),
        agent: ActiveValue::Set(event.agent.clone()),
        app: ActiveValue::Set(event.app.clone()),
        job_id: ActiveValue::Set(event.job_id.clone()),
        event_timestamp: ActiveValue::Set(event.event_timestamp as i64),
        level: ActiveValue::Set(event.level().into()),
        message: ActiveValue::Set(event.message.clone()),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    (active_model, sequences)
}

fn convert_agent_transaction_event(
    event: &crate::events::AgentTransactionEvent,
) -> (entities::agent_transaction_event::ActiveModel, Vec<u64>) {
    use entities::agent_transaction_event::*;
    use sea_orm::ActiveValue;

    let sequences = event.sequences.clone();

    let active_model = ActiveModel {
        id: ActiveValue::NotSet,
        coordinator_id: ActiveValue::Set(event.coordinator_id.clone()),
        tx_type: ActiveValue::Set(event.tx_type.clone()),
        developer: ActiveValue::Set(event.developer.clone()),
        agent: ActiveValue::Set(event.agent.clone()),
        app: ActiveValue::Set(event.app.clone()),
        job_id: ActiveValue::Set(event.job_id.clone()),
        event_timestamp: ActiveValue::Set(event.event_timestamp as i64),
        tx_hash: ActiveValue::Set(event.tx_hash.clone()),
        chain: ActiveValue::Set(event.chain.clone()),
        network: ActiveValue::Set(event.network.clone()),
        memo: ActiveValue::Set(event.memo.clone()),
        metadata: ActiveValue::Set(event.metadata.clone()),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    (active_model, sequences)
}
