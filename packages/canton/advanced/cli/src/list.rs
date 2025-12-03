//! List command implementation for viewing amulets.

use anyhow::Result;
use serde_json::json;
use tracing::{debug, info};

use crate::url::create_client_with_localhost_resolution;

/// Amulet information for display
#[derive(Debug)]
pub struct AmuletInfo {
    pub contract_id: String,
    pub amount: String,
    pub is_locked: bool,
    pub lock_holders: Vec<String>,
    pub lock_expires_at: Option<String>,
}

/// Find all Amulet contracts (locked and unlocked) for a specific party
async fn find_amulets(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
) -> Result<Vec<AmuletInfo>> {
    debug!(party = %party, "Finding all Amulet contracts");

    // Get ledger end offset
    let ledger_end_url = format!("{}v2/state/ledger-end", api_url);
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(jwt)
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
        .bearer_auth(jwt)
        .json(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut amulet_data = Vec::new();

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    if let Some(template_id) = created.get("templateId") {
                        if let Some(template_str) = template_id.as_str() {
                            // Check for both Amulet and LockedAmulet
                            let is_amulet = template_str.contains("Splice.Amulet:Amulet");
                            let is_locked_amulet = template_str.contains("Splice.Amulet:LockedAmulet");

                            if is_amulet || is_locked_amulet {
                                let cid = created
                                    .get("contractId")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| anyhow::anyhow!("Missing contractId"))?
                                    .to_string();

                                let amount = created
                                    .pointer("/createArgument/amulet/amount/initialAmount")
                                    .or_else(|| created.pointer("/createArgument/amount/initialAmount"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("0")
                                    .to_string();

                                let is_locked = is_locked_amulet;

                                let mut lock_holders = Vec::new();
                                let mut lock_expires_at = None;

                                if is_locked_amulet {
                                    // Extract lock details
                                    if let Some(holders) = created
                                        .pointer("/createArgument/lock/holders")
                                        .and_then(|v| v.as_array())
                                    {
                                        for holder in holders {
                                            if let Some(h) = holder.as_str() {
                                                lock_holders.push(h.to_string());
                                            }
                                        }
                                    }

                                    lock_expires_at = created
                                        .pointer("/createArgument/lock/expiresAt")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                }

                                amulet_data.push(AmuletInfo {
                                    contract_id: cid,
                                    amount,
                                    is_locked,
                                    lock_holders,
                                    lock_expires_at,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    info!(count = amulet_data.len(), party = %party, "Found amulet contracts");
    Ok(amulet_data)
}

/// Find all AdvancedPayment contracts for a specific party
async fn find_advanced_payments(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    package_id: &str,
) -> Result<Vec<serde_json::Value>> {
    debug!(party = %party, "Finding AdvancedPayment contracts");

    let ledger_end_url = format!("{}v2/state/ledger-end", api_url);
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(jwt)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Unable to get ledger end offset"))?;

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
        .bearer_auth(jwt)
        .json(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut payments = Vec::new();
    let template_pattern = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    if let Some(template_id) = created.get("templateId").and_then(|v| v.as_str()) {
                        if template_id.contains(&template_pattern) {
                            payments.push(created.clone());
                        }
                    }
                }
            }
        }
    }

    info!(count = payments.len(), party = %party, "Found AdvancedPayment contracts");
    Ok(payments)
}

/// AppRewardCoupon information for display
#[derive(Debug)]
pub struct AppRewardCouponInfo {
    pub contract_id: String,
    pub amount: String,
    pub round: String,
}

/// Find all AppRewardCoupon contracts for a specific party (beneficiary)
async fn find_app_reward_coupons(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
) -> Result<Vec<AppRewardCouponInfo>> {
    debug!(party = %party, "Finding AppRewardCoupon contracts");

    let ledger_end_url = format!("{}v2/state/ledger-end", api_url);
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(jwt)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Unable to get ledger end offset"))?;

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
        .bearer_auth(jwt)
        .json(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut coupons = Vec::new();

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    if let Some(template_id) = created.get("templateId").and_then(|v| v.as_str()) {
                        if template_id.contains("Splice.Amulet:AppRewardCoupon") {
                            let cid = created
                                .get("contractId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?")
                                .to_string();

                            // AppRewardCoupon has amount directly at createArgument.amount (Decimal)
                            let amount = created
                                .pointer("/createArgument/amount")
                                .and_then(|v| v.as_str())
                                .unwrap_or("0")
                                .to_string();

                            let round = created
                                .pointer("/createArgument/round/number")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?")
                                .to_string();

                            coupons.push(AppRewardCouponInfo {
                                contract_id: cid,
                                amount,
                                round,
                            });
                        }
                    }
                }
            }
        }
    }

    info!(count = coupons.len(), party = %party, "Found AppRewardCoupon contracts");
    Ok(coupons)
}

/// Find all AdvancedPaymentRequest contracts for a specific party
async fn find_advanced_payment_requests(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    package_id: &str,
) -> Result<Vec<serde_json::Value>> {
    debug!(party = %party, "Finding AdvancedPaymentRequest contracts");

    let ledger_end_url = format!("{}v2/state/ledger-end", api_url);
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(jwt)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Unable to get ledger end offset"))?;

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
        .bearer_auth(jwt)
        .json(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut requests = Vec::new();
    let template_pattern = format!("{}:AdvancedPaymentRequest:AdvancedPaymentRequest", package_id);

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    if let Some(template_id) = created.get("templateId").and_then(|v| v.as_str()) {
                        if template_id.contains(&template_pattern) {
                            requests.push(created.clone());
                        }
                    }
                }
            }
        }
    }

    info!(count = requests.len(), party = %party, "Found AdvancedPaymentRequest contracts");
    Ok(requests)
}

pub async fn handle_list(party: Option<String>) -> Result<()> {
    info!("Listing amulets and advanced payment contracts");

    let party_owner = std::env::var("PARTY_OWNER")
        .map_err(|_| anyhow::anyhow!("PARTY_OWNER not set"))?;
    let party_app =
        std::env::var("PARTY_APP").map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;

    let user_api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set"))?;
    let user_jwt =
        std::env::var("APP_USER_JWT").map_err(|_| anyhow::anyhow!("APP_USER_JWT not set"))?;

    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set"))?;
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set"))?;

    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID").unwrap_or_default();

    let client = create_client_with_localhost_resolution()?;

    let show_user = party.is_none() || party.as_deref() == Some("user") || party.as_deref() == Some("owner");
    let show_app = party.is_none() || party.as_deref() == Some("app");
    let show_provider = party.is_none() || party.as_deref() == Some("provider");

    if show_user {
        println!("\n=== OWNER ({}) ===", party_owner);

        // List amulets
        let user_amulets = find_amulets(&client, &user_api_url, &user_jwt, &party_owner).await?;

        println!("\nAmulets:");
        if user_amulets.is_empty() {
            println!("  (none)");
        } else {
            for amulet in &user_amulets {
                if amulet.is_locked {
                    println!(
                        "  [LOCKED] {} - {} CC",
                        amulet.contract_id, amulet.amount
                    );
                    if !amulet.lock_holders.is_empty() {
                        println!("           Holders: {:?}", amulet.lock_holders);
                    }
                    if let Some(expires) = &amulet.lock_expires_at {
                        println!("           Expires: {}", expires);
                    }
                } else {
                    println!("  {} - {} CC", amulet.contract_id, amulet.amount);
                }
            }
        }

        // List AdvancedPayment contracts
        if !package_id.is_empty() {
            let payments = find_advanced_payments(
                &client,
                &user_api_url,
                &user_jwt,
                &party_owner,
                &package_id,
            )
            .await?;

            println!("\nAdvanced Payments (as owner):");
            if payments.is_empty() {
                println!("  (none)");
            } else {
                for payment in &payments {
                    let cid = payment
                        .get("contractId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let locked_amount = payment
                        .pointer("/createArgument/lockedAmount")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let minimum = payment
                        .pointer("/createArgument/minimumAmount")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let expires = payment
                        .pointer("/createArgument/expiresAt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let provider = payment
                        .pointer("/createArgument/provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");

                    println!("  {} - {} CC locked (min: {} CC)", cid, locked_amount, minimum);
                    println!("    Provider: {}", provider);
                    println!("    Expires: {}", expires);
                }
            }

            // List pending requests
            let requests = find_advanced_payment_requests(
                &client,
                &user_api_url,
                &user_jwt,
                &party_owner,
                &package_id,
            )
            .await?;

            println!("\nPending Payment Requests (to accept/decline):");
            if requests.is_empty() {
                println!("  (none)");
            } else {
                for req in &requests {
                    let cid = req.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let amount = req
                        .pointer("/createArgument/lockedAmount")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let provider = req
                        .pointer("/createArgument/provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");

                    println!("  {} - {} CC requested", cid, amount);
                    println!("    From provider: {}", provider);
                }
            }
        }
    }

    if show_app {
        println!("\n=== APP ({}) ===", party_app);

        // List amulets for app (withdrawals go here)
        let app_amulets =
            find_amulets(&client, &provider_api_url, &provider_jwt, &party_app).await?;

        println!("\nAmulets:");
        if app_amulets.is_empty() {
            println!("  (none)");
        } else {
            for amulet in &app_amulets {
                if amulet.is_locked {
                    println!(
                        "  [LOCKED] {} - {} CC",
                        amulet.contract_id, amulet.amount
                    );
                    if !amulet.lock_holders.is_empty() {
                        println!("           Holders: {:?}", amulet.lock_holders);
                    }
                    if let Some(expires) = &amulet.lock_expires_at {
                        println!("           Expires: {}", expires);
                    }
                } else {
                    println!("  {} - {} CC", amulet.contract_id, amulet.amount);
                }
            }
        }

        // List AdvancedPayment contracts where app is the controller
        if !package_id.is_empty() {
            let payments = find_advanced_payments(
                &client,
                &provider_api_url,
                &provider_jwt,
                &party_app,
                &package_id,
            )
            .await?;

            println!("\nAdvanced Payments (as app controller):");
            if payments.is_empty() {
                println!("  (none)");
            } else {
                for payment in &payments {
                    let cid = payment
                        .get("contractId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let locked_amount = payment
                        .pointer("/createArgument/lockedAmount")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let owner = payment
                        .pointer("/createArgument/owner")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let expires = payment
                        .pointer("/createArgument/expiresAt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");

                    println!("  {} - {} CC locked", cid, locked_amount);
                    println!("    Owner: {}", owner);
                    println!("    Expires: {}", expires);
                }
            }
        }
    }

    if show_provider {
        println!("\n=== PROVIDER ({}) ===", party_provider);

        // List amulets
        let provider_amulets =
            find_amulets(&client, &provider_api_url, &provider_jwt, &party_provider).await?;

        println!("\nAmulets:");
        if provider_amulets.is_empty() {
            println!("  (none)");
        } else {
            for amulet in &provider_amulets {
                if amulet.is_locked {
                    println!(
                        "  [LOCKED] {} - {} CC",
                        amulet.contract_id, amulet.amount
                    );
                    if !amulet.lock_holders.is_empty() {
                        println!("           Holders: {:?}", amulet.lock_holders);
                    }
                    if let Some(expires) = &amulet.lock_expires_at {
                        println!("           Expires: {}", expires);
                    }
                } else {
                    println!("  {} - {} CC", amulet.contract_id, amulet.amount);
                }
            }
        }

        // List AdvancedPayment contracts
        if !package_id.is_empty() {
            let payments = find_advanced_payments(
                &client,
                &provider_api_url,
                &provider_jwt,
                &party_provider,
                &package_id,
            )
            .await?;

            println!("\nAdvanced Payments (as provider):");
            if payments.is_empty() {
                println!("  (none)");
            } else {
                for payment in &payments {
                    let cid = payment
                        .get("contractId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let locked_amount = payment
                        .pointer("/createArgument/lockedAmount")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let owner = payment
                        .pointer("/createArgument/owner")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let expires = payment
                        .pointer("/createArgument/expiresAt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");

                    println!("  {} - {} CC locked", cid, locked_amount);
                    println!("    Owner: {}", owner);
                    println!("    Expires: {}", expires);
                }
            }

            // List outgoing requests
            let requests = find_advanced_payment_requests(
                &client,
                &provider_api_url,
                &provider_jwt,
                &party_provider,
                &package_id,
            )
            .await?;

            println!("\nOutgoing Payment Requests (created by provider):");
            if requests.is_empty() {
                println!("  (none)");
            } else {
                for req in &requests {
                    let cid = req.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let amount = req
                        .pointer("/createArgument/lockedAmount")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let owner = req
                        .pointer("/createArgument/owner")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");

                    println!("  {} - {} CC requested", cid, amount);
                    println!("    To owner: {}", owner);
                }
            }
        }

        // List AppRewardCoupons for provider (earned from app transfers)
        let coupons =
            find_app_reward_coupons(&client, &provider_api_url, &provider_jwt, &party_provider)
                .await?;

        println!("\nApp Reward Coupons:");
        if coupons.is_empty() {
            println!("  (none)");
        } else {
            let mut total_rewards: f64 = 0.0;
            for coupon in &coupons {
                let amount_val: f64 = coupon.amount.parse().unwrap_or(0.0);
                total_rewards += amount_val;
                println!("  {} - {} (round {})", coupon.contract_id, coupon.amount, coupon.round);
            }
            println!("\n  Total unclaimed: {} ({} coupons)", total_rewards, coupons.len());
        }
    }

    Ok(())
}
