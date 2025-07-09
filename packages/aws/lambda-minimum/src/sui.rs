use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use sui_sdk::rpc_types::{SuiMoveStruct, SuiMoveValue, SuiParsedData};
use sui_sdk::{SuiClient, SuiClientBuilder};

static SUI_CLIENT: OnceLock<SuiClient> = OnceLock::new();

pub async fn get_sui_client() -> Result<&'static SuiClient, anyhow::Error> {
    if let Some(client) = SUI_CLIENT.get() {
        Ok(client)
    } else {
        let client = SuiClientBuilder::default()
            //.build("https://rpc-ws-devnet.suiscan.xyz")
            //.await?;
            .build_devnet()
            .await?;
        let _ = SUI_CLIENT.set(client.clone());
        let client = SUI_CLIENT.get();
        match client {
            Some(client) => Ok(client),
            None => Err(anyhow::anyhow!("Sui client not found")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestData {
    pub nonce: u64,
    pub agent: String,
    pub action: String,
    pub request: String,
}

const REQUEST_OBJECT_ID: &str =
    "0xca06480ff08a05c51a0aa80e8d74a655533c879370004e9e59b56f81bcb4ba3f"; // devnet

pub async fn get_request() -> Result<RequestData, Box<dyn std::error::Error>> {
    let sui_client = get_sui_client().await?;
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
