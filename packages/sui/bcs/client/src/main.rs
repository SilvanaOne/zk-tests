mod bcs;

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

            match bcs::user_request_1(object_id.clone(), new_name, new_data, new_signature).await {
                Ok(tx_digest) => {
                    println!("✅ User request processed successfully!");
                    println!("   Transaction Digest: {}", tx_digest);

                    // Example 3: Test user_request_2 with BCS serialization
                    println!("\n3. Testing user_request_2 with BCS serialization...");
                    let bcs_digest = bcs::user_request_2_with_bcs(
                        object_id.clone(),
                        "UserRequest BCS Test".to_string(),
                        U256::from(300u64),
                        vec![13, 14, 15, 16],
                    )
                    .await?;
                    println!("✅ user_request_2 with BCS completed successfully!");
                    println!("   Transaction digest: {}", bcs_digest);
                }
                Err(e) => println!("❌ Error making user request: {}", e),
            }
        }
        Err(e) => println!("❌ Error creating user: {}", e),
    }

    println!("\n🎉 Example completed!");
    Ok(())
}
