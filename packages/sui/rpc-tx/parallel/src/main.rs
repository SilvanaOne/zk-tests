mod coin;
mod state;
mod tx;
use std::time::Instant;
use std::sync::Arc;

const NUM_THREADS: usize = 10;

// Validate NUM_THREADS is in acceptable range (1-100)
const _: () = {
    if NUM_THREADS < 1 || NUM_THREADS > 100 {
        panic!("NUM_THREADS must be between 1 and 100");
    }
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::from_path("../.env").ok();
    
    println!("Starting parallel transaction sender with {} threads", NUM_THREADS);
    let overall_start = Instant::now();
    
    // Create shared state once
    println!("Creating shared State...");
    let state_start = Instant::now();
    let state_id = state::create_state().await?;
    let state_elapsed = state_start.elapsed().as_millis();
    println!("State created with id: {} in {} ms", state_id, state_elapsed);
    
    // Prepare transaction tasks for parallel execution
    let tasks = vec![
        (5, 10),   // calculate_sum(5, 10) = 15
        (7, 13),   // calculate_sum(7, 13) = 20
        (2, 8),    // calculate_sum(2, 8) = 10
        (15, 25),  // calculate_sum(15, 25) = 40
        (3, 12),   // calculate_sum(3, 12) = 15
        (9, 21),   // calculate_sum(9, 21) = 30
        (4, 16),   // calculate_sum(4, 16) = 20
        (8, 7),    // calculate_sum(8, 7) = 15
        (11, 14),  // calculate_sum(11, 14) = 25
        (6, 19),   // calculate_sum(6, 19) = 25
        (22, 8),   // calculate_sum(22, 8) = 30
        (13, 17),  // calculate_sum(13, 17) = 30
        (1, 9),    // calculate_sum(1, 9) = 10
        (18, 12),  // calculate_sum(18, 12) = 30
        (5, 5),    // calculate_sum(5, 5) = 10
    ];
    
    let state_id = Arc::new(state_id);
    let mut handles = vec![];
    
    // Spawn parallel threads (limited to NUM_THREADS for testing)
    for (i, (a, b)) in tasks.into_iter().enumerate().take(NUM_THREADS) {
        let state_id = Arc::clone(&state_id);
        
        let handle = tokio::spawn(async move {
            let thread_start = Instant::now();
            println!("[Thread {}] Starting calculate_sum({}, {})", i, a, b);
            
            // Execute calculate_sum transaction
            let sum_result = tx::calculate_sum(a, b).await;
            match sum_result {
                Ok(sum) => {
                    println!("[Thread {}] calculate_sum({}, {}) = {} in {} ms", 
                             i, a, b, sum, thread_start.elapsed().as_millis());
                    
                    // Add the result to the shared state
                    let state_start = Instant::now();
                    match state::add_to_state(*state_id, sum).await {
                        Ok(new_sum) => {
                            println!("[Thread {}] Added {} to state, new sum: {} in {} ms",
                                     i, sum, new_sum, state_start.elapsed().as_millis());
                            Ok((sum, new_sum))
                        }
                        Err(e) => {
                            eprintln!("[Thread {}] Failed to add to state: {}", i, e);
                            Err(e)
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[Thread {}] Failed to calculate sum: {}", i, e);
                    Err(e)
                }
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    let mut successful_results = 0;
    let mut failed_results = 0;
    
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.await? {
            Ok((sum, new_sum)) => {
                println!("[Result {}] Success: sum={}, final_state={}", i, sum, new_sum);
                successful_results += 1;
            }
            Err(e) => {
                eprintln!("[Result {}] Failed: {}", i, e);
                failed_results += 1;
            }
        }
    }
    
    let total_elapsed = overall_start.elapsed().as_millis();
    println!("\n=== Summary ===");
    println!("Total execution time: {} ms", total_elapsed);
    println!("Successful transactions: {}", successful_results);
    println!("Failed transactions: {}", failed_results);
    println!("Threads used: {}", NUM_THREADS);
    
    Ok(())
}
