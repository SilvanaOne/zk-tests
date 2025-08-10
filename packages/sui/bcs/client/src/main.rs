mod bcs;
mod deserialize;

use deserialize::UserStateEvent;
use dotenvy::dotenv;
use move_core_types::u256::U256;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    // Check if SUI_KEY environment variable is set
    if env::var("SUI_KEY").is_err() {
        eprintln!("Error: SUI_KEY environment variable not set");
        eprintln!("Please set it to your private key hex string");
        eprintln!("Example: export SUI_KEY=\"suiprivkey1q...\"");
        return Ok(());
    }

    println!("BCS Move Client Example");
    println!("======================");

    // Example 1: Create a new user
    println!("\n1. Creating a new user...");
    let user_name = "Alice".to_string();
    let initial_data = U256::from(42u64);
    let signature = vec![1, 2, 3, 4]; // dummy signature for testing

    match bcs::create_user(user_name.clone(), initial_data, signature.clone()).await {
        Ok(object_id) => {
            println!("✅ User created successfully!");
            println!("   Shared Object ID: {}", object_id);

            // Example 2: Make a request to the user
            println!("\n2. Making a request to the user...");
            let new_name = "Alice Updated".to_string();
            let new_data = U256::from(100u64);
            let new_signature = vec![5, 6, 7, 8]; // new signature for testing

            match bcs::user_request_1(object_id.clone(), new_name.clone(), new_data, new_signature.clone()).await {
                Ok((tx_digest, event_data)) => {
                    println!("✅ User request processed successfully!");
                    println!("   Transaction Digest: {}", tx_digest);
                    
                    // Example 2b: Test deserialization from user_request_1 event
                    if let Some(event_json) = event_data {
                        println!("\n2b. Testing deserialization from user_request_1 event...");
                        match UserStateEvent::from_json_event(&event_json) {
                            Ok(event) => {
                                println!("   ✅ Successfully parsed UserStateEvent from transaction!");
                                println!("   Event parameters:");
                                println!("   - Name: {}", event.name);
                                println!("   - Data: {}", event.data);
                                println!("   - Signature: {:?}", event.signature);
                                println!("   - Sequence: {}", event.sequence);
                                println!("   - Serialized state: {} bytes", event.serialized_state.len());
                                
                                // Deserialize the state
                                match event.deserialize_state() {
                                    Ok(deserialized) => {
                                        println!("\n   ✅ Successfully deserialized UserStateData!");
                                        println!("   Deserialized values:");
                                        println!("   - Name: {}", deserialized.name);
                                        println!("   - Data: {}", deserialized.data);
                                        println!("   - Signature: {:?}", deserialized.signature);
                                        println!("   - Sequence: {}", deserialized.sequence);
                                        
                                        // Verify consistency
                                        match event.verify_state_consistency() {
                                            Ok(is_consistent) => {
                                                if is_consistent {
                                                    println!("   ✅ State consistency verified! Event parameters match deserialized state.");
                                                } else {
                                                    println!("   ⚠️ State inconsistency detected!");
                                                }
                                            }
                                            Err(e) => println!("   ❌ Error verifying consistency: {}", e),
                                        }
                                    }
                                    Err(e) => println!("   ❌ Error deserializing state: {}", e),
                                }
                            }
                            Err(e) => println!("   ❌ Error parsing event: {}", e),
                        }
                    }

                    // Example 3: Test user_request_2 with BCS serialization
                    println!("\n3. Testing user_request_2 with BCS serialization...");
                    let (bcs_digest, event_data_2) = bcs::user_request_2_with_bcs(
                        object_id.clone(),
                        "UserRequest BCS Test".to_string(),
                        U256::from(300u64),
                        vec![13, 14, 15, 16],
                    )
                    .await?;
                    println!("✅ user_request_2 with BCS completed successfully!");
                    println!("   Transaction digest: {}", bcs_digest);

                    // Example 4: Test deserialization from user_request_2 event
                    if let Some(event_json_2) = event_data_2 {
                        println!("\n4. Testing deserialization from user_request_2 event...");
                        match UserStateEvent::from_json_event(&event_json_2) {
                            Ok(event) => {
                                println!("   ✅ Successfully parsed UserStateEvent from user_request_2!");
                                println!("   Event parameters:");
                                println!("   - Name: {}", event.name);
                                println!("   - Data: {}", event.data);
                                println!("   - Signature: {:?}", event.signature);
                                println!("   - Sequence: {}", event.sequence);
                                println!("   - Serialized state: {} bytes", event.serialized_state.len());
                                
                                // Deserialize the state from the actual Move event
                                match event.deserialize_state() {
                                    Ok(deserialized) => {
                                        println!("\n   ✅ Successfully deserialized UserStateData from Move event!");
                                        println!("   Deserialized values:");
                                        println!("   - Name: {}", deserialized.name);
                                        println!("   - Data: {}", deserialized.data);
                                        println!("   - Signature: {:?}", deserialized.signature);
                                        println!("   - Sequence: {}", deserialized.sequence);
                                        
                                        // Verify consistency
                                        match event.verify_state_consistency() {
                                            Ok(is_consistent) => {
                                                if is_consistent {
                                                    println!("\n   ✅ State consistency verified!");
                                                    println!("   The deserialized UserStateData matches the event parameters exactly.");
                                                } else {
                                                    println!("\n   ⚠️ State inconsistency detected!");
                                                    println!("   The deserialized UserStateData does not match the event parameters.");
                                                }
                                            }
                                            Err(e) => println!("   ❌ Error verifying consistency: {}", e),
                                        }
                                        
                                        // Manual verification against expected values
                                        assert_eq!(event.name, "UserRequest BCS Test", "Event name should match");
                                        assert_eq!(event.data, U256::from(300u64), "Event data should match");
                                        assert_eq!(event.signature, vec![13, 14, 15, 16], "Event signature should match");
                                        
                                        // Verify deserialized matches event
                                        assert_eq!(deserialized.name, event.name, "Deserialized name should match event");
                                        assert_eq!(deserialized.data, event.data, "Deserialized data should match event");
                                        assert_eq!(deserialized.signature, event.signature, "Deserialized signature should match event");
                                        assert_eq!(deserialized.sequence, event.sequence, "Deserialized sequence should match event");
                                        
                                        println!("   ✅ All assertions passed! Data from Move event successfully deserialized.");
                                    }
                                    Err(e) => println!("   ❌ Error deserializing state: {}", e),
                                }
                            }
                            Err(e) => println!("   ❌ Error parsing event: {}", e),
                        }
                    } else {
                        println!("\n4. No event data received from user_request_2");
                    }

                }
                Err(e) => println!("❌ Error making user request: {}", e),
            }
        }
        Err(e) => println!("❌ Error creating user: {}", e),
    }

    println!("\n🎉 Example completed!");
    Ok(())
}
