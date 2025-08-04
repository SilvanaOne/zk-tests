use std::str::FromStr;

use anchor_client::{
    solana_sdk::{
        commitment_config::CommitmentConfig, pubkey::Pubkey, signature::read_keypair_file,
    },
    Client, Cluster,
};

#[test]
fn test_initialize() {
    let program_id = "DrENg7J4SEZbTi419ZA1AnXFzh8wehfwisapdCeTEpqt";
    let anchor_wallet = std::env::var("ANCHOR_WALLET")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            format!("{}/.config/solana/id.json", home)
        });
    let payer = read_keypair_file(&anchor_wallet).unwrap();

    let client = Client::new_with_options(Cluster::Localnet, &payer, CommitmentConfig::confirmed());
    let program_id = Pubkey::from_str(program_id).unwrap();
    let program = client.program(program_id).unwrap();

    let tx = program
        .request()
        .accounts(add::accounts::Initialize {})
        .args(add::instruction::Initialize {})
        .send()
        .expect("");

    println!("Your transaction signature {}", tx);
}
