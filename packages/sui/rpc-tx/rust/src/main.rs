mod tx;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let a: u64 = 2;
    let b: u64 = 3;

    println!("Sending transaction to calculate_sum({a}, {b})...");
    let started = Instant::now();
    let sum = tx::calculate_sum(a, b).await?;
    let elapsed_ms = started.elapsed().as_millis();
    println!("Transaction executed. Returned sum: {sum}");
    println!("calculate_sum duration: {elapsed_ms} ms");

    Ok(())
}
