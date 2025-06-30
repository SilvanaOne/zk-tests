use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tonic::Request;

// Import the generated protobuf code
mod events {
    tonic::include_proto!("silvana.events");
}

use events::silvana_events_service_client::SilvanaEventsServiceClient;
use events::*;

// Configuration - easily changeable parameters
const NUM_EVENTS: usize = 10000;
const SERVER_ADDR: &str = "http://127.0.0.1:50051";
const COORDINATOR_ID: &str = "test-coordinator-001";

#[tokio::test]
async fn test_send_coordinator_and_agent_events() {
    let start_time = Instant::now();

    println!("🧪 Starting integration test...");
    println!("📊 Configuration: {} events per type", NUM_EVENTS);
    println!("🎯 Server address: {}", SERVER_ADDR);

    // Connect to the gRPC server
    let mut client = match SilvanaEventsServiceClient::connect(SERVER_ADDR).await {
        Ok(client) => {
            println!("✅ Connected to RPC server successfully");
            client
        }
        Err(e) => {
            panic!("❌ Failed to connect to RPC server at {}: {}\nMake sure the server is running with: cargo run", SERVER_ADDR, e);
        }
    };

    // Test coordinator events
    println!("\n📋 Testing Coordinator Events...");
    test_coordinator_events(&mut client).await;

    // Test agent events
    println!("\n🤖 Testing Agent Events...");
    test_agent_events(&mut client).await;

    // Test batch submission
    println!("\n📦 Testing Batch Submission...");
    test_batch_events(&mut client).await;

    let duration = start_time.elapsed();
    let duration_ms = duration.as_millis();

    // Calculate total events dispatched
    let coordinator_event_types = 6; // 6 different coordinator event types
    let agent_event_types = 3; // 3 different agent event types
    let total_events =
        (coordinator_event_types * NUM_EVENTS) + (agent_event_types * NUM_EVENTS) + NUM_EVENTS;
    let events_per_second = if duration.as_secs_f64() > 0.0 {
        total_events as f64 / duration.as_secs_f64()
    } else {
        0.0
    };

    println!("\n🎉 All integration tests completed successfully!");
    println!(
        "📊 Total events dispatched: {} events ({} coordinator + {} agent + {} batch)",
        total_events,
        coordinator_event_types * NUM_EVENTS,
        agent_event_types * NUM_EVENTS,
        NUM_EVENTS
    );
    println!(
        "⏱️  Total test duration: {}ms ({:.2}s)",
        duration_ms,
        duration.as_secs_f64()
    );
    println!("🚀 Throughput: {:.1} events/second", events_per_second);
}

async fn test_coordinator_events(
    client: &mut SilvanaEventsServiceClient<tonic::transport::Channel>,
) {
    let start_time = Instant::now();

    let test_cases = vec![
        ("coordinator_started", create_coordinator_started_event()),
        ("agent_started_job", create_agent_started_job_event()),
        ("agent_finished_job", create_agent_finished_job_event()),
        ("coordination_tx", create_coordination_tx_event()),
        ("coordinator_error", create_coordinator_error_event()),
        ("client_transaction", create_client_transaction_event()),
    ];

    for (event_type, event) in test_cases {
        println!("  📤 Sending {} events of type: {}", NUM_EVENTS, event_type);

        for i in 1..=NUM_EVENTS {
            let mut test_event = event.clone();

            // Add variation to make events unique
            modify_coordinator_event_for_uniqueness(&mut test_event, i);

            let request = Request::new(test_event);

            match client.submit_event(request).await {
                Ok(response) => {
                    let resp = response.into_inner();
                    if !resp.success {
                        println!("    ⚠️  Event {}: {}", i, resp.message);
                    }
                    assert!(
                        resp.processed_count == 1,
                        "Expected 1 processed event, got {}",
                        resp.processed_count
                    );
                }
                Err(e) => {
                    panic!("❌ Failed to send {} event {}: {}", event_type, i, e);
                }
            }
        }

        println!(
            "  ✅ Successfully sent {} {} events",
            NUM_EVENTS, event_type
        );
    }

    let duration = start_time.elapsed();
    println!(
        "  ⏱️  Coordinator events duration: {}ms",
        duration.as_millis()
    );
}

async fn test_agent_events(client: &mut SilvanaEventsServiceClient<tonic::transport::Channel>) {
    let start_time = Instant::now();

    let test_cases = vec![
        ("agent_message", create_agent_message_event()),
        ("agent_error", create_agent_error_event()),
        ("agent_transaction", create_agent_transaction_event()),
    ];

    for (event_type, event) in test_cases {
        println!("  📤 Sending {} events of type: {}", NUM_EVENTS, event_type);

        for i in 1..=NUM_EVENTS {
            let mut test_event = event.clone();

            // Add variation to make events unique
            modify_agent_event_for_uniqueness(&mut test_event, i);

            let request = Request::new(test_event);

            match client.submit_event(request).await {
                Ok(response) => {
                    let resp = response.into_inner();
                    if !resp.success {
                        println!("    ⚠️  Event {}: {}", i, resp.message);
                    }
                    assert!(
                        resp.processed_count == 1,
                        "Expected 1 processed event, got {}",
                        resp.processed_count
                    );
                }
                Err(e) => {
                    panic!("❌ Failed to send {} event {}: {}", event_type, i, e);
                }
            }
        }

        println!(
            "  ✅ Successfully sent {} {} events",
            NUM_EVENTS, event_type
        );
    }

    let duration = start_time.elapsed();
    println!("  ⏱️  Agent events duration: {}ms", duration.as_millis());
}

async fn test_batch_events(client: &mut SilvanaEventsServiceClient<tonic::transport::Channel>) {
    let start_time = Instant::now();

    println!("  📦 Creating batch of {} mixed events", NUM_EVENTS);

    let mut events = Vec::new();

    for i in 1..=NUM_EVENTS {
        // Alternate between coordinator and agent events
        let event = if i % 2 == 0 {
            let mut event = create_coordinator_started_event();
            modify_coordinator_event_for_uniqueness(&mut event, i);
            event
        } else {
            let mut event = create_agent_message_event();
            modify_agent_event_for_uniqueness(&mut event, i);
            event
        };

        events.push(event);
    }

    let request = Request::new(SubmitEventsRequest { events });

    match client.submit_events(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            println!(
                "  📊 Batch result: {} - Processed: {}",
                resp.message, resp.processed_count
            );

            if !resp.success {
                println!("    ⚠️  Batch had some failures: {}", resp.message);
            }

            // Should have processed all events or failed gracefully
            assert!(
                resp.processed_count <= NUM_EVENTS as u32,
                "Processed count {} exceeds sent count {}",
                resp.processed_count,
                NUM_EVENTS
            );
        }
        Err(e) => {
            panic!("❌ Failed to send batch events: {}", e);
        }
    }

    let duration = start_time.elapsed();
    println!("  ✅ Successfully sent batch of {} events", NUM_EVENTS);
    println!("  ⏱️  Batch events duration: {}ms", duration.as_millis());
}

// Helper functions to create different event types

fn create_coordinator_started_event() -> Event {
    Event {
        event_type: Some(event::EventType::Coordinator(CoordinatorEvent {
            event: Some(coordinator_event::Event::CoordinatorStarted(
                CoordinatorStartedEvent {
                    coordinator_id: COORDINATOR_ID.to_string(),
                    ethereum_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
                    sui_ed25519_address: "0xabcdef1234567890abcdef1234567890abcdef12".to_string(),
                    timestamp: get_current_timestamp(),
                },
            )),
        })),
    }
}

fn create_agent_started_job_event() -> Event {
    Event {
        event_type: Some(event::EventType::Coordinator(CoordinatorEvent {
            event: Some(coordinator_event::Event::AgentStartedJob(
                AgentStartedJobEvent {
                    coordinator_id: COORDINATOR_ID.to_string(),
                    developer: "test-developer".to_string(),
                    agent: "test-agent".to_string(),
                    app: "test-app".to_string(),
                    job_id: "job-123".to_string(),
                    timestamp: get_current_timestamp(),
                },
            )),
        })),
    }
}

fn create_agent_finished_job_event() -> Event {
    Event {
        event_type: Some(event::EventType::Coordinator(CoordinatorEvent {
            event: Some(coordinator_event::Event::AgentFinishedJob(
                AgentFinishedJobEvent {
                    coordinator_id: COORDINATOR_ID.to_string(),
                    developer: "test-developer".to_string(),
                    agent: "test-agent".to_string(),
                    app: "test-app".to_string(),
                    job_id: "job-123".to_string(),
                    duration: 5000, // 5 seconds
                    timestamp: get_current_timestamp(),
                },
            )),
        })),
    }
}

fn create_coordination_tx_event() -> Event {
    Event {
        event_type: Some(event::EventType::Coordinator(CoordinatorEvent {
            event: Some(coordinator_event::Event::CoordinationTx(
                CoordinationTxEvent {
                    coordinator_id: COORDINATOR_ID.to_string(),
                    developer: "test-developer".to_string(),
                    agent: "test-agent".to_string(),
                    app: "test-app".to_string(),
                    job_id: "job-123".to_string(),
                    memo: "Test coordination transaction".to_string(),
                    tx_hash: "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                        .to_string(),
                    timestamp: get_current_timestamp(),
                },
            )),
        })),
    }
}

fn create_coordinator_error_event() -> Event {
    Event {
        event_type: Some(event::EventType::Coordinator(CoordinatorEvent {
            event: Some(coordinator_event::Event::CoordinatorError(
                CoordinatorErrorEvent {
                    coordinator_id: COORDINATOR_ID.to_string(),
                    error: "Test error message".to_string(),
                    timestamp: get_current_timestamp(),
                },
            )),
        })),
    }
}

fn create_client_transaction_event() -> Event {
    Event {
        event_type: Some(event::EventType::Coordinator(CoordinatorEvent {
            event: Some(coordinator_event::Event::ClientTransaction(
                ClientTransactionEvent {
                    coordinator_id: COORDINATOR_ID.to_string(),
                    developer: "test-developer".to_string(),
                    agent: "test-agent".to_string(),
                    app: "test-app".to_string(),
                    client_ip_address: "192.168.1.100".to_string(),
                    method: "submit_proof".to_string(),
                    data: b"test transaction data".to_vec(),
                    tx_hash: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                        .to_string(),
                    sequence: 1,
                    timestamp: get_current_timestamp(),
                },
            )),
        })),
    }
}

fn create_agent_message_event() -> Event {
    Event {
        event_type: Some(event::EventType::Agent(AgentEvent {
            event: Some(agent_event::Event::Message(AgentMessageEvent {
                coordinator_id: COORDINATOR_ID.to_string(),
                r#type: "message".to_string(),
                developer: "test-developer".to_string(),
                agent: "test-agent".to_string(),
                app: "test-app".to_string(),
                job_id: "job-123".to_string(),
                sequences: vec![1, 2, 3],
                timestamp: get_current_timestamp(),
                message: "Test agent message".to_string(),
            })),
        })),
    }
}

fn create_agent_error_event() -> Event {
    Event {
        event_type: Some(event::EventType::Agent(AgentEvent {
            event: Some(agent_event::Event::Error(AgentErrorEvent {
                coordinator_id: COORDINATOR_ID.to_string(),
                r#type: "error".to_string(),
                developer: "test-developer".to_string(),
                agent: "test-agent".to_string(),
                app: "test-app".to_string(),
                job_id: "job-123".to_string(),
                sequences: vec![1, 2, 3],
                timestamp: get_current_timestamp(),
                error: "Test agent error".to_string(),
            })),
        })),
    }
}

fn create_agent_transaction_event() -> Event {
    Event {
        event_type: Some(event::EventType::Agent(AgentEvent {
            event: Some(agent_event::Event::Transaction(AgentTransactionEvent {
                coordinator_id: COORDINATOR_ID.to_string(),
                r#type: "transaction".to_string(),
                developer: "test-developer".to_string(),
                agent: "test-agent".to_string(),
                app: "test-app".to_string(),
                job_id: "job-123".to_string(),
                sequences: vec![1, 2, 3],
                timestamp: get_current_timestamp(),
                tx_hash: "0x9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba"
                    .to_string(),
                chain: "ethereum".to_string(),
                network: "mainnet".to_string(),
                tx_type: "contract_call".to_string(),
                memo: "Test agent transaction".to_string(),
                metadata: r#"{"gas_used": 21000, "gas_price": "20000000000"}"#.to_string(),
            })),
        })),
    }
}

// Helper functions to add uniqueness to events

fn modify_coordinator_event_for_uniqueness(event: &mut Event, index: usize) {
    if let Some(event::EventType::Coordinator(ref mut coord_event)) = event.event_type {
        match &mut coord_event.event {
            Some(coordinator_event::Event::CoordinatorStarted(ref mut e)) => {
                e.ethereum_address = format!("0x{:040x}", index);
                e.timestamp = get_current_timestamp() + index as u64;
            }
            Some(coordinator_event::Event::AgentStartedJob(ref mut e)) => {
                e.job_id = format!("job-{}", index);
                e.timestamp = get_current_timestamp() + index as u64;
            }
            Some(coordinator_event::Event::AgentFinishedJob(ref mut e)) => {
                e.job_id = format!("job-{}", index);
                e.duration = 1000 + (index as u64 * 100);
                e.timestamp = get_current_timestamp() + index as u64;
            }
            Some(coordinator_event::Event::CoordinationTx(ref mut e)) => {
                e.job_id = format!("job-{}", index);
                e.tx_hash = format!("0x{:064x}", index);
                e.timestamp = get_current_timestamp() + index as u64;
            }
            Some(coordinator_event::Event::CoordinatorError(ref mut e)) => {
                e.error = format!("Test error message #{}", index);
                e.timestamp = get_current_timestamp() + index as u64;
            }
            Some(coordinator_event::Event::ClientTransaction(ref mut e)) => {
                e.tx_hash = format!("0x{:064x}", index);
                e.sequence = index as u64;
                e.timestamp = get_current_timestamp() + index as u64;
            }
            None => {}
        }
    }
}

fn modify_agent_event_for_uniqueness(event: &mut Event, index: usize) {
    if let Some(event::EventType::Agent(ref mut agent_event)) = event.event_type {
        match &mut agent_event.event {
            Some(agent_event::Event::Message(ref mut e)) => {
                e.job_id = format!("job-{}", index);
                e.message = format!("Test agent message #{}", index);
                e.sequences = vec![index as u64];
                e.timestamp = get_current_timestamp() + index as u64;
            }
            Some(agent_event::Event::Error(ref mut e)) => {
                e.job_id = format!("job-{}", index);
                e.error = format!("Test agent error #{}", index);
                e.sequences = vec![index as u64];
                e.timestamp = get_current_timestamp() + index as u64;
            }
            Some(agent_event::Event::Transaction(ref mut e)) => {
                e.job_id = format!("job-{}", index);
                e.tx_hash = format!("0x{:064x}", index + 1000);
                e.sequences = vec![index as u64];
                e.timestamp = get_current_timestamp() + index as u64;
            }
            None => {}
        }
    }
}

fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
