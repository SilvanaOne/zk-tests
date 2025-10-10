//! Payment execution functionality mirroring make pay-app

use crate::context::ContractBlobsContext;
use crate::url::create_client_with_localhost_resolution;
use chrono::Utc;
use serde_json::json;
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct PaymentArgs {
    pub preapproval_cid: String,
    pub amulet_cid: String,
    pub sender: String,
    pub receiver: String,
    pub cmdid: String,
    pub amulet_rules: String,
    pub open_round: String,
    pub featured_right: String,
    pub amulet_blob: String,
    pub mining_blob: String,
    pub featured_blob: String,
    pub sync_id: String,
    pub amount: String,
    pub description: String,
}

impl PaymentArgs {
    /// Create PaymentArgs from a specific request with custom amount, sender, and description
    pub async fn from_request(
        ctx: ContractBlobsContext,
        amount: f64,
        sender: String,
        description: String,
    ) -> anyhow::Result<Self> {
        // Load environment variables for preapproval and receiver
        let preapproval_cid = std::env::var("APP_TRANSFER_PREAPPROVAL_CID")
            .map_err(|_| anyhow::anyhow!("APP_TRANSFER_PREAPPROVAL_CID not set in environment"))?;
        let receiver = std::env::var("PARTY_APP")
            .map_err(|_| anyhow::anyhow!("PARTY_APP not set in environment"))?;

        // Find an amulet contract for the specified sender
        let amulet_cid = Self::find_amulet(&sender).await?;

        // Extract user party prefix (before ::) for command ID
        let user_party_prefix = sender.split("::").next().unwrap_or("unknown");

        // Extract subscription name from description (format: "{subscription} subscription payment for {user}")
        let subscription_name = description
            .split_whitespace()
            .next()
            .unwrap_or("payment")
            .to_lowercase();

        // Generate unique command ID: pay-{subscription}-{user_party_prefix}-{timestamp}
        let cmdid = format!(
            "pay-{}-{}-{}",
            subscription_name,
            user_party_prefix,
            Utc::now().timestamp()
        );

        Ok(PaymentArgs {
            preapproval_cid,
            amulet_cid,
            sender: sender.clone(),
            receiver,
            cmdid,
            amulet_rules: ctx.amulet_rules_cid,
            open_round: ctx.open_mining_round_cid,
            featured_right: ctx.featured_app_right_cid,
            amulet_blob: ctx.amulet_rules_blob,
            mining_blob: ctx.open_mining_round_blob,
            featured_blob: ctx.featured_app_right_blob,
            sync_id: ctx.synchronizer_id,
            amount: amount.to_string(),
            description,
        })
    }

    /// Find an available Amulet contract for the given party
    pub async fn find_amulet(party: &str) -> anyhow::Result<String> {
        let api_url = std::env::var("APP_PROVIDER_API_URL")
            .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set in environment"))?;
        let jwt = std::env::var("APP_PROVIDER_JWT")
            .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set in environment"))?;

        let client = create_client_with_localhost_resolution()?;

        debug!(party = %party, "Finding Amulet contracts");

        // Get ledger end offset
        let ledger_end_url = format!("{}v2/state/ledger-end", api_url);
        let ledger_end: serde_json::Value = client
            .get(&ledger_end_url)
            .bearer_auth(&jwt)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let offset = ledger_end["offset"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Unable to get ledger end offset"))?;

        // Query active contracts
        let query = json!({
            "activeAtOffset": offset,
            "filter": {
                "filtersByParty": {
                    party: {}
                }
            },
            "verbose": true
        });

        let contracts_url = format!("{}v2/state/active-contracts?limit=500", api_url);
        let contracts: Vec<serde_json::Value> = client
            .post(&contracts_url)
            .bearer_auth(&jwt)
            .json(&query)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // Find Amulet contracts
        for contract in contracts {
            if let Some(entry) = contract.get("contractEntry") {
                if let Some(js_contract) = entry.get("JsActiveContract") {
                    if let Some(created) = js_contract.get("createdEvent") {
                        if let Some(template_id) = created.get("templateId") {
                            if template_id
                                .as_str()
                                .map(|s| s.contains("Splice.Amulet:Amulet"))
                                .unwrap_or(false)
                            {
                                if let Some(contract_id) = created.get("contractId") {
                                    let cid = contract_id.as_str().unwrap_or_default().to_string();

                                    // Log the amount if available
                                    if let Some(amount) = created
                                        .pointer("/createArgument/amount/initialAmount")
                                        .and_then(|v| v.as_str())
                                    {
                                        info!(cid = %cid, amount = %amount, "Found Amulet contract");
                                    }

                                    return Ok(cid);
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "No Amulet contracts found for party {}",
            party
        ))
    }

    /// Execute the payment using TransferPreapproval_Send
    pub async fn execute_payment(&self) -> anyhow::Result<(String, String)> {
        let api_url = std::env::var("APP_PROVIDER_API_URL")
            .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set in environment"))?;
        let jwt = std::env::var("APP_PROVIDER_JWT")
            .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set in environment"))?;

        info!(
            from = %self.sender,
            to = %self.receiver,
            amount = %self.amount,
            description = %self.description,
            "Executing TransferPreapproval_Send"
        );

        let payload = json!({
            "commands": [{
                "ExerciseCommand": {
                    "templateId": "#splice-amulet:Splice.AmuletRules:TransferPreapproval",
                    "contractId": self.preapproval_cid,
                    "choice": "TransferPreapproval_Send",
                    "choiceArgument": {
                        "context": {
                            "amuletRules": self.amulet_rules,
                            "context": {
                                "openMiningRound": self.open_round,
                                "issuingMiningRounds": [],
                                "validatorRights": [],
                                "featuredAppRight": self.featured_right
                            }
                        },
                        "inputs": [{
                            "tag": "InputAmulet",
                            "value": self.amulet_cid
                        }],
                        "amount": self.amount,
                        "sender": self.sender,
                        "description": self.description
                    }
                }
            }],
            "disclosedContracts": [
                {
                    "contractId": self.amulet_rules,
                    "contractIdActual": self.amulet_rules,
                    "blob": self.amulet_blob,
                    "createdEventBlob": self.amulet_blob,
                    "synchronizerId": self.sync_id,
                    "templateId": "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.AmuletRules:AmuletRules"
                },
                {
                    "contractId": self.open_round,
                    "contractIdActual": self.open_round,
                    "blob": self.mining_blob,
                    "createdEventBlob": self.mining_blob,
                    "synchronizerId": self.sync_id,
                    "templateId": "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound"
                },
                {
                    "contractId": self.featured_right,
                    "contractIdActual": self.featured_right,
                    "blob": self.featured_blob,
                    "createdEventBlob": self.featured_blob,
                    "synchronizerId": self.sync_id,
                    "templateId": "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Amulet:FeaturedAppRight"
                }
            ],
            "commandId": self.cmdid,
            "actAs": [self.sender.clone(), self.receiver.clone()],
            "readAs": []
        });

        let client = create_client_with_localhost_resolution()?;
        let submit_url = format!("{}v2/commands/submit-and-wait", api_url);

        let response = client
            .post(&submit_url)
            .bearer_auth(&jwt)
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        let response_body: serde_json::Value = response.json().await?;

        if status.is_success() {
            if let Some(update_id) = response_body.get("updateId").and_then(|v| v.as_str()) {
                info!(
                    amount = %self.amount,
                    from = %self.sender,
                    to = %self.receiver,
                    update_id = %update_id,
                    "Payment successful"
                );
                return Ok((self.cmdid.clone(), update_id.to_string()));
            }
        }

        // Handle error response
        let error = response_body
            .get("cause")
            .or(response_body.get("error"))
            .or(response_body.get("errors"))
            .map(|e| {
                if let Some(str) = e.as_str() {
                    str.to_string()
                } else {
                    serde_json::to_string_pretty(e).unwrap_or_else(|_| e.to_string())
                }
            })
            .unwrap_or_else(|| format!("Unknown error: {:?}", response_body));

        error!(error = %error, status = ?status, "Payment failed");
        Err(anyhow::anyhow!("Payment failed: {}", error))
    }
}
