//! Axum HTTP server for Advanced Payment API.
//!
//! Exposes CLI commands as REST endpoints.

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::context::{create_client, ContractBlobsContext};
use crate::interactive::submit_interactive;
use crate::signing::{extract_user_id_from_jwt, parse_base58_private_key};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub ledger_api_url: String,
    pub scan_api_url: String,
    pub party_provider: String,
    pub synchronizer_id: String,
    pub package_id: String,
    pub package_name: String,
    // Optional defaults from .env
    pub default_jwt: Option<String>,
    pub default_seller_party_id: Option<String>,
    pub default_seller_private_key: Option<String>,
}

impl AppState {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            ledger_api_url: std::env::var("LEDGER_API_URL")
                .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?,
            scan_api_url: std::env::var("SCAN_API_URL")
                .map_err(|_| anyhow::anyhow!("SCAN_API_URL not set"))?,
            party_provider: std::env::var("PARTY_PROVIDER")
                .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?,
            synchronizer_id: std::env::var("SYNCHRONIZER_ID")
                .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?,
            package_id: std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
                .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?,
            package_name: std::env::var("ADVANCED_PAYMENT_PACKAGE_NAME").unwrap_or_default(),
            default_jwt: std::env::var("JWT").ok(),
            default_seller_party_id: std::env::var("PARTY_SELLER").ok(),
            default_seller_private_key: std::env::var("PARTY_SELLER_PRIVATE_KEY").ok(),
        })
    }

    /// Get API URL with trailing slash
    pub fn api_url(&self) -> String {
        if self.ledger_api_url.ends_with('/') {
            self.ledger_api_url.clone()
        } else {
            format!("{}/", self.ledger_api_url)
        }
    }
}

// ============= Response Types =============

#[derive(Serialize)]
pub struct SubmissionResult {
    pub submission_id: String,
    pub update_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_contract_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_amount: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub ledger_api_url: String,
    pub scan_api_url: String,
}

#[derive(Serialize)]
pub struct AmuletInfo {
    pub contract_id: String,
    pub amount: String,
    pub is_locked: bool,
    pub lock_holders: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_expires_at: Option<String>,
}

#[derive(Serialize)]
pub struct AdvancedPaymentInfo {
    pub contract_id: String,
    pub locked_amount: String,
    pub minimum_amount: String,
    pub buyer: String,
    pub seller: String,
    pub provider: String,
    pub expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Serialize)]
pub struct AdvancedPaymentRequestInfo {
    pub contract_id: String,
    pub locked_amount: String,
    pub minimum_amount: String,
    pub buyer: String,
    pub seller: String,
    pub provider: String,
    pub expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Serialize)]
pub struct AppServiceInfo {
    pub contract_id: String,
    pub seller: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_description: Option<String>,
}

#[derive(Serialize)]
pub struct AppServiceRequestInfo {
    pub contract_id: String,
    pub seller: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_description: Option<String>,
}

// ============= Request Types =============

#[derive(Deserialize)]
pub struct ServiceRequestCreateReq {
    pub jwt: Option<String>,
    pub seller_party_id: Option<String>,
    pub seller_private_key: Option<String>,
    pub service_description: Option<String>,
}

#[derive(Deserialize)]
pub struct ServiceRequestAcceptReq {
    pub jwt: Option<String>,
    pub request_cid: String,
}

#[derive(Deserialize)]
pub struct ServiceRequestRejectReq {
    pub jwt: Option<String>,
    pub request_cid: String,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct ServiceRequestCancelReq {
    pub jwt: Option<String>,
    pub seller_party_id: Option<String>,
    pub seller_private_key: Option<String>,
    pub request_cid: String,
}

#[derive(Deserialize)]
pub struct ServiceTerminateReq {
    pub jwt: Option<String>,
    pub service_cid: String,
}

#[derive(Deserialize, Default)]
pub struct ServiceListReq {
    pub jwt: Option<String>,
    pub seller_party_id: Option<String>,
}

#[derive(Deserialize)]
pub struct PaymentRequestCreateReq {
    pub jwt: Option<String>,
    pub seller_party_id: Option<String>,
    pub seller_private_key: Option<String>,
    pub service_cid: String,
    pub buyer_party_id: String,
    pub amount: String,
    pub minimum: String,
    pub expires: Option<String>,
    pub description: Option<String>,
    pub reference: Option<String>,
}

#[derive(Deserialize)]
pub struct PaymentRequestAcceptReq {
    pub jwt: Option<String>,
    pub buyer_party_id: String,
    pub buyer_private_key: String,
    pub request_cid: String,
    pub amulet_cids: Vec<String>,
}

#[derive(Deserialize)]
pub struct PaymentRequestRejectReq {
    pub jwt: Option<String>,
    pub buyer_party_id: String,
    pub buyer_private_key: String,
    pub request_cid: String,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct PaymentRequestCancelReq {
    pub jwt: Option<String>,
    pub seller_party_id: Option<String>,
    pub seller_private_key: Option<String>,
    pub request_cid: String,
}

#[derive(Deserialize)]
pub struct PaymentWithdrawReq {
    pub jwt: Option<String>,
    pub seller_party_id: Option<String>,
    pub seller_private_key: Option<String>,
    pub payment_cid: String,
    pub amount: String,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct PaymentUnlockReq {
    pub jwt: Option<String>,
    pub buyer_party_id: String,
    pub buyer_private_key: String,
    pub payment_cid: String,
    pub amount: String,
}

#[derive(Deserialize)]
pub struct PaymentCancelReq {
    pub jwt: Option<String>,
    pub seller_party_id: Option<String>,
    pub seller_private_key: Option<String>,
    pub payment_cid: String,
}

#[derive(Deserialize)]
pub struct PaymentExpireReq {
    pub jwt: Option<String>,
    pub buyer_party_id: String,
    pub buyer_private_key: String,
    pub payment_cid: String,
}

#[derive(Deserialize)]
pub struct PaymentTopupReq {
    pub jwt: Option<String>,
    pub buyer_party_id: String,
    pub buyer_private_key: String,
    pub payment_cid: String,
    pub amount: String,
    pub new_expires: Option<String>,
    pub amulet_cids: Vec<String>,
}

#[derive(Deserialize)]
pub struct ListAmuletsReq {
    pub jwt: Option<String>,
    pub party_id: String,
}

#[derive(Deserialize)]
pub struct ListPaymentsReq {
    pub jwt: Option<String>,
    pub party_id: String,
}

#[derive(Deserialize)]
pub struct ListRequestsReq {
    pub jwt: Option<String>,
    pub party_id: String,
}

// --- Preapproval Request Types ---

#[derive(Deserialize)]
pub struct PreapprovalRequestReq {
    pub jwt: Option<String>,
    pub party_id: String,
    pub private_key: String,
}

#[derive(Deserialize)]
pub struct PreapprovalAcceptReq {
    pub jwt: Option<String>,
}

#[derive(Deserialize)]
pub struct PreapprovalCancelReq {
    pub jwt: Option<String>,
    pub party_id: Option<String>,
}

#[derive(Deserialize)]
pub struct PreapprovalTransferReq {
    pub jwt: Option<String>,
    pub sender_party_id: String,
    pub sender_private_key: String,
    pub receiver_party_id: String,
    pub amount: String,
    pub description: Option<String>,
}

// ============= Handler Helpers =============

fn error_response(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: msg.to_string(),
            details: None,
        }),
    )
}

fn get_seller_credentials(
    state: &AppState,
    jwt: Option<String>,
    party_id: Option<String>,
    private_key: Option<String>,
) -> Result<(String, String, String), (StatusCode, Json<ErrorResponse>)> {
    let jwt = jwt
        .or_else(|| state.default_jwt.clone())
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "jwt is required"))?;
    let party_id = party_id
        .or_else(|| state.default_seller_party_id.clone())
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "seller_party_id is required"))?;
    let private_key = private_key
        .or_else(|| state.default_seller_private_key.clone())
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "seller_private_key is required"))?;
    Ok((jwt, party_id, private_key))
}

fn get_jwt(state: &AppState, jwt: Option<String>) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    jwt.or_else(|| state.default_jwt.clone())
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "jwt is required"))
}

// ============= Handlers =============

async fn health_handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        ledger_api_url: state.ledger_api_url.clone(),
        scan_api_url: state.scan_api_url.clone(),
    })
}

// --- Service Handlers ---

async fn service_request_create_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ServiceRequestCreateReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let (jwt, party_seller, seller_private_key) =
        get_seller_credentials(&state, req.jwt, req.seller_party_id, req.seller_private_key)?;

    let seller_seed = parse_base58_private_key(&seller_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let context = ContractBlobsContext::fetch()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to fetch context: {}", e)))?;
    let party_dso = context.dso_party;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", state.package_id);

    let commands = vec![serde_json::json!({
        "CreateCommand": {
            "templateId": template_id,
            "createArguments": {
                "dso": party_dso,
                "app": party_seller,
                "provider": state.party_provider,
                "serviceDescription": req.service_description
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &party_seller,
        &state.synchronizer_id,
        &seller_seed,
        commands,
        vec![],
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn service_request_accept_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ServiceRequestAcceptReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;
    let user_id = extract_user_id_from_jwt(&jwt)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid JWT: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", state.package_id);
    let command_id = format!("cmd-{}", chrono::Utc::now().timestamp_millis());

    let payload = serde_json::json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": req.request_cid,
                "choice": "AppServiceRequest_Accept",
                "choiceArgument": {}
            }
        }],
        "userId": user_id,
        "commandId": command_id,
        "actAs": [state.party_provider],
        "readAs": [state.party_provider]
    });

    let response = client
        .post(&format!("{}commands/submit-and-wait", state.api_url()))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("API error: {}", text)));
    }

    let response_json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let update_id = response_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(Json(SubmissionResult {
        submission_id: command_id,
        update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn service_request_reject_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ServiceRequestRejectReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;
    let user_id = extract_user_id_from_jwt(&jwt)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid JWT: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", state.package_id);
    let command_id = format!("cmd-{}", chrono::Utc::now().timestamp_millis());

    let payload = serde_json::json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": req.request_cid,
                "choice": "AppServiceRequest_Reject",
                "choiceArgument": {
                    "reason": req.reason
                }
            }
        }],
        "userId": user_id,
        "commandId": command_id,
        "actAs": [state.party_provider],
        "readAs": [state.party_provider]
    });

    let response = client
        .post(&format!("{}commands/submit-and-wait", state.api_url()))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("API error: {}", text)));
    }

    let response_json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let update_id = response_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(Json(SubmissionResult {
        submission_id: command_id,
        update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn service_request_cancel_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ServiceRequestCancelReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let (jwt, party_seller, seller_private_key) =
        get_seller_credentials(&state, req.jwt, req.seller_party_id, req.seller_private_key)?;

    let seller_seed = parse_base58_private_key(&seller_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", state.package_id);

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.request_cid,
            "choice": "AppServiceRequest_Cancel",
            "choiceArgument": {}
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &party_seller,
        &state.synchronizer_id,
        &seller_seed,
        commands,
        vec![],
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn service_terminate_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ServiceTerminateReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;
    let user_id = extract_user_id_from_jwt(&jwt)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid JWT: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AppService:AppService", state.package_id);
    let command_id = format!("cmd-{}", chrono::Utc::now().timestamp_millis());

    let payload = serde_json::json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": req.service_cid,
                "choice": "AppService_Terminate",
                "choiceArgument": {}
            }
        }],
        "userId": user_id,
        "commandId": command_id,
        "actAs": [state.party_provider],
        "readAs": [state.party_provider]
    });

    let response = client
        .post(&format!("{}commands/submit-and-wait", state.api_url()))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("API error: {}", text)));
    }

    let response_json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let update_id = response_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(Json(SubmissionResult {
        submission_id: command_id,
        update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn service_list_handler(
    State(state): State<Arc<AppState>>,
    body: Option<Json<ServiceListReq>>,
) -> Result<Json<Vec<AppServiceInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let req = body.map(|j| j.0).unwrap_or_default();
    let jwt = get_jwt(&state, req.jwt)?;
    let party_seller = req
        .seller_party_id
        .or_else(|| state.default_seller_party_id.clone())
        .unwrap_or_default();

    let user_id = extract_user_id_from_jwt(&jwt)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid JWT: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let payload = serde_json::json!({
        "userId": user_id,
        "actAs": [state.party_provider],
        "readAs": [state.party_provider, party_seller],
        "templateFilters": [{
            "templateFilter": {
                "value": format!("{}:AppService:AppService", state.package_id)
            }
        }]
    });

    let response = client
        .post(&format!("{}state/acs", state.api_url()))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("API error: {}", text)));
    }

    let contracts: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
    let mut services = Vec::new();

    for contract in contracts {
        if let Some(active) = contract.get("activeContract") {
            let cid = active.pointer("/contract/contractId").and_then(|v| v.as_str()).unwrap_or("?");
            let seller = active.pointer("/contract/payload/app").and_then(|v| v.as_str()).unwrap_or("?");
            let provider = active.pointer("/contract/payload/provider").and_then(|v| v.as_str()).unwrap_or("?");
            let desc = active.pointer("/contract/payload/serviceDescription").and_then(|v| v.as_str());

            services.push(AppServiceInfo {
                contract_id: cid.to_string(),
                seller: seller.to_string(),
                provider: provider.to_string(),
                service_description: desc.map(|s| s.to_string()),
            });
        }
    }

    Ok(Json(services))
}

async fn service_requests_handler(
    State(state): State<Arc<AppState>>,
    body: Option<Json<ServiceListReq>>,
) -> Result<Json<Vec<AppServiceRequestInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let req = body.map(|j| j.0).unwrap_or_default();
    let jwt = get_jwt(&state, req.jwt)?;
    let party_seller = req
        .seller_party_id
        .or_else(|| state.default_seller_party_id.clone())
        .unwrap_or_default();

    let user_id = extract_user_id_from_jwt(&jwt)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid JWT: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let payload = serde_json::json!({
        "userId": user_id,
        "actAs": [state.party_provider],
        "readAs": [state.party_provider, party_seller],
        "templateFilters": [{
            "templateFilter": {
                "value": format!("{}:AppServiceRequest:AppServiceRequest", state.package_id)
            }
        }]
    });

    let response = client
        .post(&format!("{}state/acs", state.api_url()))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("API error: {}", text)));
    }

    let contracts: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
    let mut requests = Vec::new();

    for contract in contracts {
        if let Some(active) = contract.get("activeContract") {
            let cid = active.pointer("/contract/contractId").and_then(|v| v.as_str()).unwrap_or("?");
            let seller = active.pointer("/contract/payload/app").and_then(|v| v.as_str()).unwrap_or("?");
            let provider = active.pointer("/contract/payload/provider").and_then(|v| v.as_str()).unwrap_or("?");
            let desc = active.pointer("/contract/payload/serviceDescription").and_then(|v| v.as_str());

            requests.push(AppServiceRequestInfo {
                contract_id: cid.to_string(),
                seller: seller.to_string(),
                provider: provider.to_string(),
                service_description: desc.map(|s| s.to_string()),
            });
        }
    }

    Ok(Json(requests))
}

// --- Payment Request Handlers ---

async fn request_create_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentRequestCreateReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let (jwt, party_seller, seller_private_key) =
        get_seller_credentials(&state, req.jwt, req.seller_party_id, req.seller_private_key)?;

    let seller_seed = parse_base58_private_key(&seller_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AppService:AppService", state.package_id);

    let expires = req.expires.unwrap_or_else(|| {
        let one_day = chrono::Utc::now() + chrono::Duration::days(1);
        one_day.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.service_cid,
            "choice": "AppService_CreatePaymentRequest",
            "choiceArgument": {
                "buyer": req.buyer_party_id,
                "lockedAmount": req.amount,
                "minimumAmount": req.minimum,
                "expiresAt": expires,
                "description": req.description,
                "reference": req.reference
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &party_seller,
        &state.synchronizer_id,
        &seller_seed,
        commands,
        vec![],
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn request_accept_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentRequestAcceptReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;

    let user_seed = parse_base58_private_key(&req.buyer_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let context = ContractBlobsContext::fetch()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to fetch context: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AdvancedPaymentRequest:AdvancedPaymentRequest", state.package_id);

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.request_cid,
            "choice": "AdvancedPaymentRequest_Accept",
            "choiceArgument": {
                "buyerInputs": req.amulet_cids,
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &req.buyer_party_id,
        &state.synchronizer_id,
        &user_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn request_reject_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentRequestRejectReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;

    let user_seed = parse_base58_private_key(&req.buyer_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AdvancedPaymentRequest:AdvancedPaymentRequest", state.package_id);

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.request_cid,
            "choice": "AdvancedPaymentRequest_Reject",
            "choiceArgument": {
                "reason": req.reason
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &req.buyer_party_id,
        &state.synchronizer_id,
        &user_seed,
        commands,
        vec![],
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn request_cancel_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentRequestCancelReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let (jwt, party_seller, seller_private_key) =
        get_seller_credentials(&state, req.jwt, req.seller_party_id, req.seller_private_key)?;

    let seller_seed = parse_base58_private_key(&seller_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AdvancedPaymentRequest:AdvancedPaymentRequest", state.package_id);

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.request_cid,
            "choice": "AdvancedPaymentRequest_Cancel",
            "choiceArgument": {}
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &party_seller,
        &state.synchronizer_id,
        &seller_seed,
        commands,
        vec![],
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

// --- Payment Handlers ---

async fn payment_withdraw_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentWithdrawReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let (jwt, party_seller, seller_private_key) =
        get_seller_credentials(&state, req.jwt, req.seller_party_id, req.seller_private_key)?;

    let seller_seed = parse_base58_private_key(&seller_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let context = ContractBlobsContext::fetch()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to fetch context: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", state.package_id);

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.payment_cid,
            "choice": "AdvancedPayment_Withdraw",
            "choiceArgument": {
                "amount": req.amount,
                "appTransferContext": context.build_app_transfer_context(),
                "withdrawReason": req.reason
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &party_seller,
        &state.synchronizer_id,
        &seller_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn payment_unlock_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentUnlockReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;

    let user_seed = parse_base58_private_key(&req.buyer_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let context = ContractBlobsContext::fetch()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to fetch context: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", state.package_id);

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.payment_cid,
            "choice": "AdvancedPayment_Unlock",
            "choiceArgument": {
                "amount": req.amount,
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &req.buyer_party_id,
        &state.synchronizer_id,
        &user_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn payment_cancel_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentCancelReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let (jwt, party_seller, seller_private_key) =
        get_seller_credentials(&state, req.jwt, req.seller_party_id, req.seller_private_key)?;

    let seller_seed = parse_base58_private_key(&seller_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let context = ContractBlobsContext::fetch()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to fetch context: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", state.package_id);

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.payment_cid,
            "choice": "AdvancedPayment_Cancel",
            "choiceArgument": {
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &party_seller,
        &state.synchronizer_id,
        &seller_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn payment_expire_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentExpireReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;

    let user_seed = parse_base58_private_key(&req.buyer_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let context = ContractBlobsContext::fetch()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to fetch context: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", state.package_id);

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.payment_cid,
            "choice": "AdvancedPayment_Expire",
            "choiceArgument": {
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &req.buyer_party_id,
        &state.synchronizer_id,
        &user_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn payment_topup_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PaymentTopupReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;

    let user_seed = parse_base58_private_key(&req.buyer_private_key)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("Invalid private key: {}", e)))?;

    let context = ContractBlobsContext::fetch()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to fetch context: {}", e)))?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", state.package_id);

    let new_expires = req.new_expires.unwrap_or_else(|| {
        let one_day = chrono::Utc::now() + chrono::Duration::days(1);
        one_day.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    let commands = vec![serde_json::json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": req.payment_cid,
            "choice": "AdvancedPayment_TopUp",
            "choiceArgument": {
                "topUpInputs": req.amulet_cids,
                "topUpAmount": req.amount,
                "newExpiresAt": new_expires,
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    let result = submit_interactive(
        &client,
        &state.api_url(),
        &jwt,
        &req.buyer_party_id,
        &state.synchronizer_id,
        &user_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Submission failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

// --- List Handlers ---

async fn list_amulets_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ListAmuletsReq>,
) -> Result<Json<Vec<AmuletInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    // Get ledger end
    let ledger_end_url = format!("{}/state/ledger-end", state.api_url());
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(&jwt)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?
        .json()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Unable to get ledger end"))?;

    let mut filters_by_party = serde_json::Map::new();
    filters_by_party.insert(
        req.party_id.clone(),
        serde_json::json!({
            "cumulative": [
                {"identifierFilter": {"TemplateFilter": {"value": {"templateId": "#splice-amulet:Splice.Amulet:Amulet", "includeCreatedEventBlob": false}}}},
                {"identifierFilter": {"TemplateFilter": {"value": {"templateId": "#splice-amulet:Splice.Amulet:LockedAmulet", "includeCreatedEventBlob": false}}}}
            ]
        }),
    );

    let query = serde_json::json!({
        "activeAtOffset": offset,
        "filter": { "filtersByParty": filters_by_party },
        "verbose": true
    });

    let contracts: Vec<serde_json::Value> = client
        .post(&format!("{}/state/active-contracts", state.api_url()))
        .bearer_auth(&jwt)
        .json(&query)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?
        .json()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let mut amulets = Vec::new();

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    let template_id = created.get("templateId").and_then(|v| v.as_str()).unwrap_or("");
                    let is_locked = template_id.contains("LockedAmulet");

                    let cid = created.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let amount = created
                        .pointer("/createArgument/amulet/amount/initialAmount")
                        .or_else(|| created.pointer("/createArgument/amount/initialAmount"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("0");

                    let mut lock_holders = Vec::new();
                    let mut lock_expires_at = None;

                    if is_locked {
                        if let Some(holders) = created.pointer("/createArgument/lock/holders").and_then(|v| v.as_array()) {
                            for h in holders {
                                if let Some(s) = h.as_str() {
                                    lock_holders.push(s.to_string());
                                }
                            }
                        }
                        lock_expires_at = created
                            .pointer("/createArgument/lock/expiresAt")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }

                    amulets.push(AmuletInfo {
                        contract_id: cid.to_string(),
                        amount: amount.to_string(),
                        is_locked,
                        lock_holders,
                        lock_expires_at,
                    });
                }
            }
        }
    }

    Ok(Json(amulets))
}

async fn list_payments_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ListPaymentsReq>,
) -> Result<Json<Vec<AdvancedPaymentInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;

    if state.package_name.is_empty() {
        return Ok(Json(vec![]));
    }

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let ledger_end_url = format!("{}/state/ledger-end", state.api_url());
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(&jwt)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?
        .json()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Unable to get ledger end"))?;

    let template_id = format!("#{}:AdvancedPayment:AdvancedPayment", state.package_name);

    let mut filters_by_party = serde_json::Map::new();
    filters_by_party.insert(
        req.party_id.clone(),
        serde_json::json!({
            "cumulative": [{"identifierFilter": {"TemplateFilter": {"value": {"templateId": template_id, "includeCreatedEventBlob": false}}}}]
        }),
    );

    let query = serde_json::json!({
        "activeAtOffset": offset,
        "filter": { "filtersByParty": filters_by_party },
        "verbose": true
    });

    let contracts: Vec<serde_json::Value> = client
        .post(&format!("{}/state/active-contracts", state.api_url()))
        .bearer_auth(&jwt)
        .json(&query)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?
        .json()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let mut payments = Vec::new();

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    let cid = created.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let locked = created.pointer("/createArgument/lockedAmount").and_then(|v| v.as_str()).unwrap_or("?");
                    let minimum = created.pointer("/createArgument/minimumAmount").and_then(|v| v.as_str()).unwrap_or("?");
                    let buyer = created.pointer("/createArgument/buyer").and_then(|v| v.as_str()).unwrap_or("?");
                    let seller = created.pointer("/createArgument/seller").and_then(|v| v.as_str()).unwrap_or("?");
                    let provider = created.pointer("/createArgument/provider").and_then(|v| v.as_str()).unwrap_or("?");
                    let expires = created.pointer("/createArgument/expiresAt").and_then(|v| v.as_str()).unwrap_or("?");
                    let desc = created.pointer("/createArgument/description").and_then(|v| v.as_str());
                    let reference = created.pointer("/createArgument/reference").and_then(|v| v.as_str());

                    payments.push(AdvancedPaymentInfo {
                        contract_id: cid.to_string(),
                        locked_amount: locked.to_string(),
                        minimum_amount: minimum.to_string(),
                        buyer: buyer.to_string(),
                        seller: seller.to_string(),
                        provider: provider.to_string(),
                        expires_at: expires.to_string(),
                        description: desc.map(|s| s.to_string()),
                        reference: reference.map(|s| s.to_string()),
                    });
                }
            }
        }
    }

    Ok(Json(payments))
}

async fn list_requests_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ListRequestsReq>,
) -> Result<Json<Vec<AdvancedPaymentRequestInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let jwt = get_jwt(&state, req.jwt)?;

    if state.package_name.is_empty() {
        return Ok(Json(vec![]));
    }

    let client = create_client()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create client: {}", e)))?;

    let ledger_end_url = format!("{}/state/ledger-end", state.api_url());
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(&jwt)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?
        .json()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Unable to get ledger end"))?;

    let template_id = format!("#{}:AdvancedPaymentRequest:AdvancedPaymentRequest", state.package_name);

    let mut filters_by_party = serde_json::Map::new();
    filters_by_party.insert(
        req.party_id.clone(),
        serde_json::json!({
            "cumulative": [{"identifierFilter": {"TemplateFilter": {"value": {"templateId": template_id, "includeCreatedEventBlob": false}}}}]
        }),
    );

    let query = serde_json::json!({
        "activeAtOffset": offset,
        "filter": { "filtersByParty": filters_by_party },
        "verbose": true
    });

    let contracts: Vec<serde_json::Value> = client
        .post(&format!("{}/state/active-contracts", state.api_url()))
        .bearer_auth(&jwt)
        .json(&query)
        .send()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Request failed: {}", e)))?
        .json()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Invalid response: {}", e)))?;

    let mut requests = Vec::new();

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    let cid = created.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let locked = created.pointer("/createArgument/lockedAmount").and_then(|v| v.as_str()).unwrap_or("?");
                    let minimum = created.pointer("/createArgument/minimumAmount").and_then(|v| v.as_str()).unwrap_or("?");
                    let buyer = created.pointer("/createArgument/buyer").and_then(|v| v.as_str()).unwrap_or("?");
                    let seller = created.pointer("/createArgument/seller").and_then(|v| v.as_str()).unwrap_or("?");
                    let provider = created.pointer("/createArgument/provider").and_then(|v| v.as_str()).unwrap_or("?");
                    let expires = created.pointer("/createArgument/expiresAt").and_then(|v| v.as_str()).unwrap_or("?");
                    let desc = created.pointer("/createArgument/description").and_then(|v| v.as_str());
                    let reference = created.pointer("/createArgument/reference").and_then(|v| v.as_str());

                    requests.push(AdvancedPaymentRequestInfo {
                        contract_id: cid.to_string(),
                        locked_amount: locked.to_string(),
                        minimum_amount: minimum.to_string(),
                        buyer: buyer.to_string(),
                        seller: seller.to_string(),
                        provider: provider.to_string(),
                        expires_at: expires.to_string(),
                        description: desc.map(|s| s.to_string()),
                        reference: reference.map(|s| s.to_string()),
                    });
                }
            }
        }
    }

    Ok(Json(requests))
}

// --- Preapproval Handlers ---

async fn preapproval_request_handler(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<PreapprovalRequestReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    // Set JWT in env if provided (for preapproval module to use)
    if let Some(jwt) = &req.jwt {
        std::env::set_var("JWT_PROVIDER", jwt);
    }

    let update_id = crate::preapproval::handle_request_preapproval(req.party_id, req.private_key)
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: "request-preapproval".to_string(),
        update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

async fn preapproval_accept_handler(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<PreapprovalAcceptReq>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    // Set JWT in env if provided
    if let Some(jwt) = &req.jwt {
        std::env::set_var("JWT_PROVIDER", jwt);
    }

    let update_ids = crate::preapproval::handle_accept_preapprovals()
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed: {}", e)))?;

    Ok(Json(update_ids))
}

async fn preapproval_cancel_handler(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<PreapprovalCancelReq>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    let update_ids = crate::preapproval::handle_cancel_preapprovals(req.party_id, req.jwt)
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed: {}", e)))?;

    Ok(Json(update_ids))
}

async fn preapproval_transfer_handler(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<PreapprovalTransferReq>,
) -> Result<Json<SubmissionResult>, (StatusCode, Json<ErrorResponse>)> {
    // Set JWT in env if provided
    if let Some(jwt) = &req.jwt {
        std::env::set_var("JWT_PROVIDER", jwt);
    }

    let result = crate::preapproval::handle_transfer(
        req.sender_party_id,
        req.sender_private_key,
        req.receiver_party_id,
        req.amount,
        req.description,
    )
    .await
    .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed: {}", e)))?;

    Ok(Json(SubmissionResult {
        submission_id: result.submission_id,
        update_id: result.update_id,
        contract_id: None,
        new_contract_id: None,
        remaining_amount: None,
    }))
}

// ============= Router =============

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health
        .route("/api/health", get(health_handler))
        // Service endpoints
        .route("/api/service/request/create", post(service_request_create_handler))
        .route("/api/service/request/accept", post(service_request_accept_handler))
        .route("/api/service/request/reject", post(service_request_reject_handler))
        .route("/api/service/request/cancel", post(service_request_cancel_handler))
        .route("/api/service/terminate", post(service_terminate_handler))
        .route("/api/service/list", post(service_list_handler))
        .route("/api/service/requests", post(service_requests_handler))
        // Payment request endpoints
        .route("/api/request/create", post(request_create_handler))
        .route("/api/request/accept", post(request_accept_handler))
        .route("/api/request/reject", post(request_reject_handler))
        .route("/api/request/cancel", post(request_cancel_handler))
        // Payment endpoints
        .route("/api/payment/withdraw", post(payment_withdraw_handler))
        .route("/api/payment/unlock", post(payment_unlock_handler))
        .route("/api/payment/cancel", post(payment_cancel_handler))
        .route("/api/payment/expire", post(payment_expire_handler))
        .route("/api/payment/topup", post(payment_topup_handler))
        // List endpoints
        .route("/api/list/amulets", post(list_amulets_handler))
        .route("/api/list/payments", post(list_payments_handler))
        .route("/api/list/requests", post(list_requests_handler))
        // Preapproval endpoints
        .route("/api/preapproval/request", post(preapproval_request_handler))
        .route("/api/preapproval/accept", post(preapproval_accept_handler))
        .route("/api/preapproval/cancel", post(preapproval_cancel_handler))
        .route("/api/preapproval/transfer", post(preapproval_transfer_handler))
        .layer(cors)
        .with_state(state)
}

/// Start the HTTP server
pub async fn start_server(port: u16) -> anyhow::Result<()> {
    let state = Arc::new(AppState::from_env()?);

    let router = create_router(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
