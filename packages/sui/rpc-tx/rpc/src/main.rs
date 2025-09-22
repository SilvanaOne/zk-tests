mod state;
mod tx;
mod v2;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::from_path("../.env").ok();
    let a: u64 = 5;
    let b: u64 = 10;

    // println!("Sending transaction to calculate_sum({a}, {b})...");
    // let started = Instant::now();
    // let sum = tx::calculate_sum(a, b).await?;
    // let elapsed_ms = started.elapsed().as_millis();
    // println!("Transaction executed. Returned sum: {sum}");
    // println!("calculate_sum duration: {elapsed_ms} ms");

    // // Test chained calculate_sums
    // let pairs = vec![(3, 4), (5, 6), (7, 8)];
    // println!("\nSending transaction with chained calculate_sums for pairs: {:?}...", pairs);
    // let started = Instant::now();
    // let sums = tx::calculate_sums(pairs.clone()).await?;
    // let elapsed_ms = started.elapsed().as_millis();
    // println!("Transaction executed. Results:");
    // for (i, sum) in sums.iter().enumerate() {
    //     if i == 0 {
    //         println!("  {} + {} = {}", pairs[i].0, pairs[i].1, sum);
    //     } else {
    //         println!("  {} + {} = {}", sums[i-1], pairs[i].1, sum);
    //     }
    // }
    // println!("calculate_sums duration: {elapsed_ms} ms");

    println!("Creating shared State using v2 API...");
    let started = Instant::now();
    let state_id = v2::create_state().await?;
    let elapsed_ms = started.elapsed().as_millis();
    println!("State created with id: {state_id}");
    println!("create_state (v2) duration: {elapsed_ms} ms");

    let add_value: u64 = 7;
    println!("Adding {add_value} to State {state_id} using v2 API...");
    let started = Instant::now();
    let new_sum = v2::add_to_state(state_id, add_value).await?;
    let elapsed_ms = started.elapsed().as_millis();
    println!("State updated. New sum: {new_sum}");
    println!("add_to_state (v2) duration: {elapsed_ms} ms");

    // Test multiple adds in one transaction
    let add_values = vec![3, 5, 2];
    println!("\nAdding multiple values {:?} to State {state_id} in one transaction using v2 API...", add_values);
    let started = Instant::now();
    let new_sums = v2::multiple_add_to_state(state_id, add_values.clone()).await?;
    let elapsed_ms = started.elapsed().as_millis();
    println!("State updated through multiple operations. Results:");
    for (i, sum) in new_sums.iter().enumerate() {
        if i == 0 {
            println!("  {} + {} = {}", new_sum, add_values[i], sum);
        } else {
            println!("  {} + {} = {}", new_sums[i-1], add_values[i], sum);
        }
    }
    println!("multiple_add_to_state (v2) duration: {elapsed_ms} ms");

    Ok(())
}
