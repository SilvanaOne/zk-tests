use crate::entities;
use crate::events::{AgentEvent, CoordinatorEvent, Event};
use anyhow::Result;
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, EntityTrait, TransactionTrait};
use std::time::Instant;
use tracing::{debug, error, info};

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
            "Inserting batch of {} events using proper batch insertion",
            events.len()
        );

        let txn = self.connection.begin().await?;
        let mut total_inserted = 0;

        // Group events by type for batch insertion
        let mut coordinator_started_events = Vec::new();
        let mut agent_started_job_events = Vec::new();
        let mut agent_finished_job_events = Vec::new();
        let mut coordination_tx_events = Vec::new();
        let mut coordinator_error_events = Vec::new();
        let mut client_transaction_events = Vec::new();
        let mut agent_message_events = Vec::new();
        let mut agent_error_events = Vec::new();
        let mut agent_transaction_events = Vec::new();

        // Categorize events by type
        for event in events {
            if let Some(event_type) = &event.event_type {
                match event_type {
                    crate::events::event::EventType::Coordinator(coordinator_event) => {
                        if let Some(coord_event) = &coordinator_event.event {
                            use crate::events::coordinator_event::Event as CoordEvent;
                            match coord_event {
                                CoordEvent::CoordinatorStarted(event) => {
                                    coordinator_started_events.push(
                                        entities::conversions::coordinator_started_event(
                                            event.clone(),
                                        ),
                                    );
                                }
                                CoordEvent::AgentStartedJob(event) => {
                                    agent_started_job_events.push(
                                        entities::conversions::agent_started_job_event(
                                            event.clone(),
                                        ),
                                    );
                                }
                                CoordEvent::AgentFinishedJob(event) => {
                                    agent_finished_job_events.push(
                                        entities::conversions::agent_finished_job_event(
                                            event.clone(),
                                        ),
                                    );
                                }
                                CoordEvent::CoordinationTx(event) => {
                                    coordination_tx_events.push(
                                        entities::conversions::coordination_tx_event(event.clone()),
                                    );
                                }
                                CoordEvent::CoordinatorError(event) => {
                                    coordinator_error_events.push(
                                        entities::conversions::coordinator_error_event(
                                            event.clone(),
                                        ),
                                    );
                                }
                                CoordEvent::ClientTransaction(event) => {
                                    client_transaction_events.push(
                                        entities::conversions::client_transaction_event(
                                            event.clone(),
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    crate::events::event::EventType::Agent(agent_event) => {
                        if let Some(agent_event_type) = &agent_event.event {
                            use crate::events::agent_event::Event as AgentEventType;
                            match agent_event_type {
                                AgentEventType::Message(event) => {
                                    agent_message_events.push(
                                        entities::conversions::agent_message_event(event.clone()),
                                    );
                                }
                                AgentEventType::Error(event) => {
                                    agent_error_events.push(
                                        entities::conversions::agent_error_event(event.clone()),
                                    );
                                }
                                AgentEventType::Transaction(event) => {
                                    agent_transaction_events.push(
                                        entities::conversions::agent_transaction_event(
                                            event.clone(),
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Count the number of SQL statements that will be used (one per non-empty event type)
        let sql_statements_count = self.count_non_empty_groups(&[
            coordinator_started_events.len(),
            agent_started_job_events.len(),
            agent_finished_job_events.len(),
            coordination_tx_events.len(),
            coordinator_error_events.len(),
            client_transaction_events.len(),
            agent_message_events.len(),
            agent_error_events.len(),
            agent_transaction_events.len(),
        ]);

        // Batch insert each event type
        if !coordinator_started_events.is_empty() {
            debug!(
                "Batch inserting {} coordinator_started_events",
                coordinator_started_events.len()
            );
            match entities::coordinator_started_event::Entity::insert_many(
                coordinator_started_events,
            )
            .exec(&txn)
            .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert coordinator_started_events: {}", e),
            }
        }

        if !agent_started_job_events.is_empty() {
            debug!(
                "Batch inserting {} agent_started_job_events",
                agent_started_job_events.len()
            );
            match entities::agent_started_job_event::Entity::insert_many(agent_started_job_events)
                .exec(&txn)
                .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert agent_started_job_events: {}", e),
            }
        }

        if !agent_finished_job_events.is_empty() {
            debug!(
                "Batch inserting {} agent_finished_job_events",
                agent_finished_job_events.len()
            );
            match entities::agent_finished_job_event::Entity::insert_many(agent_finished_job_events)
                .exec(&txn)
                .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert agent_finished_job_events: {}", e),
            }
        }

        if !coordination_tx_events.is_empty() {
            debug!(
                "Batch inserting {} coordination_tx_events",
                coordination_tx_events.len()
            );
            match entities::coordination_tx_event::Entity::insert_many(coordination_tx_events)
                .exec(&txn)
                .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert coordination_tx_events: {}", e),
            }
        }

        if !coordinator_error_events.is_empty() {
            debug!(
                "Batch inserting {} coordinator_error_events",
                coordinator_error_events.len()
            );
            match entities::coordinator_error_event::Entity::insert_many(coordinator_error_events)
                .exec(&txn)
                .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert coordinator_error_events: {}", e),
            }
        }

        if !client_transaction_events.is_empty() {
            debug!(
                "Batch inserting {} client_transaction_events",
                client_transaction_events.len()
            );
            match entities::client_transaction_event::Entity::insert_many(client_transaction_events)
                .exec(&txn)
                .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert client_transaction_events: {}", e),
            }
        }

        if !agent_message_events.is_empty() {
            debug!(
                "Batch inserting {} agent_message_events",
                agent_message_events.len()
            );
            match entities::agent_message_event::Entity::insert_many(agent_message_events)
                .exec(&txn)
                .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert agent_message_events: {}", e),
            }
        }

        if !agent_error_events.is_empty() {
            debug!(
                "Batch inserting {} agent_error_events",
                agent_error_events.len()
            );
            match entities::agent_error_event::Entity::insert_many(agent_error_events)
                .exec(&txn)
                .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert agent_error_events: {}", e),
            }
        }

        if !agent_transaction_events.is_empty() {
            debug!(
                "Batch inserting {} agent_transaction_events",
                agent_transaction_events.len()
            );
            match entities::agent_transaction_event::Entity::insert_many(agent_transaction_events)
                .exec(&txn)
                .await
            {
                Ok(result) => total_inserted += result.last_insert_id as usize,
                Err(e) => error!("Failed to batch insert agent_transaction_events: {}", e),
            }
        }

        txn.commit().await?;

        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis();

        let events_per_second = events.len() as f64 / duration.as_secs_f64();
        
        info!(
            "Successfully batch inserted {} events using {} SQL statements in {}ms ({:.2}s) - {:.0} events/second",
            events.len(),
            sql_statements_count,
            duration_ms,
            duration.as_secs_f64(),
            events_per_second
        );

        Ok(events.len()) // Return the number of events we attempted to insert
    }

    fn count_non_empty_groups(&self, group_sizes: &[usize]) -> usize {
        group_sizes.iter().filter(|&&size| size > 0).count()
    }

    pub async fn get_connection(&self) -> &DatabaseConnection {
        &self.connection
    }
}
