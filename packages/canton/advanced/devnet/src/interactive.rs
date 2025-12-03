//! Interactive submission helpers for external party transactions.
//!
//! Implements the prepare → sign → execute flow required for external parties.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::signing::{extract_user_id_from_jwt, get_fingerprint, sign_transaction_hash};

/// Result of preparing a transaction for interactive submission.
#[derive(Debug)]
pub struct PreparedTransaction {
    pub prepared_transaction: String,
    pub prepared_transaction_hash: String,
    pub hashing_scheme_version: String,
}

/// Result of executing a transaction via interactive submission.
#[derive(Debug, Clone)]
pub struct SubmissionResult {
    /// The submission ID (our generated UUID)
    pub submission_id: String,
    /// The update ID returned by the ledger
    pub update_id: String,
}

/// Prepare a transaction via interactive submission.
///
/// This is step 1 of the interactive submission flow.
/// Returns the prepared transaction and hash that needs to be signed.
pub async fn prepare_transaction(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    synchronizer_id: &str,
    commands: Vec<Value>,
    disclosed_contracts: Vec<Value>,
) -> Result<PreparedTransaction> {
    let user_id = extract_user_id_from_jwt(jwt)?;
    let command_id = format!("cmd-{}", chrono::Utc::now().timestamp_millis());

    debug!(
        user_id = %user_id,
        party = %party,
        command_id = %command_id,
        "Preparing interactive submission"
    );

    let payload = json!({
        "userId": user_id,
        "commandId": command_id,
        "actAs": [party],
        "readAs": [party],
        "synchronizerId": synchronizer_id,
        "packageIdSelectionPreference": [],
        "verboseHashing": false,
        "commands": commands,
        "disclosedContracts": disclosed_contracts
    });

    debug!("Prepare payload: {}", serde_json::to_string_pretty(&payload)?);

    let url = format!("{}/interactive-submission/prepare", api_url);
    let response = client
        .post(&url)
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow!(
            "Prepare submission failed (HTTP {}): {}",
            status,
            text
        ));
    }

    let body: Value = serde_json::from_str(&text)
        .map_err(|e| anyhow!("Failed to parse prepare response: {} - {}", e, text))?;

    let prepared_transaction = body
        .get("preparedTransaction")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing preparedTransaction in response"))?
        .to_string();

    let prepared_transaction_hash = body
        .get("preparedTransactionHash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing preparedTransactionHash in response"))?
        .to_string();

    let hashing_scheme_version = body
        .get("hashingSchemeVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("HASHING_SCHEME_VERSION_V2")
        .to_string();

    info!(
        hash = %prepared_transaction_hash,
        scheme = %hashing_scheme_version,
        "Transaction prepared successfully"
    );

    Ok(PreparedTransaction {
        prepared_transaction,
        prepared_transaction_hash,
        hashing_scheme_version,
    })
}

/// Execute a signed transaction via interactive submission.
///
/// This is step 2 of the interactive submission flow.
/// Signs the prepared transaction hash and submits with party signatures.
pub async fn execute_transaction(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    prepared: &PreparedTransaction,
    party: &str,
    private_key_seed: &[u8; 32],
) -> Result<SubmissionResult> {
    let user_id = extract_user_id_from_jwt(jwt)?;
    let fingerprint = get_fingerprint(party)?;
    let signature = sign_transaction_hash(private_key_seed, &prepared.prepared_transaction_hash)?;
    let submission_id = format!("submit-{}", uuid::Uuid::new_v4());

    debug!(
        fingerprint = %fingerprint,
        submission_id = %submission_id,
        "Executing signed transaction"
    );

    let payload = json!({
        "preparedTransaction": prepared.prepared_transaction,
        "partySignatures": {
            "signatures": [{
                "party": party,
                "signatures": [{
                    "format": "SIGNATURE_FORMAT_CONCAT",
                    "signature": signature,
                    "signedBy": fingerprint,
                    "signingAlgorithmSpec": "SIGNING_ALGORITHM_SPEC_ED25519"
                }]
            }]
        },
        "submissionId": &submission_id,
        "userId": user_id,
        "hashingSchemeVersion": prepared.hashing_scheme_version,
        "deduplicationPeriod": {
            "DeduplicationDuration": {
                "value": "PT60S"
            }
        }
    });

    debug!("Execute payload: {}", serde_json::to_string_pretty(&payload)?);

    let url = format!("{}/interactive-submission/execute", api_url);
    let response = client
        .post(&url)
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    info!(
        status = %status,
        response = %text,
        "Execute submission response"
    );

    if !status.is_success() {
        return Err(anyhow!(
            "Execute submission failed (HTTP {}): {}",
            status,
            text
        ));
    }

    // Parse response to verify it's valid JSON (even though ExecuteSubmissionResponse is empty)
    let _body: Value = serde_json::from_str(&text)
        .map_err(|e| anyhow!("Failed to parse execute response: {} - {}", e, text))?;

    // Execute response is empty per OpenAPI spec, so we need to poll completions
    // to get the real updateId
    let update_id = get_completion_update_id(
        client,
        api_url,
        jwt,
        &user_id,
        party,
        &submission_id,
    )
    .await?;

    info!(
        submission_id = %submission_id,
        update_id = %update_id,
        "Transaction executed successfully"
    );

    Ok(SubmissionResult {
        submission_id,
        update_id,
    })
}

/// Poll completions endpoint to get the real update_id for a submission
async fn get_completion_update_id(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    user_id: &str,
    party: &str,
    submission_id: &str,
) -> Result<String> {
    let payload = json!({
        "userId": user_id,
        "parties": [party],
        "beginExclusive": "0"
    });

    info!(
        user_id = %user_id,
        party = %party,
        submission_id = %submission_id,
        "Polling completions for update_id"
    );

    // Retry a few times with delay
    for attempt in 0..10 {
        let response = client
            .post(&format!("{}commands/completions", api_url))
            .bearer_auth(jwt)
            .query(&[("stream_idle_timeout_ms", "2000")])
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            info!(attempt = attempt, status = %status, "Completions request failed");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            continue;
        }

        let text = response.text().await?;
        info!(attempt = attempt, response = %text, "Completions response");

        let completions: Vec<Value> = serde_json::from_str(&text).unwrap_or_default();

        // Find completion matching our submission_id
        for completion in &completions {
            // Format: {"completionResponse": {"Completion": {"value": {...}}}}
            if let Some(comp) = completion
                .get("completionResponse")
                .and_then(|cr| cr.get("Completion"))
                .and_then(|c| c.get("value"))
            {
                let sid = comp.get("submissionId").and_then(|v| v.as_str());
                if sid == Some(submission_id) {
                    if let Some(update_id) = comp.get("updateId").and_then(|v| v.as_str()) {
                        info!(
                            submission_id = %submission_id,
                            update_id = %update_id,
                            "Found completion with updateId"
                        );
                        return Ok(update_id.to_string());
                    }
                }
            }
        }

        // Wait before retry
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Fallback to submission_id if not found
    debug!(
        submission_id = %submission_id,
        "Could not find completion, using submission_id as fallback"
    );
    Ok(submission_id.to_string())
}

/// Prepare and execute a transaction in one call.
///
/// Convenience function that combines prepare_transaction and execute_transaction.
pub async fn submit_interactive(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    synchronizer_id: &str,
    private_key_seed: &[u8; 32],
    commands: Vec<Value>,
    disclosed_contracts: Vec<Value>,
) -> Result<SubmissionResult> {
    // Step 1: Prepare
    let prepared = prepare_transaction(
        client,
        api_url,
        jwt,
        party,
        synchronizer_id,
        commands,
        disclosed_contracts,
    )
    .await?;

    // Step 2: Execute with signature
    execute_transaction(client, api_url, jwt, &prepared, party, private_key_seed).await
}
