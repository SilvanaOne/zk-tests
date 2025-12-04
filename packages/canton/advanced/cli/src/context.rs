//! Data structure for contract blobs context required for AppTransferContext.
//! Fetches AmuletRules, OpenMiningRound, and FeaturedAppRight from Scan API.

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD_ENGINE;
use base64::Engine;
use regex::Regex;
use tracing::debug;

use crate::url::create_client_with_localhost_resolution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractBlobsContext {
    /// Hex-encoded contract ID for AmuletRules
    pub amulet_rules_cid: String,
    /// Hex-encoded contract ID for OpenMiningRound
    pub open_mining_round_cid: String,
    /// Hex-encoded contract ID for FeaturedAppRight
    pub featured_app_right_cid: String,

    /// Base64-encoded blob for AmuletRules disclosure
    pub amulet_rules_blob: String,
    /// Base64-encoded blob for OpenMiningRound disclosure
    pub open_mining_round_blob: String,
    /// Base64-encoded blob for FeaturedAppRight disclosure
    pub featured_app_right_blob: String,

    /// Template ID for AmuletRules
    pub amulet_rules_template_id: String,
    /// Template ID for OpenMiningRound
    pub open_mining_round_template_id: String,
    /// Template ID for FeaturedAppRight
    pub featured_app_right_template_id: String,

    /// Synchronizer identifier
    pub synchronizer_id: String,

    /// DSO party identifier from AmuletRules
    pub dso_party: String,
}

impl ContractBlobsContext {
    pub async fn fetch() -> anyhow::Result<Self> {
        // Load required configuration
        let scan_api_url = std::env::var("SCAN_API_URL")
            .map_err(|_| anyhow::anyhow!("SCAN_API_URL not set in environment"))?;
        let dso_party = std::env::var("PARTY_DSO")
            .map_err(|_| anyhow::anyhow!("PARTY_DSO not set in environment"))?;
        let party_seller = std::env::var("PARTY_PROVIDER").ok();
        let provider_hint = party_seller
            .as_deref()
            .map(|s| s.split("::").next().unwrap_or(s).to_string());

        // Build client with localhost resolution
        let client = create_client_with_localhost_resolution()?;

        // Step 1: GET /v0/dso -> AmuletRules and latest mining round
        let dso_url = format!("{}v0/dso", scan_api_url);
        let dso: serde_json::Value = client
            .get(&dso_url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let amulet_rules_blob = dso
            .pointer("/amulet_rules/contract/created_event_blob")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let amulet_rules_cid = dso
            .pointer("/amulet_rules/contract/contract_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let amulet_rules_template_id = dso
            .pointer("/amulet_rules/contract/template_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let latest_mining_blob = dso
            .pointer("/latest_mining_round/contract/created_event_blob")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let latest_mining_cid = dso
            .pointer("/latest_mining_round/contract/contract_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let latest_mining_template_id = dso
            .pointer("/latest_mining_round/contract/template_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Step 2: POST /v0/open-and-issuing-mining-rounds -> find current open round
        let rounds_url = format!("{}v0/open-and-issuing-mining-rounds", scan_api_url);
        let rounds_body = serde_json::json!({
            "cached_open_mining_round_contract_ids": [],
            "cached_issuing_round_contract_ids": []
        });
        let rounds_resp: serde_json::Value = client
            .post(&rounds_url)
            .json(&rounds_body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .unwrap_or_else(|_| serde_json::json!({}));

        // Determine current open round CID by sorting open_mining_rounds by payload.opensAt
        let (open_round_cid, open_round_blob, open_round_template_id) = if let Some(obj) =
            rounds_resp
                .get("open_mining_rounds")
                .and_then(|v| v.as_object())
        {
            let mut entries: Vec<(&String, &serde_json::Value)> = obj.iter().collect();

            debug!(count = entries.len(), "Found open mining rounds");

            entries.sort_by_key(|(_, v)| {
                v.pointer("/contract/payload/opensAt")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            });

            if let Some((_, first)) = entries.first() {
                let cid = first
                    .pointer("/contract/contract_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let blob = first
                    .pointer("/contract/created_event_blob")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let template_id = first
                    .pointer("/contract/template_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                (cid, blob, template_id)
            } else {
                (String::new(), String::new(), String::new())
            }
        } else {
            debug!("No open_mining_rounds found in response");
            (String::new(), String::new(), String::new())
        };

        let open_mining_round_cid = if !open_round_cid.is_empty() {
            debug!(cid = %open_round_cid, "Using selected open round CID");
            open_round_cid
        } else {
            debug!(cid = %latest_mining_cid, "Falling back to latest mining round CID");
            latest_mining_cid
        };
        let open_mining_round_blob = if !open_round_blob.is_empty() {
            open_round_blob
        } else {
            latest_mining_blob
        };
        let open_mining_round_template_id = if !open_round_template_id.is_empty() {
            open_round_template_id
        } else {
            latest_mining_template_id
        };

        // Step 3: GET /v0/featured-apps -> FeaturedAppRight
        let featured_url = format!("{}v0/featured-apps", scan_api_url);
        let featured: serde_json::Value = client
            .get(&featured_url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let featured_apps = featured
            .get("featured_apps")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let (featured_app_right_blob, featured_app_right_cid, featured_app_right_template_id) =
            if let Some(app_hint) = provider_hint.as_deref() {
                let mut found_blob = String::new();
                let mut found_cid = String::new();
                let mut found_template_id = String::new();
                for entry in featured_apps.iter() {
                    let provider = entry
                        .pointer("/payload/provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if provider.contains(app_hint) {
                        found_blob = entry
                            .pointer("/created_event_blob")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        found_cid = entry
                            .pointer("/contract_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        found_template_id = entry
                            .pointer("/template_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        break;
                    }
                }
                if found_cid.is_empty() && !featured_apps.is_empty() {
                    let first = &featured_apps[0];
                    (
                        first
                            .pointer("/created_event_blob")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        first
                            .pointer("/contract_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        first
                            .pointer("/template_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    )
                } else {
                    (found_blob, found_cid, found_template_id)
                }
            } else if let Some(first) = featured_apps.first() {
                (
                    first
                        .pointer("/created_event_blob")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    first
                        .pointer("/contract_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    first
                        .pointer("/template_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                )
            } else {
                (String::new(), String::new(), String::new())
            };

        // Step 4: Extract synchronizer id from amulet_rules_blob if available
        let synchronizer_id = if !amulet_rules_blob.is_empty() {
            match BASE64_STANDARD_ENGINE.decode(&amulet_rules_blob) {
                Ok(decoded) => {
                    let text = String::from_utf8_lossy(&decoded);
                    let re: Regex = Regex::new(r"global-domain::[a-f0-9]+")?;
                    re.find(&text)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default()
                }
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };

        Ok(ContractBlobsContext {
            amulet_rules_cid,
            open_mining_round_cid,
            featured_app_right_cid,
            amulet_rules_blob,
            open_mining_round_blob,
            featured_app_right_blob,
            amulet_rules_template_id,
            open_mining_round_template_id,
            featured_app_right_template_id,
            synchronizer_id,
            dso_party,
        })
    }

    /// Build the AppTransferContext JSON for choice arguments
    pub fn build_app_transfer_context(&self) -> serde_json::Value {
        serde_json::json!({
            "amuletRules": self.amulet_rules_cid,
            "openMiningRound": self.open_mining_round_cid,
            "featuredAppRight": if self.featured_app_right_cid.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(self.featured_app_right_cid)
            }
        })
    }

    /// Build disclosed contracts array for command submission
    pub fn build_disclosed_contracts(&self) -> Vec<serde_json::Value> {
        let mut contracts = vec![
            serde_json::json!({
                "contractId": self.amulet_rules_cid,
                "createdEventBlob": self.amulet_rules_blob,
                "synchronizerId": self.synchronizer_id,
                "templateId": self.amulet_rules_template_id
            }),
            serde_json::json!({
                "contractId": self.open_mining_round_cid,
                "createdEventBlob": self.open_mining_round_blob,
                "synchronizerId": self.synchronizer_id,
                "templateId": self.open_mining_round_template_id
            }),
        ];

        if !self.featured_app_right_cid.is_empty() {
            contracts.push(serde_json::json!({
                "contractId": self.featured_app_right_cid,
                "createdEventBlob": self.featured_app_right_blob,
                "synchronizerId": self.synchronizer_id,
                "templateId": self.featured_app_right_template_id
            }));
        }

        contracts
    }
}
