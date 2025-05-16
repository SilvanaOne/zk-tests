use sui_sdk::rpc_types::{SuiMoveStruct, SuiMoveValue, SuiParsedData};
use sui_sdk::{SuiClient, SuiClientBuilder};

pub async fn get_sui_client() -> Result<SuiClient, Box<dyn std::error::Error>> {
    let sui_devnet = SuiClientBuilder::default().build_devnet().await?;
    println!("Sui devnet version: {}", sui_devnet.api_version());
    Ok(sui_devnet)
}

#[derive(Debug)]
pub struct RequestData {
    pub nonce: u64,
    pub agent: String,
    pub action: String,
    pub request: String,
}

const REQUEST_OBJECT_ID: &str =
    "0x402eb0550a27eaad2911ff5bd898ee61e6406c1eeac9c66a75cf2fe94cab3136";

pub async fn get_request(
    sui_client: &SuiClient,
) -> Result<RequestData, Box<dyn std::error::Error>> {
    let response = sui_client
        .read_api()
        .get_object_with_options(
            REQUEST_OBJECT_ID.parse()?,
            sui_sdk::rpc_types::SuiObjectDataOptions::new().with_content(),
        )
        .await?;

    // Extract fields from the response
    let content = response.data.unwrap().content.unwrap();
    if let SuiParsedData::MoveObject(move_obj) = content {
        // Handle SuiMoveStruct properly
        if let SuiMoveStruct::WithFields(fields_map) = move_obj.fields {
            // Extract the fields from the map
            let nonce = match fields_map.get("nonce") {
                Some(SuiMoveValue::String(s)) => s.clone(),
                _ => String::default(),
            };

            let agent = match fields_map.get("name") {
                Some(SuiMoveValue::String(s)) => s.clone(),
                _ => String::default(),
            };

            let action = match fields_map.get("action") {
                Some(SuiMoveValue::String(s)) => s.clone(),
                _ => String::default(),
            };

            let request = match fields_map.get("request") {
                Some(SuiMoveValue::String(s)) => s.clone(),
                _ => String::default(),
            };

            Ok(RequestData {
                nonce: nonce.parse::<u64>()?,
                agent,
                action,
                request,
            })
        } else {
            Err("Object fields not in expected format".into())
        }
    } else {
        Err("Invalid object format".into())
    }
}
