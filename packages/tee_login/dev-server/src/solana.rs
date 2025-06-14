use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use std::str::FromStr;

pub fn verify_signature(
    address: &str,
    signature: &str,
    message: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let pubkey = Pubkey::from_str(address)?;
    let signature_bytes = hex::decode(signature)?;
    if signature_bytes.len() != 64 {
        return Err("Invalid signature length".into());
    }

    let message_bytes = message.as_bytes().to_vec();
    let signature = Signature::try_from(signature_bytes.as_slice())?;
    let is_valid = signature.verify(pubkey.as_ref(), &message_bytes);

    Ok(is_valid)
}
