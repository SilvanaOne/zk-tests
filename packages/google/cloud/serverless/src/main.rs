use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use google_cloud_secretmanager_v1::client::SecretManagerService;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};

const SECRET_NAME_SIGNER_NAME: &str = "SIGNER_NAME";
const SECRET_NAME_PRIVATE_KEY: &str = "SIGNER_PRIVATE_KEY";

#[derive(Clone)]
struct AppState {
    name: String,
    signing_key: SigningKey,
    public_key: VerifyingKey,
}

#[derive(Deserialize)]
struct SignRequest {
    message: String,
}

#[derive(Serialize)]
struct SignResponse {
    name: String,
    public_key: String,
    signature: String,
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Secret Manager error: {0}")]
    SecretManager(String),
    #[error("Invalid private key: {0}")]
    InvalidKey(String),
    #[error("Missing environment variable: {0}")]
    MissingEnv(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let message = self.to_string();
        error!("{}", message);
        (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
    }
}

async fn get_secret(client: &SecretManagerService, project_id: &str, secret_name: &str) -> Result<String, AppError> {
    let secret_path = format!("projects/{}/secrets/{}/versions/latest", project_id, secret_name);

    let response = client
        .access_secret_version()
        .set_name(&secret_path)
        .send()
        .await
        .map_err(|e| AppError::SecretManager(format!("Failed to access {}: {}", secret_name, e)))?;

    let payload = response
        .payload
        .ok_or_else(|| AppError::SecretManager(format!("No payload for secret {}", secret_name)))?;

    String::from_utf8(payload.data.into())
        .map_err(|e| AppError::SecretManager(format!("Invalid UTF-8 in secret {}: {}", secret_name, e)))
}

async fn load_secrets() -> Result<AppState, AppError> {
    let project_id = std::env::var("GCP_PROJECT_ID")
        .map_err(|_| AppError::MissingEnv("GCP_PROJECT_ID".to_string()))?;

    let client = SecretManagerService::builder()
        .build()
        .await
        .map_err(|e| AppError::SecretManager(format!("Failed to create Secret Manager client: {}", e)))?;

    let name = get_secret(&client, &project_id, SECRET_NAME_SIGNER_NAME).await?;
    let private_key_b64 = get_secret(&client, &project_id, SECRET_NAME_PRIVATE_KEY).await?;

    let private_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(private_key_b64.trim())
        .map_err(|e| AppError::InvalidKey(format!("Failed to decode base64: {}", e)))?;

    let key_array: [u8; 32] = private_key_bytes
        .try_into()
        .map_err(|_| AppError::InvalidKey("Private key must be exactly 32 bytes".to_string()))?;

    let signing_key = SigningKey::from_bytes(&key_array);
    let public_key = signing_key.verifying_key();

    info!("Loaded secrets successfully. Public key: {}", hex::encode(public_key.as_bytes()));

    Ok(AppState {
        name: name.trim().to_string(),
        signing_key,
        public_key,
    })
}

async fn health() -> &'static str {
    "OK"
}

async fn sign(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SignRequest>,
) -> Result<Json<SignResponse>, AppError> {
    let message_bytes = payload.message.as_bytes();
    let signature: Signature = state.signing_key.sign(message_bytes);

    Ok(Json(SignResponse {
        name: state.name.clone(),
        public_key: hex::encode(state.public_key.as_bytes()),
        signature: hex::encode(signature.to_bytes()),
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting signer serverless function...");

    let state = match load_secrets().await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            error!("Failed to load secrets: {}", e);
            std::process::exit(1);
        }
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/sign", post(sign))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
