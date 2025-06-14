use std::str::FromStr;

use sui_sdk::types::base_types::SuiAddress;
use sui_sdk::types::crypto::EncodeDecodeBase64;
use sui_sdk::types::signature::GenericSignature;
use sui_sdk::verify_personal_message_signature::verify_personal_message_signature;

pub async fn verify_signature(
    address: &str,
    signature_hex: &str,
    message: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "Sui verify signature: {:?}",
        (address, signature_hex, message)
    );
    let bytes = message.as_bytes();
    let signature: GenericSignature = GenericSignature::decode_base64(signature_hex)?;
    let sui_address = SuiAddress::from_str(address)?;

    let res = verify_personal_message_signature(signature, bytes, sui_address, None).await;
    println!("Sui verify signature result: {:?}", res);
    let ok = res.is_ok();
    println!("Sui verify signature result: {:?}", ok);
    Ok(ok)
}
