// Manual hex encoding/decoding to avoid hex dependency

use super::error::Error;

/// Encode bytes to hex string (lowercase, no 0x prefix)
pub fn encode(bytes: &[u8]) -> alloc::string::String {
    use alloc::format;
    
    let mut hex = alloc::string::String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

/// Decode hex string to bytes (handles 0x prefix)
pub fn decode(hex: &str) -> Result<alloc::vec::Vec<u8>, Error> {
    let hex = if hex.starts_with("0x") || hex.starts_with("0X") {
        &hex[2..]
    } else {
        hex
    };
    
    if hex.len() % 2 != 0 {
        return Err(Error::InvalidData);
    }
    
    let mut bytes = alloc::vec::Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte_str = &hex[i..i + 2];
        let byte = u8::from_str_radix(byte_str, 16).map_err(|_| Error::InvalidData)?;
        bytes.push(byte);
    }
    
    Ok(bytes)
}