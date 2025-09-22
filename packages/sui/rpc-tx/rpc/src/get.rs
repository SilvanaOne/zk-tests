use anyhow::Result;
use std::env;
use std::str::FromStr;
use sui_rpc::proto::sui::rpc::v2 as proto;
use sui_rpc::client::v2::Client as GrpcClient;
use sui_sdk_types as sui;
use serde_json;

fn rpc_url_from_env() -> String {
    let chain = env::var("SUI_CHAIN").unwrap_or_else(|_| "devnet".to_string());
    println!("[get] using chain={}", chain);
    match chain.as_str() {
        "mainnet" => "https://fullnode.mainnet.sui.io:443".to_string(),
        "testnet" => "https://fullnode.testnet.sui.io:443".to_string(),
        "devnet" => "https://fullnode.devnet.sui.io:443".to_string(),
        _ => "https://fullnode.devnet.sui.io:443".to_string(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::from_path("../.env").ok();
    
    // Get the State object ID from environment
    let object_id_str = env::var("SUI_OBJECT_ID")
        .expect("SUI_OBJECT_ID environment variable not set");
    println!("[get] fetching object: {}", object_id_str);
    
    // Parse the object ID
    let object_id = sui::Address::from_str(&object_id_str)?;
    
    // Create gRPC client
    let rpc_url = rpc_url_from_env();
    println!("[get] rpc_url={}", rpc_url);
    let mut client = GrpcClient::new(rpc_url)?;

    // First, get the State object
    let state_object = {
        let mut ledger = client.ledger_client();
        let mut req = proto::GetObjectRequest::default();
        req.object_id = Some(object_id.to_string());
        req.version = None;
        req.read_mask = Some(prost_types::FieldMask {
            paths: vec!["*".into()],
        });

        println!("[get] sending GetObject request...");
        let resp = ledger.get_object(req).await?.into_inner();
        resp.object
    };

    // Process the State object
    if let Some(object) = state_object {
        let json = serde_json::to_string_pretty(&object)?;
        println!("\n=== State Object ===");
        println!("{}", json);

        // Extract the values ObjectTable ID
        let values_id = if let Some(json_obj) = &object.json {
            use prost_types::value::Kind;
            if let Some(Kind::StructValue(s)) = &json_obj.kind {
                if let Some(values_field) = s.fields.get("values") {
                    if let Some(Kind::StructValue(values_struct)) = &values_field.kind {
                        if let Some(id_field) = values_struct.fields.get("id") {
                            if let Some(Kind::StringValue(id)) = &id_field.kind {
                                Some(id.clone())
                            } else { None }
                        } else { None }
                    } else { None }
                } else { None }
            } else { None }
        } else { None };

        // If we found the values table ID, list its dynamic fields
        if let Some(values_id) = values_id {
            println!("\n=== Values ObjectTable ===");
            println!("[get] fetching ObjectTable entries: {}", values_id);

            // List dynamic fields
            let dynamic_fields = {
                let mut state = client.state_client();
                let mut list_req = proto::ListDynamicFieldsRequest::default();
                list_req.parent = Some(values_id.clone());
                list_req.page_size = Some(100);
                list_req.read_mask = Some(prost_types::FieldMask {
                    paths: vec!["field_id".into(), "name".into(), "field_object.object_id".into()],
                });
                let resp = state.list_dynamic_fields(list_req).await?.into_inner();
                resp.dynamic_fields
            };

            println!("[get] Found {} entries in ObjectTable", dynamic_fields.len());

            // Print the raw dynamic fields response
            println!("\n=== Dynamic Fields Response ===");
            println!("{}", serde_json::to_string_pretty(&dynamic_fields)?);

            // Collect object IDs to fetch
            let mut object_ids_to_fetch = Vec::new();
            for field in &dynamic_fields {
                if let Some(field_obj) = &field.field_object {
                    if let Some(obj_id) = &field_obj.object_id {
                        object_ids_to_fetch.push(obj_id.clone());
                    }
                }
            }

            // Fetch all objects in one go
            if !object_ids_to_fetch.is_empty() {
                let mut ledger = client.ledger_client();
                for (idx, obj_id) in object_ids_to_fetch.iter().enumerate() {
                    println!("\n--- Entry {} ---", idx + 1);

                    // Print the key from the dynamic field
                    if idx < dynamic_fields.len() {
                        if let Some(name) = &dynamic_fields[idx].name {
                            println!("Key: {}", serde_json::to_string_pretty(name)?);
                        }
                    }

                    let mut obj_req = proto::GetObjectRequest::default();
                    obj_req.object_id = Some(obj_id.clone());
                    obj_req.version = None;
                    obj_req.read_mask = Some(prost_types::FieldMask {
                        paths: vec!["*".into()],
                    });

                    let obj_resp = ledger.get_object(obj_req).await?.into_inner();
                    if let Some(object) = obj_resp.object {
                        println!("Value: {}", serde_json::to_string_pretty(&object)?);
                    }
                }
            }
        }
    } else {
        println!("[get] Object not found");
    }
    
    Ok(())
}