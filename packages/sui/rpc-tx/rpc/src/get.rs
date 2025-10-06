use anyhow::Result;
use std::env;
use std::str::FromStr;
use sui_rpc::proto::sui::rpc::v2 as proto;
use sui_rpc::client::v2::Client as GrpcClient;
use sui_sdk_types as sui;
use serde_json;
use serde::{Serialize, Deserialize};

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

/// Wrapper type for ObjectTable keys (matches Move's dynamic_object_field::Wrapper)
#[derive(Serialize, Deserialize, Debug)]
struct DOFWrapper<T> {
    name: T,
}

/// Compute the dynamic field ID for an ObjectTable entry
fn derive_object_table_field_id(parent: sui::Address, key: u64) -> Result<sui::Address> {
    use sui_sdk_types::{Identifier, StructTag, TypeTag};

    // Create the wrapper for the key
    let wrapper = DOFWrapper { name: key };

    // Serialize the key using BCS
    let key_bytes = bcs::to_bytes(&wrapper)?;
    println!("[debug] Key bytes (hex): {}", hex::encode(&key_bytes));

    // Create the TypeTag for 0x2::dynamic_object_field::Wrapper<u64>
    let wrapper_struct = StructTag {
        address: sui::Address::TWO,
        module: Identifier::new("dynamic_object_field")?,
        name: Identifier::new("Wrapper")?,
        type_params: vec![TypeTag::U64],
    };
    let type_tag = TypeTag::Struct(Box::new(wrapper_struct));

    println!("[debug] Parent: {}", parent);

    // Use the built-in derive_dynamic_child_id method which handles all the hashing correctly
    let field_id = parent.derive_dynamic_child_id(&type_tag, &key_bytes);

    Ok(field_id)
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

        // If we found the values table ID, fetch entries
        if let Some(values_id) = values_id {
            println!("\n=== Values ObjectTable ===");

            // Check if we should fetch a specific key
            let specific_key = env::var("SUI_FETCH_KEY").ok().and_then(|s| s.parse::<u64>().ok());

            if let Some(key) = specific_key {
                // Direct fetch for specific key
                println!("[get] Fetching specific key {} from ObjectTable {}", key, values_id);

                // Compute the field ID for this key
                let field_id = match derive_object_table_field_id(sui::Address::from_str(&values_id)?, key) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to compute field ID: {}", e);
                        return Ok(());
                    }
                };

                println!("[get] Computed field ID: {}", field_id);

                // Print the actual field ID for key=2 (from our earlier output)
                if key == 2 {
                    println!("[get] Expected field ID for key=2: 0x82cb0cdc4fc56d199f933622373dd399438877217a976fb90ad74a05098e29df");
                }

                // Fetch the field object directly
                let mut ledger = client.ledger_client();
                let mut field_req = proto::GetObjectRequest::default();
                field_req.object_id = Some(field_id.to_string());
                field_req.version = None;
                field_req.read_mask = Some(prost_types::FieldMask {
                    paths: vec!["*".into()],
                });

                match ledger.get_object(field_req).await {
                    Ok(resp) => {
                        if let Some(field_obj) = resp.into_inner().object {
                            println!("\n=== Field Object for Key {} ===", key);
                            println!("Field Object Type: {}", field_obj.object_type.as_deref().unwrap_or("unknown"));

                            // Extract the Add object ID from the field
                            if let Some(json) = &field_obj.json {
                                use prost_types::value::Kind;
                                if let Some(Kind::StructValue(s)) = &json.kind {
                                    if let Some(value_field) = s.fields.get("value") {
                                        if let Some(Kind::StringValue(add_obj_id)) = &value_field.kind {
                                            println!("Add object ID: {}", add_obj_id);

                                            // Fetch the Add object
                                            let mut add_req = proto::GetObjectRequest::default();
                                            add_req.object_id = Some(add_obj_id.clone());
                                            add_req.version = None;
                                            add_req.read_mask = Some(prost_types::FieldMask {
                                                paths: vec!["*".into()],
                                            });

                                            match ledger.get_object(add_req).await {
                                                Ok(add_resp) => {
                                                    if let Some(add_obj) = add_resp.into_inner().object {
                                                        println!("\n=== Add Object (key={}) ===", key);
                                                        println!("{}", serde_json::to_string_pretty(&add_obj)?);
                                                    }
                                                }
                                                Err(e) => {
                                                    println!("Failed to fetch Add object: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            println!("Field object not found for key {}", key);
                        }
                    }
                    Err(e) => {
                        println!("Failed to fetch field object: {}", e);
                        println!("Note: Field ID computation may need adjustment for proper TypeTag serialization");
                    }
                }

                return Ok(());
            }

            // Otherwise, list all entries as before
            println!("[get] fetching ObjectTable entries: {}", values_id);

            // List dynamic fields
            let dynamic_fields = {
                let mut state = client.state_client();
                let mut list_req = proto::ListDynamicFieldsRequest::default();
                list_req.parent = Some(values_id.clone());
                list_req.page_size = Some(10);
                // Use default read_mask which is "parent,field_id"
                let resp = state.list_dynamic_fields(list_req).await?.into_inner();
                resp.dynamic_fields
            };

            println!("[get] Found {} entries in ObjectTable", dynamic_fields.len());

            // Print the raw dynamic fields response
            println!("\n=== Dynamic Fields Response ===");
            println!("{}", serde_json::to_string_pretty(&dynamic_fields)?);

            // Process each dynamic field - we'll need to fetch full details for each one
            let mut ledger = client.ledger_client();
            for (idx, field) in dynamic_fields.iter().enumerate() {
                println!("\n--- Entry {} ---", idx + 1);

                // We only have field_id from the default read_mask
                if let Some(field_id) = &field.field_id {
                    println!("Dynamic Field ID: {}", field_id);

                    // Fetch the actual field object to get its details
                    let mut field_req = proto::GetObjectRequest::default();
                    field_req.object_id = Some(field_id.clone());
                    field_req.version = None;
                    field_req.read_mask = Some(prost_types::FieldMask {
                        paths: vec!["*".into()],
                    });

                    match ledger.get_object(field_req).await {
                        Ok(resp) => {
                            if let Some(field_obj) = resp.into_inner().object {
                                // This is the dynamic field wrapper object
                                println!("Field Object Type: {}", field_obj.object_type.as_deref().unwrap_or("unknown"));

                                // Parse the field to get the key and value info
                                if let Some(json) = &field_obj.json {
                                    println!("Field Contents (JSON): {:#?}", json);

                                    // For ObjectTable, the value field should contain the object ID
                                    use prost_types::value::Kind;
                                    if let Some(Kind::StructValue(s)) = &json.kind {
                                        // Extract the key (name field)
                                        let mut key_str = String::new();
                                        if let Some(name_field) = s.fields.get("name") {
                                            if let Some(Kind::StructValue(name_struct)) = &name_field.kind {
                                                if let Some(key) = name_struct.fields.get("name") {
                                                    if let Some(Kind::StringValue(k)) = &key.kind {
                                                        key_str = k.clone();
                                                        println!("Key: {}", key_str);
                                                    }
                                                }
                                            }
                                        }

                                        // The value field contains the object ID (not nested in a struct)
                                        if let Some(value_field) = s.fields.get("value") {
                                            if let Some(Kind::StringValue(add_obj_id)) = &value_field.kind {
                                                println!("Add object ID: {}", add_obj_id);

                                                // Fetch the actual Add object
                                                let mut add_req = proto::GetObjectRequest::default();
                                                add_req.object_id = Some(add_obj_id.clone());
                                                add_req.version = None;
                                                add_req.read_mask = Some(prost_types::FieldMask {
                                                    paths: vec!["*".into()],
                                                });

                                                match ledger.get_object(add_req).await {
                                                    Ok(add_resp) => {
                                                        if let Some(add_obj) = add_resp.into_inner().object {
                                                            println!("\nAdd Object (key={}):", key_str);
                                                            println!("{}", serde_json::to_string_pretty(&add_obj)?);
                                                        }
                                                    }
                                                    Err(e) => {
                                                        println!("Failed to fetch Add object: {}", e);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("Failed to fetch field object: {}", e);
                        }
                    }
                }
            }
        }
    } else {
        println!("[get] Object not found");
    }
    
    Ok(())
}