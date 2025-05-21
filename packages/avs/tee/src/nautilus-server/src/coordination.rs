use serde_json;
use reqwest;

#[derive(Debug)]
pub struct RequestData {
    pub nonce: u64,
    pub agent: String,
    pub action: String,
    pub request: String,
}

const REQUEST_OBJECT_ID: &str =
    "0x904a847618f0a6724e3a8894286310190c4e53aa81d8ac61ddd1f073c6881a15";

pub async fn get_request() -> Result<RequestData, Box<dyn std::error::Error + Send + Sync>> {
    let url = "https://fullnode.testnet.sui.io";
    let client = reqwest::Client::new();

    // Create request payload similar to the JavaScript example
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sui_getObject",
        "params": [
            REQUEST_OBJECT_ID,
            {
                "showContent": true
            }
        ]
    });

    // Make the POST request
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch object data: {}", response.status()).into());
    }

    // Parse the response
    let data: serde_json::Value = response.json().await?;
    
    // Extract fields from the response
    let fields = data["result"]["data"]["content"]["fields"]
        .as_object()
        .ok_or("Failed to extract fields from response")?;

    // Extract values
    let nonce_str = fields["nonce"]
        .as_str()
        .ok_or("Missing or invalid nonce field")?;
    
    let agent = fields["name"]
        .as_str()
        .ok_or("Missing or invalid name field")?
        .to_string();
    
    let action = fields["action"]
        .as_str()
        .ok_or("Missing or invalid action field")?
        .to_string();
    
    let request = fields["request"]
        .as_str()
        .ok_or("Missing or invalid request field")?
        .to_string();

    Ok(RequestData {
        nonce: nonce_str.parse::<u64>()?,
        agent,
        action,
        request,
    })
}
