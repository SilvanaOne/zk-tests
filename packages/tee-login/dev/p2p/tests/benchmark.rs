use rand::Rng;
use std::time::Instant;
use tokio;
use tonic::Request;

// Include the generated protobuf code
pub mod kv {
    tonic::include_proto!("kv");
}

use kv::kv_client::KvClient;
use kv::{GetRequest, PutRequest};

const NUM_OPERATIONS: usize = 100000;
const SERVER_ADDR: &str = "http://127.0.0.1:50051";

#[tokio::test]
async fn benchmark_kv_operations() {
    println!("Starting benchmark test...");
    println!("Connecting to server at {}", SERVER_ADDR);

    // Connect to the running service
    let mut client = KvClient::connect(SERVER_ADDR)
        .await
        .expect("Failed to connect to server. Make sure the server is running with: cargo run --release -- 127.0.0.1:50051");

    println!("Connected successfully!");

    // Generate random test data
    let mut rng = rand::thread_rng();
    let mut test_keys = Vec::with_capacity(NUM_OPERATIONS);
    let mut test_values = Vec::with_capacity(NUM_OPERATIONS);

    println!("Generating {} random key-value pairs...", NUM_OPERATIONS);
    for i in 0..NUM_OPERATIONS {
        // Generate random key (8-16 characters)
        let key_len = rng.r#gen_range(8..=16);
        let mut key_suffix = String::with_capacity(key_len);
        for _ in 0..key_len {
            key_suffix.push(rng.r#gen_range(b'a'..=b'z') as char);
        }
        let key = format!("test_key_{}_{}", i, key_suffix);

        // Generate random value (100-1000 bytes)
        let value_len = rng.r#gen_range(100..=1000);
        let mut value = Vec::with_capacity(value_len);
        for _ in 0..value_len {
            value.push(rng.r#gen::<u8>());
        }

        test_keys.push(key);
        test_values.push(value);
    }

    println!("Starting PUT operations benchmark...");

    // Benchmark PUT operations
    let put_start = Instant::now();
    for i in 0..NUM_OPERATIONS {
        let request = Request::new(PutRequest {
            key: test_keys[i].clone(),
            value: test_values[i].clone(),
        });

        client
            .put(request)
            .await
            .expect(&format!("PUT operation {} failed", i));

        // // Print progress every 1000 operations
        // if (i + 1) % 1000 == 0 {
        //     println!("Completed {} PUT operations", i + 1);
        // }
    }
    let put_duration = put_start.elapsed();

    println!("Preparing GET keys...");

    // Prepare all GET keys in advance (mix of existing and non-existing keys)
    let mut get_keys = Vec::with_capacity(NUM_OPERATIONS);
    for i in 0..NUM_OPERATIONS {
        let key = if i < NUM_OPERATIONS / 2 {
            // First half: get existing keys
            test_keys[i * 2 % test_keys.len()].clone()
        } else {
            // Second half: try some non-existing keys
            format!("non_existing_key_{}", i)
        };
        get_keys.push(key);
    }

    println!("Starting GET operations benchmark...");

    // Benchmark GET operations
    let get_start = Instant::now();
    let mut found_count = 0;
    let mut not_found_count = 0;

    for i in 0..NUM_OPERATIONS {
        let request = Request::new(GetRequest {
            key: get_keys[i].clone(),
        });
        let response = client
            .get(request)
            .await
            .expect(&format!("GET operation {} failed", i));

        if response.into_inner().found {
            found_count += 1;
        } else {
            not_found_count += 1;
        }

        // // Print progress every 1_000_000 operations
        // if (i + 1) % 1_000_000 == 0 {
        //     println!("Completed {} GET operations", i + 1);
        // }
    }
    let get_duration = get_start.elapsed();

    // Print results
    println!("\n=== BENCHMARK RESULTS ===");
    println!("PUT Operations:");
    println!("  Total operations: {}", NUM_OPERATIONS);
    println!("  Total time: {:?}", put_duration);
    println!(
        "  Average time per PUT: {:?}",
        put_duration / NUM_OPERATIONS as u32
    );
    println!(
        "  Operations per second: {:.2}",
        NUM_OPERATIONS as f64 / put_duration.as_secs_f64()
    );

    println!("\nGET Operations:");
    println!("  Total operations: {}", NUM_OPERATIONS);
    println!("  Total time: {:?}", get_duration);
    println!(
        "  Average time per GET: {:?}",
        get_duration / NUM_OPERATIONS as u32
    );
    println!(
        "  Operations per second: {:.2}",
        NUM_OPERATIONS as f64 / get_duration.as_secs_f64()
    );
    println!("  Keys found: {}", found_count);
    println!("  Keys not found: {}", not_found_count);

    println!("\nCombined Performance:");
    let total_operations = NUM_OPERATIONS * 2;
    let total_time = put_duration + get_duration;
    println!("  Total operations: {}", total_operations);
    println!("  Total time: {:?}", total_time);
    println!(
        "  Average time per operation: {:?}",
        total_time / total_operations as u32
    );
    println!(
        "  Operations per second: {:.2}",
        total_operations as f64 / total_time.as_secs_f64()
    );

    println!("\nBenchmark completed successfully!");
}
