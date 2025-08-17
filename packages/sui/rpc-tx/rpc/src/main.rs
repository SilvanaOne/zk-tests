mod state;
mod tx;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::from_path("../.env").ok();
    let a: u64 = 5;
    let b: u64 = 10;

    println!("Sending transaction to calculate_sum({a}, {b})...");
    let started = Instant::now();
    let sum = tx::calculate_sum(a, b).await?;
    let elapsed_ms = started.elapsed().as_millis();
    println!("Transaction executed. Returned sum: {sum}");
    println!("calculate_sum duration: {elapsed_ms} ms");

    println!("Creating shared State...");
    let started = Instant::now();
    let state_id = state::create_state().await?;
    let elapsed_ms = started.elapsed().as_millis();
    println!("State created with id: {state_id}");
    println!("create_state duration: {elapsed_ms} ms");

    let add_value: u64 = 7;
    println!("Adding {add_value} to State {state_id}...");
    let started = Instant::now();
    let new_sum = state::add_to_state(state_id, add_value).await?;
    let elapsed_ms = started.elapsed().as_millis();
    println!("State updated. New sum: {new_sum}");
    println!("add_to_state duration: {elapsed_ms} ms");

    Ok(())
}
