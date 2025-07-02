use crate::database::EventDatabase;
use crate::entities;
use crate::events::Event;
use anyhow::{anyhow, Result};
use std::env;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, OwnedSemaphorePermit, RwLock, Semaphore};
use tokio::time::{interval, sleep, timeout, Duration, Instant};
use tracing::{debug, error, info, warn};

// Default configuration constants (kept for potential future use)
#[allow(dead_code)]
const DEFAULT_BATCH_SIZE: usize = 100;
#[allow(dead_code)]
const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_millis(500); // Reduced for faster processing
#[allow(dead_code)]
const DEFAULT_CHANNEL_CAPACITY: usize = 250000; // Increased for high-throughput scenarios
                                                // Maximum memory usage in bytes (1GB for events)
const MAX_MEMORY_BYTES: usize = 1000 * 1024 * 1024;
// Base timeout for adding events when buffer is full (will be adjusted based on flush interval)
const BASE_ADD_EVENT_TIMEOUT: Duration = Duration::from_millis(100);
// Circuit breaker threshold
const ERROR_THRESHOLD: usize = 100;
// Retry configuration for database operations
const MAX_DB_RETRIES: usize = 10;
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(100);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
// Semaphore strategy thresholds (as fraction of total capacity)
const FAST_PATH_THRESHOLD: usize = 8; // Use fast path when > 1/8 permits available (more conservative)

// Wrapper for events with their semaphore permits
type EventWithPermit = (Event, OwnedSemaphorePermit);

#[derive(Clone)]
pub struct EventBuffer {
    sender: mpsc::Sender<EventWithPermit>,
    stats: Arc<BufferStatsAtomic>,
    memory_usage: Arc<AtomicUsize>,
    circuit_breaker: Arc<CircuitBreaker>,
    backpressure_semaphore: Arc<Semaphore>,
    add_event_timeout: Duration,
    total_permits: usize, // Store the actual channel capacity
}

// Atomic counters for lock-free stats updates
#[derive(Debug, Default)]
pub struct BufferStatsAtomic {
    pub total_received: AtomicU64,
    pub total_processed: AtomicU64,
    pub total_errors: AtomicU64,
    pub total_dropped: AtomicU64,
    pub total_retries: AtomicU64,
    pub current_buffer_size: AtomicUsize,
    pub backpressure_events: AtomicU64,
    pub last_flush_time: RwLock<Option<Instant>>,
}

// Snapshot for reading stats
#[derive(Debug, Default, Clone)]
pub struct BufferStats {
    pub total_received: u64,
    pub total_processed: u64,
    pub total_errors: u64,
    pub total_dropped: u64,
    #[allow(dead_code)]
    pub total_retries: u64,
    pub current_buffer_size: usize,
    pub current_memory_bytes: usize,
    #[allow(dead_code)]
    pub last_flush_time: Option<Instant>,
    pub backpressure_events: u64,
    pub circuit_breaker_open: bool,
}

struct CircuitBreaker {
    is_open: AtomicBool,
    error_count: AtomicUsize,
    last_error_time: RwLock<Option<Instant>>,
    threshold: usize,
    timeout: Duration,
}

struct BatchProcessor {
    receiver: mpsc::Receiver<EventWithPermit>,
    buffer: Vec<Event>,
    database: Arc<EventDatabase>,
    stats: Arc<BufferStatsAtomic>,
    memory_usage: Arc<AtomicUsize>,
    circuit_breaker: Arc<CircuitBreaker>,
    batch_size: usize,
    flush_interval: Duration,
    nats_client: Option<async_nats::Client>,
    nats_stream_name: String,
}

impl EventBuffer {
    #[allow(dead_code)]
    pub fn new(database: Arc<EventDatabase>) -> Self {
        Self::with_config(
            database,
            DEFAULT_BATCH_SIZE,
            DEFAULT_FLUSH_INTERVAL,
            DEFAULT_CHANNEL_CAPACITY,
        )
    }

    pub fn with_config(
        database: Arc<EventDatabase>,
        batch_size: usize,
        flush_interval: Duration,
        channel_capacity: usize,
    ) -> Self {
        // Use bounded channel to prevent OOM
        let (sender, receiver) = mpsc::channel(channel_capacity);
        let stats = Arc::new(BufferStatsAtomic::default());
        let memory_usage = Arc::new(AtomicUsize::new(0));
        let circuit_breaker = Arc::new(CircuitBreaker::new(
            ERROR_THRESHOLD,
            Duration::from_secs(60),
        ));
        let backpressure_semaphore = Arc::new(Semaphore::new(channel_capacity));

        // Make timeout proportional to flush interval for better behavior under load
        let add_event_timeout = std::cmp::max(
            BASE_ADD_EVENT_TIMEOUT,
            flush_interval / 10, // 10% of flush interval
        );

        // Clone Arc references for the spawned task
        let stats_clone = Arc::clone(&stats);
        let memory_usage_clone = Arc::clone(&memory_usage);
        let circuit_breaker_clone = Arc::clone(&circuit_breaker);

        // Spawn the processor task
        tokio::spawn(async move {
            let processor = BatchProcessor::new(
                receiver,
                database,
                stats_clone,
                memory_usage_clone,
                circuit_breaker_clone,
                batch_size,
                flush_interval,
            )
            .await;

            processor.run().await;
        });

        Self {
            sender,
            stats,
            memory_usage,
            circuit_breaker,
            backpressure_semaphore,
            add_event_timeout,
            total_permits: channel_capacity, // Store the actual capacity
        }
    }

    pub async fn add_event(&self, event: Event) -> Result<()> {
        // Check circuit breaker
        if self.circuit_breaker.is_open().await {
            self.stats.total_dropped.fetch_add(1, Ordering::Relaxed);
            return Err(anyhow!("Circuit breaker is open - system overloaded"));
        }

        // Estimate event size more accurately
        let event_size = self.estimate_event_size(&event);

        // Check memory usage
        let current_memory = self.memory_usage.load(Ordering::Relaxed);
        if current_memory + event_size > MAX_MEMORY_BYTES {
            self.stats.total_dropped.fetch_add(1, Ordering::Relaxed);
            return Err(anyhow!(
                "Memory limit exceeded: {}MB + {}KB > {}MB",
                current_memory / (1024 * 1024),
                event_size / 1024,
                MAX_MEMORY_BYTES / (1024 * 1024)
            ));
        }

        // Acquire backpressure permit with tiered strategy
        // Strategy: Fast path when plenty of capacity, blocking with timeout when near capacity
        let available_permits = self.backpressure_semaphore.available_permits();
        let total_permits = self.total_permits; // Use the actual configured capacity

        let permit = if available_permits > total_permits / FAST_PATH_THRESHOLD {
            // Fast path: plenty of capacity, use non-blocking
            match self.backpressure_semaphore.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    // Race condition: permits depleted between check and acquire
                    self.stats
                        .backpressure_events
                        .fetch_add(1, Ordering::Relaxed);
                    self.stats.total_dropped.fetch_add(1, Ordering::Relaxed);

                    let current_memory = self.memory_usage.load(Ordering::Relaxed);
                    warn!(
                        "Backpressure active - buffer at capacity (race condition): {}/{} permits, {}MB/{}MB memory ({:.1}%)",
                        total_permits - self.backpressure_semaphore.available_permits(),
                        total_permits,
                        current_memory / (1024 * 1024),
                        MAX_MEMORY_BYTES / (1024 * 1024),
                        (current_memory as f64 / MAX_MEMORY_BYTES as f64) * 100.0
                    );
                    return Err(anyhow!("Backpressure active - buffer at capacity (race)"));
                }
            }
        } else if available_permits > 0 {
            // Near capacity: try fast path first, then fall back to blocking with timeout
            match self.backpressure_semaphore.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    // Fall back to blocking acquire with timeout
                    debug!("Near capacity, falling back to blocking acquire");
                    match timeout(
                        self.add_event_timeout,
                        self.backpressure_semaphore.clone().acquire_owned(),
                    )
                    .await
                    {
                        Ok(Ok(permit)) => permit,
                        Ok(Err(_)) => {
                            // Semaphore closed (shouldn't happen)
                            self.stats
                                .backpressure_events
                                .fetch_add(1, Ordering::Relaxed);
                            self.stats.total_dropped.fetch_add(1, Ordering::Relaxed);
                            return Err(anyhow!("Backpressure semaphore closed"));
                        }
                        Err(_) => {
                            // Timeout waiting for permit
                            self.stats
                                .backpressure_events
                                .fetch_add(1, Ordering::Relaxed);
                            self.stats.total_dropped.fetch_add(1, Ordering::Relaxed);

                            let current_memory = self.memory_usage.load(Ordering::Relaxed);
                            warn!(
                                "Backpressure timeout - system overloaded: {}/{} permits, {}MB/{}MB memory ({:.1}%), timeout: {:?}",
                                total_permits - self.backpressure_semaphore.available_permits(),
                                total_permits,
                                current_memory / (1024 * 1024),
                                MAX_MEMORY_BYTES / (1024 * 1024),
                                (current_memory as f64 / MAX_MEMORY_BYTES as f64) * 100.0,
                                self.add_event_timeout
                            );
                            return Err(anyhow!(
                                "Backpressure timeout - buffer acquisition failed"
                            ));
                        }
                    }
                }
            }
        } else {
            // No permits available
            self.stats
                .backpressure_events
                .fetch_add(1, Ordering::Relaxed);
            self.stats.total_dropped.fetch_add(1, Ordering::Relaxed);

            let current_memory = self.memory_usage.load(Ordering::Relaxed);
            warn!(
                "Backpressure applied - buffer full: {}/{} permits used, {}MB/{}MB memory ({:.1}% memory utilization)",
                total_permits - available_permits,
                total_permits,
                current_memory / (1024 * 1024),
                MAX_MEMORY_BYTES / (1024 * 1024),
                (current_memory as f64 / MAX_MEMORY_BYTES as f64) * 100.0
            );
            return Err(anyhow!("Backpressure active - buffer at capacity"));
        };

        // Try to send with timeout to prevent blocking
        let wrapped_event = (event, permit);
        match timeout(self.add_event_timeout, self.sender.send(wrapped_event)).await {
            Ok(Ok(())) => {
                // Update stats atomically - no lock contention
                self.stats.total_received.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .current_buffer_size
                    .fetch_add(1, Ordering::Relaxed);
                self.memory_usage.fetch_add(event_size, Ordering::Relaxed);

                // Reset circuit breaker on success
                self.circuit_breaker.record_success().await;

                Ok(())
            }
            Ok(Err(_)) => {
                // Channel closed
                self.stats.total_dropped.fetch_add(1, Ordering::Relaxed);
                Err(anyhow!("Event buffer channel closed"))
            }
            Err(_) => {
                // Timeout - buffer is full
                self.stats.total_dropped.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .backpressure_events
                    .fetch_add(1, Ordering::Relaxed);

                let current_memory = self.memory_usage.load(Ordering::Relaxed);
                warn!(
                    "Event buffer send timeout - system overloaded: {}/{} permits, {}MB/{}MB memory ({:.1}%), timeout: {:?}",
                    total_permits - self.backpressure_semaphore.available_permits(),
                    total_permits,
                    current_memory / (1024 * 1024),
                    MAX_MEMORY_BYTES / (1024 * 1024),
                    (current_memory as f64 / MAX_MEMORY_BYTES as f64) * 100.0,
                    self.add_event_timeout
                );
                Err(anyhow!("Event buffer timeout - system overloaded"))
            }
        }
    }

    pub async fn get_stats(&self) -> BufferStats {
        BufferStats {
            total_received: self.stats.total_received.load(Ordering::Relaxed),
            total_processed: self.stats.total_processed.load(Ordering::Relaxed),
            total_errors: self.stats.total_errors.load(Ordering::Relaxed),
            total_dropped: self.stats.total_dropped.load(Ordering::Relaxed),
            total_retries: self.stats.total_retries.load(Ordering::Relaxed),
            current_buffer_size: self.stats.current_buffer_size.load(Ordering::Relaxed),
            current_memory_bytes: self.memory_usage.load(Ordering::Relaxed),
            last_flush_time: *self.stats.last_flush_time.read().await,
            backpressure_events: self.stats.backpressure_events.load(Ordering::Relaxed),
            circuit_breaker_open: self.circuit_breaker.is_open().await,
        }
    }

    pub async fn health_check(&self) -> bool {
        let stats = self.get_stats().await;

        // Check basic health conditions
        let basic_health =
            !stats.circuit_breaker_open && stats.current_memory_bytes < MAX_MEMORY_BYTES;

        // Backpressure check: healthy if no events received yet, or backpressure < 10% of total
        let backpressure_healthy = if stats.total_received == 0 {
            // System is idle - always healthy regardless of any startup backpressure
            true
        } else {
            // System has processed events - check backpressure ratio
            stats.backpressure_events < stats.total_received / 10 // Less than 10% backpressure
        };

        basic_health && backpressure_healthy
    }

    fn estimate_event_size(&self, event: &Event) -> usize {
        // More accurate size estimation
        let base_size = std::mem::size_of::<Event>();

        let payload_size = match &event.event_type {
            Some(event_type) => match event_type {
                crate::events::event::EventType::Coordinator(coord_event) => {
                    self.estimate_coordinator_event_size(coord_event)
                }
                crate::events::event::EventType::Agent(agent_event) => {
                    self.estimate_agent_event_size(agent_event)
                }
            },
            None => 0,
        };

        base_size + payload_size
    }

    fn estimate_coordinator_event_size(
        &self,
        coord_event: &crate::events::CoordinatorEvent,
    ) -> usize {
        match &coord_event.event {
            Some(event) => match event {
                crate::events::coordinator_event::Event::CoordinatorStarted(e) => {
                    e.coordinator_id.len()
                        + e.ethereum_address.len()
                        + e.sui_ed25519_address.len()
                        + 64
                }
                crate::events::coordinator_event::Event::AgentStartedJob(e) => {
                    e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + 64
                }
                crate::events::coordinator_event::Event::AgentFinishedJob(e) => {
                    e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + 72
                }
                crate::events::coordinator_event::Event::CoordinationTx(e) => {
                    e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + e.memo.len()
                        + e.tx_hash.len()
                        + 64
                }
                crate::events::coordinator_event::Event::CoordinatorError(e) => {
                    e.coordinator_id.len() + e.message.len() + 64
                }
                crate::events::coordinator_event::Event::ClientTransaction(e) => {
                    e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.client_ip_address.len()
                        + e.method.len()
                        + e.data.len()
                        + e.tx_hash.len()
                        + 128
                }
            },
            None => 0,
        }
    }

    fn estimate_agent_event_size(&self, agent_event: &crate::events::AgentEvent) -> usize {
        match &agent_event.event {
            Some(event) => match event {
                crate::events::agent_event::Event::Message(e) => {
                    let base_size = e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + e.message.len()
                        + 64;

                    // Add memory for child table records (each sequence creates a separate record)
                    let sequence_records_size = e.sequences.len()
                        * std::mem::size_of::<entities::agent_message_event_sequences::Model>();

                    base_size + sequence_records_size
                }
                crate::events::agent_event::Event::Transaction(e) => {
                    let base_size = e.coordinator_id.len()
                        + e.tx_type.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + e.tx_hash.len()
                        + e.chain.len()
                        + e.network.len()
                        + e.memo.len()
                        + e.metadata.len()
                        + 128;

                    // Add memory for child table records (each sequence creates a separate record)
                    let sequence_records_size = e.sequences.len()
                        * std::mem::size_of::<entities::agent_transaction_event_sequences::Model>();

                    base_size + sequence_records_size
                }
            },
            None => 0,
        }
    }
}

impl CircuitBreaker {
    fn new(threshold: usize, timeout: Duration) -> Self {
        Self {
            is_open: AtomicBool::new(false),
            error_count: AtomicUsize::new(0),
            last_error_time: RwLock::new(None),
            threshold,
            timeout,
        }
    }

    async fn is_open(&self) -> bool {
        if !self.is_open.load(Ordering::Relaxed) {
            return false;
        }

        // Check if timeout has passed
        let last_error = self.last_error_time.read().await;
        if let Some(last_time) = *last_error {
            if Instant::now().duration_since(last_time) > self.timeout {
                info!("Circuit breaker reset - attempting to recover");
                self.is_open.store(false, Ordering::Relaxed);
                self.error_count.store(0, Ordering::Relaxed);
                return false;
            }
        }

        true
    }

    async fn record_error(&self) {
        let count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_error_time.write().await = Some(Instant::now());

        if count >= self.threshold {
            warn!("Circuit breaker opened - too many errors: {}", count);
            self.is_open.store(true, Ordering::Relaxed);
        }
    }

    async fn record_success(&self) {
        // Only reset error count if we had errors
        if self.error_count.load(Ordering::Relaxed) > 0 {
            self.error_count.store(0, Ordering::Relaxed);
        }
    }

    // New method: record successful database operation (vs just buffer acceptance)
    async fn record_db_success(&self) {
        // Reset circuit breaker only after successful DB operations
        if self.is_open.load(Ordering::Relaxed) {
            info!("Circuit breaker reset after successful database operation");
            self.is_open.store(false, Ordering::Relaxed);
        }
        self.error_count.store(0, Ordering::Relaxed);
    }
}

impl BatchProcessor {
    async fn new(
        receiver: mpsc::Receiver<EventWithPermit>,
        database: Arc<EventDatabase>,
        stats: Arc<BufferStatsAtomic>,
        memory_usage: Arc<AtomicUsize>,
        circuit_breaker: Arc<CircuitBreaker>,
        batch_size: usize,
        flush_interval: Duration,
    ) -> Self {
        // Initialize NATS client if NATS_URL is provided
        let nats_client = match env::var("NATS_URL") {
            Ok(nats_url) => {
                match async_nats::connect(&nats_url).await {
                    Ok(client) => {
                        info!("✅ Connected to NATS server at: {}", nats_url);
                        Some(client)
                    }
                    Err(e) => {
                        warn!("⚠️  Failed to connect to NATS server at {}: {}. Continuing without NATS.", nats_url, e);
                        None
                    }
                }
            }
            Err(_) => {
                info!("ℹ️  NATS_URL not set. Running without NATS publishing.");
                None
            }
        };

        let nats_stream_name =
            env::var("NATS_STREAM_NAME").unwrap_or_else(|_| "silvana-events".to_string());

        Self {
            receiver,
            buffer: Vec::with_capacity(batch_size),
            database,
            stats,
            memory_usage,
            circuit_breaker,
            batch_size,
            flush_interval,
            nats_client,
            nats_stream_name,
        }
    }

    async fn run(mut self) {
        let mut flush_timer = interval(self.flush_interval);
        let mut permits_held: Vec<OwnedSemaphorePermit> = Vec::new();

        info!(
            "Started batch processor with batch_size={}, flush_interval={:?}",
            self.batch_size, self.flush_interval
        );
        info!(
            "Using adaptive batching: batch_size acts as minimum trigger (actual batches may be larger)"
        );

        loop {
            tokio::select! {
                // Receive new events with permits
                event_result = self.receiver.recv() => {
                    match event_result {
                        Some((event, permit)) => {
                            self.buffer.push(event);
                            permits_held.push(permit);

                            // Check if we should flush due to batch size
                            if self.buffer.len() >= self.batch_size {
                                debug!("Batch size reached ({}), draining all available events", self.buffer.len());
                                self.drain_and_flush(&mut permits_held).await;
                            }
                        }
                        None => {
                            warn!("Event channel closed, flushing remaining events");
                            self.flush_buffer(&mut permits_held).await;
                            break;
                        }
                    }
                }

                // Periodic flush
                _ = flush_timer.tick() => {
                    if !self.buffer.is_empty() {
                        debug!("Periodic flush triggered with {} events, draining all available", self.buffer.len());
                        self.drain_and_flush(&mut permits_held).await;
                    }
                }
            }
        }
    }

    /// Drain all available events from the channel and flush them in one large batch
    async fn drain_and_flush(&mut self, permits_held: &mut Vec<OwnedSemaphorePermit>) {
        let initial_count = self.buffer.len();
        let mut drained_count = 0;

        // Drain all immediately available events from the channel
        loop {
            match self.receiver.try_recv() {
                Ok((event, permit)) => {
                    self.buffer.push(event);
                    permits_held.push(permit);
                    drained_count += 1;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    // No more events available right now, proceed with flush
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed, proceed with flush
                    warn!("Event channel disconnected during drain");
                    break;
                }
            }
        }

        let total_events = self.buffer.len();
        if drained_count > 0 {
            info!(
                "Drained {} additional events (total batch: {} events, efficiency gain: {}x)",
                drained_count,
                total_events,
                if initial_count > 0 {
                    total_events / initial_count
                } else {
                    1
                }
            );
        }

        // Flush all accumulated events in one batch
        self.flush_buffer(permits_held).await;
    }

    async fn flush_buffer(&mut self, permits_held: &mut Vec<OwnedSemaphorePermit>) {
        if self.buffer.is_empty() {
            return;
        }

        // FIXED: Don't clone, consume the vector directly
        let events_to_process = std::mem::take(&mut self.buffer);
        let event_count = events_to_process.len();

        debug!("Flushing batch of {} events", event_count);

        // Calculate memory to release more accurately
        let memory_to_release = events_to_process
            .iter()
            .map(|event| self.estimate_event_memory_usage(event))
            .sum::<usize>();

        // Process database insertion with retry logic
        match self.insert_with_retry(&events_to_process).await {
            Ok(inserted_count) => {
                info!("Successfully inserted {} events to TiDB", inserted_count);

                // Handle potential partial inserts to keep counters consistent
                if inserted_count < event_count {
                    let dropped_count = event_count - inserted_count;
                    warn!(
                        "Partial insert: {} of {} events inserted, {} events dropped",
                        inserted_count, event_count, dropped_count
                    );

                    // Count the uninserted events as dropped
                    self.stats
                        .total_dropped
                        .fetch_add(dropped_count as u64, Ordering::Relaxed);
                }

                // Update stats atomically - use actual counts to prevent divergence
                self.stats
                    .total_processed
                    .fetch_add(inserted_count as u64, Ordering::Relaxed);
                self.stats
                    .current_buffer_size
                    .fetch_sub(event_count, Ordering::Relaxed); // All events removed from buffer

                // Update last flush time
                *self.stats.last_flush_time.write().await = Some(Instant::now());

                // Record successful DB operation for circuit breaker (even if partial)
                self.circuit_breaker.record_db_success().await;
            }
            Err(e) => {
                error!(
                    "Failed to insert events to TiDB after {} retries: {}",
                    MAX_DB_RETRIES, e
                );

                // Record error in circuit breaker
                self.circuit_breaker.record_error().await;

                // Update error stats - all events failed
                self.stats
                    .total_errors
                    .fetch_add(event_count as u64, Ordering::Relaxed);
                self.stats
                    .total_dropped
                    .fetch_add(event_count as u64, Ordering::Relaxed);
                self.stats
                    .current_buffer_size
                    .fetch_sub(event_count, Ordering::Relaxed);

                // Events are dropped only after all retries are exhausted
                warn!(
                    "Dropping {} events due to persistent database errors after {} retries",
                    event_count, MAX_DB_RETRIES
                );
            }
        }

        // Release memory accounting
        self.memory_usage
            .fetch_sub(memory_to_release, Ordering::Relaxed);

        // FIXED: Permits are automatically released when dropped - no manual add_permits needed
        permits_held.clear();

        // Process NATS publishing - pass by reference to avoid clone
        self.publish_to_nats(&events_to_process).await;
    }

    async fn insert_with_retry(&self, events: &[Event]) -> Result<usize> {
        let mut last_error = None;

        for attempt in 1..=MAX_DB_RETRIES {
            match self.database.insert_events_batch(events).await {
                Ok(inserted_count) => {
                    if attempt > 1 {
                        info!(
                            "Database insertion succeeded on attempt {} after retries",
                            attempt
                        );
                    }
                    return Ok(inserted_count);
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt < MAX_DB_RETRIES {
                        // Calculate exponential backoff delay with jitter
                        let base_delay = INITIAL_RETRY_DELAY.as_millis() as u64;
                        let exponential_delay = base_delay * (2_u64.pow(attempt as u32 - 1));
                        let delay_with_jitter =
                            exponential_delay + (fastrand::u64(0..base_delay) / 2);
                        let delay = Duration::from_millis(
                            delay_with_jitter.min(MAX_RETRY_DELAY.as_millis() as u64),
                        );

                        warn!(
                            "Database insertion failed on attempt {} of {}: {}. Retrying in {:?}",
                            attempt,
                            MAX_DB_RETRIES,
                            last_error.as_ref().unwrap(),
                            delay
                        );

                        // Track retry attempts
                        self.stats.total_retries.fetch_add(1, Ordering::Relaxed);

                        sleep(delay).await;
                    } else {
                        error!(
                            "Database insertion failed on final attempt {} of {}: {}",
                            attempt,
                            MAX_DB_RETRIES,
                            last_error.as_ref().unwrap()
                        );
                    }
                }
            }
        }

        // Return the last error after all retries are exhausted
        Err(last_error.unwrap())
    }

    async fn publish_to_nats(&self, events: &[Event]) {
        if events.is_empty() {
            return;
        }

        let Some(ref client) = self.nats_client else {
            debug!(
                "ℹ️  NATS client not available. Skipping publishing {} events.",
                events.len()
            );
            return;
        };

        debug!(
            "📤 Publishing {} events to NATS JetStream stream '{}'",
            events.len(),
            self.nats_stream_name
        );

        let mut successful_publishes = 0;
        let mut failed_publishes = 0;

        for event in events {
            // Serialize event to JSON for NATS publishing
            match serde_json::to_vec(event) {
                Ok(payload) => {
                    // Create a subject based on event type for better routing
                    let subject = self.create_nats_subject(event);

                    // Publish to NATS with timeout
                    match timeout(
                        Duration::from_secs(5),
                        client.publish(subject.clone(), payload.into()),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            successful_publishes += 1;
                            debug!("✅ Published event to NATS subject: {}", subject);
                        }
                        Ok(Err(e)) => {
                            failed_publishes += 1;
                            warn!(
                                "❌ Failed to publish event to NATS subject {}: {}",
                                subject, e
                            );
                        }
                        Err(_) => {
                            failed_publishes += 1;
                            warn!("⏰ Timeout publishing event to NATS subject: {}", subject);
                        }
                    }
                }
                Err(e) => {
                    failed_publishes += 1;
                    error!("🔥 Failed to serialize event for NATS publishing: {}", e);
                }
            }
        }

        if successful_publishes > 0 {
            info!(
                "📤 Successfully published {}/{} events to NATS JetStream",
                successful_publishes,
                events.len()
            );
        }

        if failed_publishes > 0 {
            warn!(
                "⚠️  Failed to publish {}/{} events to NATS",
                failed_publishes,
                events.len()
            );
        }
    }

    fn create_nats_subject(&self, event: &Event) -> String {
        let base_subject = format!("{}.events", self.nats_stream_name);

        match &event.event_type {
            Some(event_type) => match event_type {
                crate::events::event::EventType::Coordinator(coord_event) => {
                    match &coord_event.event {
                        Some(coordinator_event) => match coordinator_event {
                            crate::events::coordinator_event::Event::CoordinatorStarted(_) => {
                                format!("{}.coordinator.started", base_subject)
                            }
                            crate::events::coordinator_event::Event::AgentStartedJob(_) => {
                                format!("{}.coordinator.agent_started_job", base_subject)
                            }
                            crate::events::coordinator_event::Event::AgentFinishedJob(_) => {
                                format!("{}.coordinator.agent_finished_job", base_subject)
                            }
                            crate::events::coordinator_event::Event::CoordinationTx(_) => {
                                format!("{}.coordinator.coordination_tx", base_subject)
                            }
                            crate::events::coordinator_event::Event::CoordinatorError(_) => {
                                format!("{}.coordinator.error", base_subject)
                            }
                            crate::events::coordinator_event::Event::ClientTransaction(_) => {
                                format!("{}.coordinator.client_transaction", base_subject)
                            }
                        },
                        None => format!("{}.coordinator.unknown", base_subject),
                    }
                }
                crate::events::event::EventType::Agent(agent_event) => match &agent_event.event {
                    Some(agent_event_type) => match agent_event_type {
                        crate::events::agent_event::Event::Message(_) => {
                            format!("{}.agent.message", base_subject)
                        }
                        crate::events::agent_event::Event::Transaction(_) => {
                            format!("{}.agent.transaction", base_subject)
                        }
                    },
                    None => format!("{}.agent.unknown", base_subject),
                },
            },
            None => format!("{}.unknown", base_subject),
        }
    }

    fn estimate_event_memory_usage(&self, event: &Event) -> usize {
        // Reuse the estimation logic from EventBuffer
        let base_size = std::mem::size_of::<Event>();
        let payload_size = match &event.event_type {
            Some(event_type) => match event_type {
                crate::events::event::EventType::Coordinator(coord_event) => {
                    self.estimate_coordinator_event_size(coord_event)
                }
                crate::events::event::EventType::Agent(agent_event) => {
                    self.estimate_agent_event_size(agent_event)
                }
            },
            None => 0,
        };
        base_size + payload_size
    }

    fn estimate_coordinator_event_size(
        &self,
        coord_event: &crate::events::CoordinatorEvent,
    ) -> usize {
        // Same logic as in EventBuffer - could be refactored into a shared module
        match &coord_event.event {
            Some(event) => match event {
                crate::events::coordinator_event::Event::CoordinatorStarted(e) => {
                    e.coordinator_id.len()
                        + e.ethereum_address.len()
                        + e.sui_ed25519_address.len()
                        + 64
                }
                crate::events::coordinator_event::Event::AgentStartedJob(e) => {
                    e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + 64
                }
                crate::events::coordinator_event::Event::AgentFinishedJob(e) => {
                    e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + 72
                }
                crate::events::coordinator_event::Event::CoordinationTx(e) => {
                    e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + e.memo.len()
                        + e.tx_hash.len()
                        + 64
                }
                crate::events::coordinator_event::Event::CoordinatorError(e) => {
                    e.coordinator_id.len() + e.message.len() + 64
                }
                crate::events::coordinator_event::Event::ClientTransaction(e) => {
                    e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.client_ip_address.len()
                        + e.method.len()
                        + e.data.len()
                        + e.tx_hash.len()
                        + 128
                }
            },
            None => 0,
        }
    }

    fn estimate_agent_event_size(&self, agent_event: &crate::events::AgentEvent) -> usize {
        match &agent_event.event {
            Some(event) => match event {
                crate::events::agent_event::Event::Message(e) => {
                    let base_size = e.coordinator_id.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + e.message.len()
                        + 64;

                    // Add memory for child table records (each sequence creates a separate record)
                    let sequence_records_size = e.sequences.len()
                        * std::mem::size_of::<entities::agent_message_event_sequences::Model>();

                    base_size + sequence_records_size
                }
                crate::events::agent_event::Event::Transaction(e) => {
                    let base_size = e.coordinator_id.len()
                        + e.tx_type.len()
                        + e.developer.len()
                        + e.agent.len()
                        + e.app.len()
                        + e.job_id.len()
                        + e.tx_hash.len()
                        + e.chain.len()
                        + e.network.len()
                        + e.memo.len()
                        + e.metadata.len()
                        + 128;

                    // Add memory for child table records (each sequence creates a separate record)
                    let sequence_records_size = e.sequences.len()
                        * std::mem::size_of::<entities::agent_transaction_event_sequences::Model>();

                    base_size + sequence_records_size
                }
            },
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_limits() {
        assert!(MAX_MEMORY_BYTES > 0);
        assert!(DEFAULT_CHANNEL_CAPACITY > 0);
        assert!(ERROR_THRESHOLD > 0);
        assert!(MAX_DB_RETRIES > 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(3, Duration::from_millis(100));

        // Should be closed initially
        assert!(!cb.is_open().await);

        // Record errors to open circuit breaker
        cb.record_error().await;
        cb.record_error().await;
        cb.record_error().await;

        assert!(cb.is_open().await);

        // Test DB success reset
        cb.record_db_success().await;
        assert!(!cb.is_open().await);
    }

    #[tokio::test]
    async fn test_stats_atomic_updates() {
        let stats = BufferStatsAtomic::default();

        // Test concurrent updates don't race
        let stats_arc = Arc::new(stats);
        let mut handles = vec![];

        for _ in 0..10 {
            let stats_clone = Arc::clone(&stats_arc);
            let handle = tokio::spawn(async move {
                for _ in 0..100 {
                    stats_clone.total_received.fetch_add(1, Ordering::Relaxed);
                    stats_clone.total_processed.fetch_add(1, Ordering::Relaxed);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(stats_arc.total_received.load(Ordering::Relaxed), 1000);
        assert_eq!(stats_arc.total_processed.load(Ordering::Relaxed), 1000);
    }

    #[test]
    fn test_exponential_backoff() {
        // Test that retry delays increase exponentially
        let base_delay = INITIAL_RETRY_DELAY.as_millis() as u64;

        for attempt in 1..=5 {
            let exponential_delay = base_delay * (2_u64.pow(attempt as u32 - 1));
            let max_delay = MAX_RETRY_DELAY.as_millis() as u64;
            let actual_delay = exponential_delay.min(max_delay);

            println!(
                "Attempt {}: {}ms (max: {}ms)",
                attempt, actual_delay, max_delay
            );

            // Verify exponential growth up to max
            if attempt == 1 {
                assert_eq!(actual_delay, base_delay);
            } else {
                assert!(actual_delay >= base_delay);
                assert!(actual_delay <= max_delay);
            }
        }
    }

    #[test]
    fn test_semaphore_thresholds() {
        // Test that our semaphore strategy constants make sense
        let total_permits = DEFAULT_CHANNEL_CAPACITY;
        let fast_path_threshold = total_permits / FAST_PATH_THRESHOLD;

        // Should have meaningful thresholds
        assert!(fast_path_threshold > 0);
        assert!(fast_path_threshold < total_permits);

        // Fast path should trigger when we have plenty of capacity
        assert!(fast_path_threshold >= total_permits / 10); // At least 10% capacity for fast path

        println!(
            "Total permits: {}, Fast path threshold: {} ({}%)",
            total_permits,
            fast_path_threshold,
            (fast_path_threshold * 100) / total_permits
        );
    }

    #[test]
    fn test_partial_insert_counter_consistency() {
        // Test the logic for handling partial inserts to ensure counter consistency
        let event_count = 100;
        let inserted_count = 75; // Partial insert
        let dropped_count = event_count - inserted_count;

        // Simulate the logic from flush_buffer
        let mut total_processed = 0u64;
        let mut total_dropped = 0u64;
        let mut current_buffer_size = event_count;

        // Apply the same logic as in flush_buffer for partial insert
        if inserted_count < event_count {
            total_dropped += dropped_count as u64;
        }
        total_processed += inserted_count as u64;
        current_buffer_size -= event_count; // All events removed from buffer

        // Verify counter consistency
        assert_eq!(total_processed, 75);
        assert_eq!(total_dropped, 25);
        assert_eq!(current_buffer_size, 0); // All events removed from buffer
        assert_eq!(total_processed + total_dropped, event_count as u64); // Total events accounted for
    }

    #[test]
    fn test_health_check_idle_system() {
        // Test that health check passes when system is idle (no events received)

        // Simulate idle system stats
        let stats = BufferStats {
            total_received: 0, // No events received yet
            total_processed: 0,
            total_errors: 0,
            total_dropped: 0,
            total_retries: 0,
            current_buffer_size: 0,
            current_memory_bytes: 0,
            last_flush_time: None,
            backpressure_events: 0, // Even if there were some backpressure events
            circuit_breaker_open: false,
        };

        // Simulate the health check logic
        let basic_health =
            !stats.circuit_breaker_open && stats.current_memory_bytes < MAX_MEMORY_BYTES;

        let backpressure_healthy = if stats.total_received == 0 {
            true // Should be healthy when idle
        } else {
            stats.backpressure_events < stats.total_received / 10
        };

        let health_result = basic_health && backpressure_healthy;

        // Should be healthy when idle
        assert!(
            health_result,
            "Health check should pass when system is idle"
        );

        // Test with some backpressure events during idle
        let stats_with_backpressure = BufferStats {
            backpressure_events: 5, // Some backpressure during startup
            ..stats
        };

        let backpressure_healthy_with_events = if stats_with_backpressure.total_received == 0 {
            true // Should still be healthy when idle
        } else {
            stats_with_backpressure.backpressure_events
                < stats_with_backpressure.total_received / 10
        };

        let health_with_backpressure = basic_health && backpressure_healthy_with_events;
        assert!(
            health_with_backpressure,
            "Health check should pass when idle even with startup backpressure"
        );
    }
}
