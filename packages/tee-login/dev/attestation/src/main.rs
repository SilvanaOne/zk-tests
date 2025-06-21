mod attestation;
mod nitro_attestation;
use serde_json::Value;
fn main() {
    let attestation = include_str!("../data/release4.json");
    let attestation_str = parse_attestation(attestation).expect("Failed to parse attestation");
    let result = attestation::verify_attestation(&attestation_str);
    println!("{:?}", result);
}

fn parse_attestation(attestation: &str) -> Result<String, Box<dyn std::error::Error>> {
    let json: Value = match serde_json::from_str(attestation) {
        Ok(v) => v,
        Err(e) => return Err(format!("Invalid attestation json: {}", e).into()),
    };

    let attestation_str = match json.get("attestation").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return Err("Invalid attestation json - missing attestation field".into()),
    };

    Ok(attestation_str.to_string())
}
