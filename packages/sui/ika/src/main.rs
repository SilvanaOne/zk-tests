use ika_sdk::IkaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Ika testnet -- https://fullnode.testnet.ika.io:443
    let ika_testnet = IkaClientBuilder::default().build_testnet().await?;
    println!("Ika testnet version: {}", ika_testnet.api_version());

    // Ika devnet -- https://fullnode.devnet.ika.io:443
    let ika_devnet = IkaClientBuilder::default().build_devnet().await?;
    println!("Ika devnet version: {}", ika_devnet.api_version());

    Ok(())
}
