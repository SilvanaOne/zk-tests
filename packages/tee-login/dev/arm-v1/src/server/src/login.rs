use crate::auth::{ethereum, github, google, solana, sui};
use crate::db::{Key, Share, Value};
use crate::dynamodb::DynamoDB;
use crate::encrypt::encrypt_shares;
use crate::hash::hash_login_request;
use crate::logger::{log_database_error, log_encryption_error, log_verification_error};
use crate::seed::generate_seed;
use crate::shamir::split_mnemonic;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

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

/// Wrapper struct containing the request payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessDataRequest<T> {
    pub payload: T,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginResponse {
    pub success: bool,
    pub data: Option<Vec<String>>,
    pub indexes: Option<Vec<u32>>,
    pub error: Option<String>,
}

pub async fn process_login(login_request: LoginRequest, db: &DynamoDB) -> LoginResponse {
    info!(
        "Processing login request for chain: {}, wallet: {}, address: {}",
        login_request.chain, login_request.wallet, login_request.address
    );
    if login_request.nonce > chrono::Utc::now().timestamp_millis() as u64 {
        return LoginResponse {
            success: false,
            data: None,
            indexes: None,
            error: Some("Nonce error E101".into()),
        };
    }

    if !hash_login_request(&login_request) {
        return LoginResponse {
            success: false,
            data: None,
            indexes: None,
            error: Some("Hash error E102".into()),
        };
    }

    let verification_result = verify(&login_request).await;
    info!(
        "Verification result: ok={}, error={:?}",
        verification_result.is_valid, verification_result.error
    );
    let address = if verification_result.address.is_some() {
        verification_result.address.unwrap()
    } else {
        login_request.address.clone()
    };
    let request_nonce = if verification_result.nonce.is_some() {
        verification_result.nonce.unwrap()
    } else {
        login_request.nonce
    };

    if verification_result.nonce.is_some()
        && verification_result.nonce.unwrap() > chrono::Utc::now().timestamp_millis() as u64
    {
        return LoginResponse {
            success: false,
            data: None,
            indexes: None,
            error: Some("Nonce error E103".into()),
        };
    }

    let mut data: Option<Vec<String>> = None;
    let mut indexes: Option<Vec<u32>> = None;
    if verification_result.is_valid {
        // Create a key from the login request
        let key = Key {
            login_type: login_request.login_type.clone(),
            chain: login_request.chain.clone(),
            wallet: login_request.wallet.clone(),
            address: address.clone(),
        };

        // Try to get existing data from the database
        match db.read(&key).await {
            Ok(Some((value, nonce))) => {
                info!("Found existing data in database for address: {}", &address);
                if request_nonce > nonce {
                    info!("Nonce is greater than the existing nonce, updating the database");
                    if let Err(e) = db.update(&key, request_nonce).await {
                        let error_msg = format!("Failed to update database: {}", e);
                        log_database_error("update", &error_msg);
                    }
                } else {
                    error!("Nonce is less than the existing nonce, returning error, request nonce: {}, existing nonce: {}", request_nonce, nonce);
                    return LoginResponse {
                        success: false,
                        data: None,
                        indexes: None,
                        error: Some("Nonce error E104: reusing access token is prohibited for security reasons".into()),
                    };
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
                        return LoginResponse {
                            success: false,
                            data: None,
                            indexes: None,
                            error: Some(error_msg),
                        };
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
                        return LoginResponse {
                            success: false,
                            data: None,
                            indexes: None,
                            error: Some(error_msg),
                        };
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
                            return LoginResponse {
                                success: false,
                                data: None,
                                indexes: None,
                                error: Some(error_msg),
                            };
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to encrypt seed: {}", e);
                        log_encryption_error(&error_msg);
                        return LoginResponse {
                            success: false,
                            data: None,
                            indexes: None,
                            error: Some(error_msg),
                        };
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Database error: {}", e);
                log_database_error("get_kv", &error_msg);
                return LoginResponse {
                    success: false,
                    data: None,
                    indexes: None,
                    error: Some(error_msg),
                };
            }
        }
    }

    LoginResponse {
        success: verification_result.is_valid,
        data,
        indexes,
        error: verification_result.error,
    }
}

pub struct VerifyResult {
    pub is_valid: bool,
    pub address: Option<String>,
    pub nonce: Option<u64>,
    pub error: Option<String>,
}

async fn verify(request: &LoginRequest) -> VerifyResult {
    let message = &request.message;
    let signature = &request.signature;
    let address = &request.address;
    let result: Result<VerifyResult, Box<dyn std::error::Error>> = match request.login_type.as_str()
    {
        "social" => match request.chain.as_str() {
            "google" => google::verify_signature(&request.address, signature, message).await,
            "github" => github::verify_signature(&request.address, signature, message).await,
            _ => {
                return VerifyResult {
                    is_valid: false,
                    address: None,
                    nonce: None,
                    error: Some("Invalid social login provider".into()),
                };
            }
        },
        "wallet" => match request.chain.as_str() {
            "solana" => solana::verify_signature(address, signature, message),
            "sui" => sui::verify_signature(address, signature, message).await,
            "ethereum" => ethereum::verify_signature(address, signature, message),
            _ => {
                return VerifyResult {
                    is_valid: false,
                    address: None,
                    nonce: None,
                    error: Some("Invalid chain".into()),
                };
            }
        },
        _ => {
            return VerifyResult {
                is_valid: false,
                address: None,
                nonce: None,
                error: Some("Invalid login type".into()),
            };
        }
    };

    match result {
        Ok(result) => result,
        Err(e) => {
            let error_msg = format!("Error verifying signature: {}", e);
            log_verification_error(&request.chain, &request.address, &error_msg);
            VerifyResult {
                is_valid: false,
                address: None,
                nonce: None,
                error: Some(error_msg),
            }
        }
    }
}
