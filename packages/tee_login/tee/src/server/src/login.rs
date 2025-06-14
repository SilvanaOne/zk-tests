use crate::EnclaveError;
use crate::db::{Key, Share, Value};
use crate::dynamodb::DynamoDB;
use crate::encrypt::encrypt_shares;
use crate::hash::hash_login_request;
use crate::logger::{log_database_error, log_encryption_error, log_verification_error};
use crate::seed::generate_seed;
use crate::shamir::split_mnemonic;
use crate::solana;
use crate::sui;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginRequest {
    pub login_type: String,
    pub chain: String,
    pub wallet: String,
    pub message: String,
    pub signature: String,
    pub address: String,
    pub public_key: String,
    pub nonce: u64,
    pub share_indexes: Vec<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginResponse {
    pub success: bool,
    pub data: Option<Vec<String>>,
    pub indexes: Option<Vec<u32>>,
    pub error: Option<String>,
}

pub async fn process_login(
    login_request: LoginRequest,
    db: &DynamoDB,
) -> Result<LoginResponse, EnclaveError> {
    info!(
        "Processing login request for chain: {}, wallet: {}, address: {}",
        login_request.chain, login_request.wallet, login_request.address
    );
    if login_request.nonce > chrono::Utc::now().timestamp_millis() as u64 {
        return Ok(LoginResponse {
            success: false,
            data: None,
            indexes: None,
            error: Some("Nonce error E101".into()),
        });
    }

    if !hash_login_request(&login_request) {
        return Ok(LoginResponse {
            success: false,
            data: None,
            indexes: None,
            error: Some("Hash error E103".into()),
        });
    }

    let (ok, error) = verify(&login_request).await;
    info!("Verification result: ok={}, error={:?}", ok, error);
    let mut data: Option<Vec<String>> = None;
    let mut indexes: Option<Vec<u32>> = None;
    if ok {
        // Create a key from the login request
        let key = Key {
            chain: login_request.chain.clone(),
            wallet: login_request.wallet.clone(),
            address: login_request.address.clone(),
        };

        // Try to get existing data from the database
        match db.read(&key).await {
            Ok(Some((value, nonce))) => {
                info!(
                    "Found existing data in database for address: {}",
                    login_request.address
                );
                if login_request.nonce > nonce {
                    info!("Nonce is greater than the existing nonce, updating the database");
                    if let Err(e) = db.update(&key, login_request.nonce).await {
                        let error_msg = format!("Failed to update database: {}", e);
                        log_database_error("update", &error_msg);
                    }
                } else {
                    info!("Nonce is less than the existing nonce, returning error");
                    return Ok(LoginResponse {
                        success: false,
                        data: None,
                        indexes: None,
                        error: Some("Nonce error E102".into()),
                    });
                }
                let filtered_shares: Vec<Share> = value
                    .shares
                    .iter()
                    .filter(|s| login_request.share_indexes.contains(&s.index))
                    .cloned()
                    .collect();
                indexes = Some(filtered_shares.iter().map(|s| s.index).collect());

                match encrypt_shares(&filtered_shares, &login_request.public_key) {
                    Ok(encrypted_data) => {
                        data = Some(encrypted_data);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to encrypt seed: {}", e);
                        log_encryption_error(&error_msg);
                        return Ok(LoginResponse {
                            success: false,
                            data: None,
                            indexes: None,
                            error: Some(error_msg),
                        });
                    }
                }
            }
            Ok(None) => {
                info!(
                    "No existing data found, generating new seed for address: {}",
                    login_request.address
                );
                let seed = generate_seed(12);
                let splitted_seed = match split_mnemonic(&seed) {
                    Ok(shares) => shares,
                    Err(e) => {
                        let error_msg = format!("Failed to split mnemonic: {}", e);
                        log_encryption_error(&error_msg);
                        return Ok(LoginResponse {
                            success: false,
                            data: None,
                            indexes: None,
                            error: Some(error_msg),
                        });
                    }
                };

                let shares: Vec<Share> = splitted_seed
                    .iter()
                    .enumerate()
                    .map(|(index, s)| Share {
                        index: index as u32,
                        data: s.clone(),
                    })
                    .collect();

                let filtered_shares: Vec<Share> = shares
                    .iter()
                    .filter(|s| login_request.share_indexes.contains(&s.index))
                    .cloned()
                    .collect();
                indexes = Some(filtered_shares.iter().map(|s| s.index).collect());

                match encrypt_shares(&filtered_shares, &login_request.public_key) {
                    Ok(encrypted_data) => {
                        data = Some(encrypted_data);

                        if let Err(e) = db
                            .create(
                                &key,
                                &Value {
                                    created_at: chrono::Utc::now().timestamp_millis() as u64,
                                    expiry: (chrono::Utc::now().timestamp_millis()
                                        + 1000 * 60 * 60 * 24 * 365)
                                        as u64,
                                    shares,
                                },
                                chrono::Utc::now().timestamp_millis() as u64, // nonce
                            )
                            .await
                        {
                            let error_msg = format!("Failed to store seed in database: {}", e);
                            log_database_error("put_kv", &error_msg);
                            return Ok(LoginResponse {
                                success: false,
                                data: None,
                                indexes: None,
                                error: Some(error_msg),
                            });
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to encrypt seed: {}", e);
                        log_encryption_error(&error_msg);
                        return Ok(LoginResponse {
                            success: false,
                            data: None,
                            indexes: None,
                            error: Some(error_msg),
                        });
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Database error: {}", e);
                log_database_error("get_kv", &error_msg);
                return Ok(LoginResponse {
                    success: false,
                    data: None,
                    indexes: None,
                    error: Some(error_msg),
                });
            }
        }
    }

    Ok(LoginResponse {
        success: true,
        data,
        indexes,
        error,
    })
}

async fn verify(request: &LoginRequest) -> (bool, Option<String>) {
    let message = &request.message;
    let address = &request.address;
    let signature = &request.signature;
    let result = match request.chain.as_str() {
        "solana" => solana::verify_signature(address, signature, message),
        "sui" => sui::verify_signature(address, signature, message).await,
        _ => return (false, Some("Invalid chain".into())),
    };
    match result {
        Ok(is_valid) => {
            if is_valid {
                (true, None)
            } else {
                (false, Some("Invalid signature".into()))
            }
        }
        Err(e) => {
            let error_msg = format!("Error verifying signature: {}", e);
            log_verification_error(&request.chain, &request.address, &error_msg);
            (false, Some(error_msg))
        }
    }
}
