mod context;
mod url;
mod pay;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Get command line arguments
    let args: Vec<String> = std::env::args().collect();

    // Fetch contract blobs context
    let ctx = context::ContractBlobsContext::fetch().await?;

    if args.len() > 1 && args[1] == "pay" {
        // Execute a payment
        println!("Executing payment from subscriber to PARTY_APP using preapproval...");
        let payment_args = pay::PaymentArgs::from_context(ctx).await?;
        payment_args.execute_payment().await?;
    } else {
        // Default behavior: print the context
        println!("{:#?}", ctx);
        println!("\nTo execute a payment, run: cargo run -- pay");
    }

    Ok(())
}
