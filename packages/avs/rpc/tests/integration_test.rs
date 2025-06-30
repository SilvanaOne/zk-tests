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
const SERVER_ADDR: &str = "http://18.194.39.156:50051"; //"http://127.0.0.1:50051";
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

    // Clone clients for parallel execution
    let mut client1 = client.clone();
    let mut client2 = client.clone();
    let mut client3 = client.clone();

    println!("\n🚀 Running tests in parallel...");
    
    // Run all tests concurrently
    let (coordinator_result, agent_result, batch_result) = tokio::join!(
        async {
            println!("📋 Starting Coordinator Events...");
            test_coordinator_events(&mut client1).await
        },
        async {
            println!("🤖 Starting Agent Events...");
            test_agent_events(&mut client2).await
        },
        async {
            println!("📦 Starting Batch Submission...");
            test_batch_events(&mut client3).await
        }
    );

    println!("✅ All parallel tests completed!");

    let duration = start_time.elapsed();
    let duration_ms = duration.as_millis();

    // Calculate total events dispatched (sent concurrently)
    let coordinator_event_types = 6; // 6 different coordinator event types
    let agent_event_types = 3; // 3 different agent event types
    let total_events =
        (coordinator_event_types * NUM_EVENTS) + (agent_event_types * NUM_EVENTS) + NUM_EVENTS;
    let events_per_second = if duration.as_secs_f64() > 0.0 {
        total_events as f64 / duration.as_secs_f64()
    } else {
        0.0
    };

    println!("\n🎉 All parallel integration tests completed successfully!");
    println!(
        "📊 Total events dispatched concurrently: {} events ({} coordinator + {} agent + {} batch)",
        total_events,
        coordinator_event_types * NUM_EVENTS,
        agent_event_types * NUM_EVENTS,
        NUM_EVENTS
    );
    println!(
        "⏱️  Total parallel execution time: {}ms ({:.2}s)",
        duration_ms,
        duration.as_secs_f64()
    );
    println!("🚀 Concurrent throughput: {:.1} events/second", events_per_second);
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

    println!("  🚀 Sending {} event types concurrently with {} events each", test_cases.len(), NUM_EVENTS);

    // Run all event types in parallel
    let handles: Vec<_> = test_cases.into_iter().map(|(event_type, event)| {
        let mut client_clone = client.clone();
        let event_type = event_type.to_string();
        
        tokio::spawn(async move {
            println!("  📤 Starting {} events of type: {}", NUM_EVENTS, event_type);
            
            // Send events in chunks of 100 concurrently for each type
            const CHUNK_SIZE: usize = 100;
            let chunks: Vec<_> = (1..=NUM_EVENTS).collect::<Vec<_>>().chunks(CHUNK_SIZE).map(|chunk| chunk.to_vec()).collect();
            
            for chunk in chunks {
                let chunk_handles: Vec<_> = chunk.into_iter().map(|i| {
                    let mut test_event = event.clone();
                    modify_coordinator_event_for_uniqueness(&mut test_event, i);
                    let mut client_clone2 = client_clone.clone();
                    let event_type_clone = event_type.clone();
                    
                    tokio::spawn(async move {
                        let request = Request::new(test_event);
                        match client_clone2.submit_event(request).await {
                            Ok(response) => {
                                let resp = response.into_inner();
                                if !resp.success {
                                    println!("    ⚠️  {} Event {}: {}", event_type_clone, i, resp.message);
                                }
                                assert!(
                                    resp.processed_count == 1,
                                    "Expected 1 processed event, got {}",
                                    resp.processed_count
                                );
                                Ok(())
                            }
                            Err(e) => {
                                Err(format!("❌ Failed to send {} event {}: {}", event_type_clone, i, e))
                            }
                        }
                    })
                }).collect();
                
                // Wait for this chunk to complete
                for handle in chunk_handles {
                    if let Err(e) = handle.await.unwrap() {
                        panic!("{}", e);
                    }
                }
            }
            
            println!("  ✅ Successfully sent {} {} events", NUM_EVENTS, event_type);
            event_type
        })
    }).collect();

    // Wait for all event types to complete
    for handle in handles {
        handle.await.unwrap();
    }

    let duration = start_time.elapsed();
    println!(
        "  ⏱️  Coordinator events duration: {}ms (parallel execution)",
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

    println!("  🚀 Sending {} agent event types concurrently with {} events each", test_cases.len(), NUM_EVENTS);

    // Run all event types in parallel
    let handles: Vec<_> = test_cases.into_iter().map(|(event_type, event)| {
        let mut client_clone = client.clone();
        let event_type = event_type.to_string();
        
        tokio::spawn(async move {
            println!("  📤 Starting {} events of type: {}", NUM_EVENTS, event_type);
            
            // Send events in chunks of 100 concurrently for each type
            const CHUNK_SIZE: usize = 100;
            let chunks: Vec<_> = (1..=NUM_EVENTS).collect::<Vec<_>>().chunks(CHUNK_SIZE).map(|chunk| chunk.to_vec()).collect();
            
            for chunk in chunks {
                let chunk_handles: Vec<_> = chunk.into_iter().map(|i| {
                    let mut test_event = event.clone();
                    modify_agent_event_for_uniqueness(&mut test_event, i);
                    let mut client_clone2 = client_clone.clone();
                    let event_type_clone = event_type.clone();
                    
                    tokio::spawn(async move {
                        let request = Request::new(test_event);
                        match client_clone2.submit_event(request).await {
                            Ok(response) => {
                                let resp = response.into_inner();
                                if !resp.success {
                                    println!("    ⚠️  {} Event {}: {}", event_type_clone, i, resp.message);
                                }
                                assert!(
                                    resp.processed_count == 1,
                                    "Expected 1 processed event, got {}",
                                    resp.processed_count
                                );
                                Ok(())
                            }
                            Err(e) => {
                                Err(format!("❌ Failed to send {} event {}: {}", event_type_clone, i, e))
                            }
                        }
                    })
                }).collect();
                
                // Wait for this chunk to complete
                for handle in chunk_handles {
                    if let Err(e) = handle.await.unwrap() {
                        panic!("{}", e);
                    }
                }
            }
            
            println!("  ✅ Successfully sent {} {} events", NUM_EVENTS, event_type);
            event_type
        })
    }).collect();

    // Wait for all event types to complete
    for handle in handles {
        handle.await.unwrap();
    }

    let duration = start_time.elapsed();
    println!("  ⏱️  Agent events duration: {}ms (parallel execution)", duration.as_millis());
}

async fn test_batch_events(client: &mut SilvanaEventsServiceClient<tonic::transport::Channel>) {
    let start_time = Instant::now();

    // Split into multiple concurrent batches
    const BATCH_SIZE: usize = 1000; // Send batches of 1000 events each
    let num_batches = (NUM_EVENTS + BATCH_SIZE - 1) / BATCH_SIZE;
    
    println!("  🚀 Creating {} concurrent batches of ~{} mixed events each (total: {})", 
             num_batches, BATCH_SIZE, NUM_EVENTS);

    // Create concurrent batch handles
    let handles: Vec<_> = (0..num_batches).map(|batch_idx| {
        let mut client_clone = client.clone();
        
        tokio::spawn(async move {
            let start_event_idx = batch_idx * BATCH_SIZE + 1;
            let end_event_idx = std::cmp::min((batch_idx + 1) * BATCH_SIZE, NUM_EVENTS);
            let batch_event_count = end_event_idx - start_event_idx + 1;
            
            println!("  📦 Creating batch {} with {} events (events {}-{})", 
                     batch_idx + 1, batch_event_count, start_event_idx, end_event_idx);

            let mut events = Vec::new();

            for i in start_event_idx..=end_event_idx {
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

            match client_clone.submit_events(request).await {
                Ok(response) => {
                    let resp = response.into_inner();
                    println!(
                        "  📊 Batch {} result: {} - Processed: {}/{}",
                        batch_idx + 1, resp.message, resp.processed_count, batch_event_count
                    );

                    if !resp.success {
                        println!("    ⚠️  Batch {} had some failures: {}", batch_idx + 1, resp.message);
                    }

                    // Should have processed all events or failed gracefully
                    assert!(
                        resp.processed_count <= batch_event_count as u32,
                        "Batch {} processed count {} exceeds sent count {}",
                        batch_idx + 1,
                        resp.processed_count,
                        batch_event_count
                    );

                    Ok((batch_idx + 1, resp.processed_count as usize, batch_event_count))
                }
                Err(e) => {
                    Err(format!("❌ Failed to send batch {} events: {}", batch_idx + 1, e))
                }
            }
        })
    }).collect();

    // Wait for all batches to complete and collect results
    let mut total_processed = 0;
    let mut total_sent = 0;
    
    for handle in handles {
        match handle.await.unwrap() {
            Ok((batch_num, processed, sent)) => {
                total_processed += processed;
                total_sent += sent;
                println!("  ✅ Batch {} completed: {}/{} events", batch_num, processed, sent);
            }
            Err(e) => panic!("{}", e),
        }
    }

    let duration = start_time.elapsed();
    println!("  🎉 Successfully sent {} concurrent batches totaling {}/{} events", 
             num_batches, total_processed, total_sent);
    println!("  ⏱️  Concurrent batch duration: {}ms", duration.as_millis());
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
