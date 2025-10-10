mod context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let ctx = context::ContractBlobsContext::fetch().await?;
    println!("{:#?}", ctx);
    Ok(())
}
