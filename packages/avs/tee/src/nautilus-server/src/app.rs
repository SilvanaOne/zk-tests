// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::common::IntentMessage;
use crate::common::{to_signed_response, IntentScope, ProcessDataRequest, ProcessedDataResponse};
use crate::AppState;
use crate::EnclaveError;
use crate::agent::start_agent;
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
//use serde_json::Value;
use std::sync::Arc;
/// ====
/// Core Nautilus server logic, replace it with your own
/// relavant structs and process_data endpoint.
/// ====

/// Inner type T for IntentMessage<T>
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StartStats {
    pub cpu_cores: u64,
    pub memory: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StartResponse {
    pub memo: String,
    pub time_started: u64,
    pub stats: StartStats,
}

/// Inner type T for ProcessDataRequest<T>
#[derive(Debug, Serialize, Deserialize)]
pub struct StartRequest {
    pub memo: String,
}

pub async fn process_data(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ProcessDataRequest<StartRequest>>,
) -> Result<Json<ProcessedDataResponse<IntentMessage<StartResponse>>>, EnclaveError> {
    // let url = format!(
    //     "https://api.weatherapi.com/v1/current.json?key={}&q={}",
    //     state.api_key, request.payload.location
    // );
    // let response = reqwest::get(url.clone()).await.map_err(|e| {
    //     EnclaveError::GenericError(format!("Failed to get weather response: {}", e))
    // })?;
    // let json = response.json::<Value>().await.map_err(|e| {
    //     EnclaveError::GenericError(format!("Failed to parse weather response: {}", e))
    // })?;
    // let location = json["location"]["name"].as_str().unwrap_or("Unknown");
    // let temperature = json["current"]["temp_c"].as_f64().unwrap_or(0.0) as u64;
    // let last_updated_epoch = json["current"]["last_updated_epoch"].as_u64().unwrap_or(0);
    // let last_updated_timestamp_ms = last_updated_epoch * 1000_u64;
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EnclaveError::GenericError(format!("Failed to get current timestamp: {}", e)))?
        .as_millis() as u64;

    // // 1 hour in milliseconds = 60 * 60 * 1000 = 3_600_000
    // if last_updated_timestamp_ms + 3_600_000 < current_timestamp {
    //     return Err(EnclaveError::GenericError(
    //         "Weather API timestamp is too old".to_string(),
    //     ));
    // }
    let start_stats = get_worker_stats();
    let key = state.api_key.clone();
    tokio::spawn(async move {
        if let Err(e) = start_agent(&key).await {
            eprintln!("Agent error: {}", e);
        }
    });

    Ok(Json(to_signed_response(
        &state.eph_kp,
        StartResponse {
            memo: request.payload.memo.clone(),
            time_started: current_timestamp,
            stats: start_stats,
        },
        current_timestamp,
        IntentScope::Start,
    )))
}

pub fn get_worker_stats() -> StartStats {
    let cpu_cores = num_cpus::get() as u64;
    let memory = sys_info::mem_info()
        .map(|info| info.total)
        .unwrap_or(0);

    StartStats {
        cpu_cores,
        memory,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::common::IntentMessage;
    use axum::{extract::State, Json};
    use fastcrypto::{ed25519::Ed25519KeyPair, traits::KeyPair};

    #[tokio::test]
    async fn test_process_data() {
        let state = Arc::new(AppState {
            eph_kp: Ed25519KeyPair::generate(&mut rand::thread_rng()),
            api_key: "045a27812dbe456392913223221306".to_string(),
        });
        let start_response = process_data(
            State(state),
            Json(ProcessDataRequest {
                payload: StartRequest {
                    memo: "start request 1".to_string(),
                },
            }),
        )
        .await
        .unwrap();
        println!("start_response: {:?}", start_response.response.data);
        assert_eq!(
            start_response.response.data.memo,
            "start request 1"
        );
    }

    // #[test]
    // fn test_serde() {
    //     // test result should be consistent with test_serde in `move/enclave/sources/enclave.move`.
    //     use fastcrypto::encoding::{Encoding, Hex};
    //     let payload = WeatherResponse {
    //         location: "San Francisco".to_string(),
    //         temperature: 13,
    //     };
    //     let timestamp = 1744038900000;
    //     let intent_msg = IntentMessage::new(payload, timestamp, IntentScope::Weather);
    //     let signing_payload = bcs::to_bytes(&intent_msg).expect("should not fail");
    //     assert!(
    //         signing_payload
    //             == Hex::decode("0020b1d110960100000d53616e204672616e636973636f0d00000000000000")
    //                 .unwrap()
    //     );
    // }
}
