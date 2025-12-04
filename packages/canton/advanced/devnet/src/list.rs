//! List command implementation for viewing amulets on devnet.

use anyhow::Result;
use serde_json::json;
use tracing::{debug, info};

use crate::context::create_client;

/// Build a template filter query for the active-contracts API
fn build_template_filter_query(
    party: &str,
    template_ids: &[&str],
    offset: u64,
) -> serde_json::Value {
    let cumulative: Vec<serde_json::Value> = template_ids
        .iter()
        .map(|tid| {
            json!({
                "identifierFilter": {
                    "TemplateFilter": {
                        "value": {
                            "templateId": tid,
                            "includeCreatedEventBlob": false
                        }
                    }
                }
            })
        })
        .collect();

    // Build filters_by_party with the party as a dynamic key
    let mut filters_by_party = serde_json::Map::new();
    filters_by_party.insert(
        party.to_string(),
        json!({
            "cumulative": cumulative
        }),
    );

    json!({
        "activeAtOffset": offset,
        "filter": {
            "filtersByParty": filters_by_party
        },
        "verbose": true
    })
}

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
    let ledger_end_url = format!("{}/state/ledger-end", api_url);
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

    // Query active contracts with template filter for Amulet and LockedAmulet
    let query = build_template_filter_query(
        party,
        &[
            "#splice-amulet:Splice.Amulet:Amulet",
            "#splice-amulet:Splice.Amulet:LockedAmulet",
        ],
        offset,
    );

    let contracts_url = format!("{}/state/active-contracts", api_url);
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
                            // Determine if locked based on template
                            let is_locked_amulet = template_str.contains("LockedAmulet");

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
                                is_locked: is_locked_amulet,
                                lock_holders,
                                lock_expires_at,
                            });
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
    package_name: &str,
) -> Result<Vec<serde_json::Value>> {
    debug!(party = %party, "Finding AdvancedPayment contracts");

    let ledger_end_url = format!("{}/state/ledger-end", api_url);
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

    // Use package name format for template filter
    let template_id = format!("#{}:AdvancedPayment:AdvancedPayment", package_name);
    let query = build_template_filter_query(party, &[&template_id], offset);

    let contracts_url = format!("{}/state/active-contracts", api_url);
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

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    payments.push(created.clone());
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

    let ledger_end_url = format!("{}/state/ledger-end", api_url);
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

    // Use template filter for AppRewardCoupon
    let query = build_template_filter_query(
        party,
        &["#splice-amulet:Splice.Amulet:AppRewardCoupon"],
        offset,
    );

    let contracts_url = format!("{}/state/active-contracts", api_url);
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

    info!(count = coupons.len(), party = %party, "Found AppRewardCoupon contracts");
    Ok(coupons)
}

/// Find all AdvancedPaymentRequest contracts for a specific party
async fn find_advanced_payment_requests(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    package_name: &str,
) -> Result<Vec<serde_json::Value>> {
    debug!(party = %party, "Finding AdvancedPaymentRequest contracts");

    let ledger_end_url = format!("{}/state/ledger-end", api_url);
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

    // Use package name format for template filter
    let template_id = format!("#{}:AdvancedPaymentRequest:AdvancedPaymentRequest", package_name);
    let query = build_template_filter_query(party, &[&template_id], offset);

    let contracts_url = format!("{}/state/active-contracts", api_url);
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

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    requests.push(created.clone());
                }
            }
        }
    }

    info!(count = requests.len(), party = %party, "Found AdvancedPaymentRequest contracts");
    Ok(requests)
}

/// Find all AppService contracts for a specific party
async fn find_app_services(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    package_name: &str,
) -> Result<Vec<serde_json::Value>> {
    debug!(party = %party, "Finding AppService contracts");

    let ledger_end_url = format!("{}/state/ledger-end", api_url);
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

    // Use package name format for template filter
    let template_id = format!("#{}:AppService:AppService", package_name);
    let query = build_template_filter_query(party, &[&template_id], offset);

    let contracts_url = format!("{}/state/active-contracts", api_url);
    let contracts: Vec<serde_json::Value> = client
        .post(&contracts_url)
        .bearer_auth(jwt)
        .json(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut services = Vec::new();

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    services.push(created.clone());
                }
            }
        }
    }

    info!(count = services.len(), party = %party, "Found AppService contracts");
    Ok(services)
}

/// Find all AppServiceRequest contracts for a specific party
async fn find_app_service_requests(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    package_name: &str,
) -> Result<Vec<serde_json::Value>> {
    debug!(party = %party, "Finding AppServiceRequest contracts");

    let ledger_end_url = format!("{}/state/ledger-end", api_url);
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

    // Use package name format for template filter
    let template_id = format!("#{}:AppServiceRequest:AppServiceRequest", package_name);
    let query = build_template_filter_query(party, &[&template_id], offset);

    let contracts_url = format!("{}/state/active-contracts", api_url);
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

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    requests.push(created.clone());
                }
            }
        }
    }

    info!(count = requests.len(), party = %party, "Found AppServiceRequest contracts");
    Ok(requests)
}

pub async fn handle_list(party: Option<String>, user_party: Option<String>) -> Result<()> {
    info!("Listing amulets and advanced payment contracts (devnet)");

    // Devnet uses single API endpoint and JWT for all parties
    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_seller = std::env::var("PARTY_SELLER")
        .map_err(|_| anyhow::anyhow!("PARTY_SELLER not set"))?;
    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;

    let package_name = std::env::var("ADVANCED_PAYMENT_PACKAGE_NAME").unwrap_or_default();

    let client = create_client()?;

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    // Show user section only if user_party is provided (or explicitly requested with --party user)
    let show_user = user_party.is_some() || party.as_deref() == Some("user");
    let show_app = party.is_none() || party.as_deref() == Some("app");
    let show_provider = party.is_none() || party.as_deref() == Some("provider");

    if show_user {
        // For user section, we need user_party to be set
        let party_buyer = match &user_party {
            Some(p) => p.clone(),
            None => {
                return Err(anyhow::anyhow!(
                    "User party ID required. Use --user-party <PARTY_ID> to specify the user."
                ));
            }
        };

        println!("\n=== BUYER ({}) ===", party_buyer);

        // List amulets
        let user_amulets = find_amulets(&client, &api_url, &jwt, &party_buyer).await?;

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
        if !package_name.is_empty() {
            let payments = find_advanced_payments(
                &client,
                &api_url,
                &jwt,
                &party_buyer,
                &package_name,
            )
            .await?;

            println!("\nAdvanced Payments (as buyer):");
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

                    let description = payment
                        .pointer("/createArgument/description")
                        .and_then(|v| v.as_str());
                    let reference = payment
                        .pointer("/createArgument/reference")
                        .and_then(|v| v.as_str());

                    println!("  {} - {} CC locked (min: {} CC)", cid, locked_amount, minimum);
                    println!("    Provider: {}", provider);
                    println!("    Expires: {}", expires);
                    if let Some(desc) = description {
                        println!("    Description: {}", desc);
                    }
                    if let Some(ref_num) = reference {
                        println!("    Reference: {}", ref_num);
                    }
                }
            }

            // List pending requests
            let requests = find_advanced_payment_requests(
                &client,
                &api_url,
                &jwt,
                &party_buyer,
                &package_name,
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
                    let description = req
                        .pointer("/createArgument/description")
                        .and_then(|v| v.as_str());
                    let reference = req
                        .pointer("/createArgument/reference")
                        .and_then(|v| v.as_str());

                    println!("  {} - {} CC requested", cid, amount);
                    println!("    From provider: {}", provider);
                    if let Some(desc) = description {
                        println!("    Description: {}", desc);
                    }
                    if let Some(ref_num) = reference {
                        println!("    Reference: {}", ref_num);
                    }
                }
            }
        }
    }

    if show_app {
        println!("\n=== SELLER ({}) ===", party_seller);

        // List amulets for app (withdrawals go here)
        let app_amulets = find_amulets(&client, &api_url, &jwt, &party_seller).await?;

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
        if !package_name.is_empty() {
            let payments = find_advanced_payments(
                &client,
                &api_url,
                &jwt,
                &party_seller,
                &package_name,
            )
            .await?;

            println!("\nAdvanced Payments (as seller):");
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
                    let buyer = payment
                        .pointer("/createArgument/buyer")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let expires = payment
                        .pointer("/createArgument/expiresAt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let description = payment
                        .pointer("/createArgument/description")
                        .and_then(|v| v.as_str());
                    let reference = payment
                        .pointer("/createArgument/reference")
                        .and_then(|v| v.as_str());

                    println!("  {} - {} CC locked", cid, locked_amount);
                    println!("    Buyer: {}", buyer);
                    println!("    Expires: {}", expires);
                    if let Some(desc) = description {
                        println!("    Description: {}", desc);
                    }
                    if let Some(ref_num) = reference {
                        println!("    Reference: {}", ref_num);
                    }
                }
            }

            // List AppService contracts
            let services = find_app_services(
                &client,
                &api_url,
                &jwt,
                &party_seller,
                &package_name,
            )
            .await?;

            println!("\nApp Services:");
            if services.is_empty() {
                println!("  (none)");
            } else {
                for svc in &services {
                    let cid = svc.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let provider = svc
                        .pointer("/createArgument/provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let svc_desc = svc
                        .pointer("/createArgument/serviceDescription")
                        .and_then(|v| v.as_str());

                    println!("  {}", cid);
                    println!("    Provider: {}", provider);
                    if let Some(desc) = svc_desc {
                        println!("    Description: {}", desc);
                    }
                }
            }

            // List AppServiceRequest contracts
            let svc_requests = find_app_service_requests(
                &client,
                &api_url,
                &jwt,
                &party_seller,
                &package_name,
            )
            .await?;

            println!("\nPending App Service Requests:");
            if svc_requests.is_empty() {
                println!("  (none)");
            } else {
                for req in &svc_requests {
                    let cid = req.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let provider = req
                        .pointer("/createArgument/provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let svc_desc = req
                        .pointer("/createArgument/serviceDescription")
                        .and_then(|v| v.as_str());

                    println!("  {}", cid);
                    println!("    Provider: {}", provider);
                    if let Some(desc) = svc_desc {
                        println!("    Description: {}", desc);
                    }
                }
            }
        }
    }

    if show_provider {
        println!("\n=== PROVIDER ({}) ===", party_provider);

        // List amulets
        let provider_amulets =
            find_amulets(&client, &api_url, &jwt, &party_provider).await?;

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
        if !package_name.is_empty() {
            let payments = find_advanced_payments(
                &client,
                &api_url,
                &jwt,
                &party_provider,
                &package_name,
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
                    let buyer = payment
                        .pointer("/createArgument/buyer")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let expires = payment
                        .pointer("/createArgument/expiresAt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let description = payment
                        .pointer("/createArgument/description")
                        .and_then(|v| v.as_str());
                    let reference = payment
                        .pointer("/createArgument/reference")
                        .and_then(|v| v.as_str());

                    println!("  {} - {} CC locked", cid, locked_amount);
                    println!("    Buyer: {}", buyer);
                    println!("    Expires: {}", expires);
                    if let Some(desc) = description {
                        println!("    Description: {}", desc);
                    }
                    if let Some(ref_num) = reference {
                        println!("    Reference: {}", ref_num);
                    }
                }
            }

            // List outgoing requests
            let requests = find_advanced_payment_requests(
                &client,
                &api_url,
                &jwt,
                &party_provider,
                &package_name,
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
                    let buyer = req
                        .pointer("/createArgument/buyer")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let description = req
                        .pointer("/createArgument/description")
                        .and_then(|v| v.as_str());
                    let reference = req
                        .pointer("/createArgument/reference")
                        .and_then(|v| v.as_str());

                    println!("  {} - {} CC requested", cid, amount);
                    println!("    To buyer: {}", buyer);
                    if let Some(desc) = description {
                        println!("    Description: {}", desc);
                    }
                    if let Some(ref_num) = reference {
                        println!("    Reference: {}", ref_num);
                    }
                }
            }

            // List AppService contracts for provider
            let services = find_app_services(
                &client,
                &api_url,
                &jwt,
                &party_provider,
                &package_name,
            )
            .await?;

            println!("\nApp Services:");
            if services.is_empty() {
                println!("  (none)");
            } else {
                for svc in &services {
                    let cid = svc.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let seller = svc
                        .pointer("/createArgument/seller")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let svc_desc = svc
                        .pointer("/createArgument/serviceDescription")
                        .and_then(|v| v.as_str());

                    println!("  {}", cid);
                    println!("    Seller: {}", seller);
                    if let Some(desc) = svc_desc {
                        println!("    Description: {}", desc);
                    }
                }
            }

            // List AppServiceRequest contracts for provider (to accept/reject)
            let svc_requests = find_app_service_requests(
                &client,
                &api_url,
                &jwt,
                &party_provider,
                &package_name,
            )
            .await?;

            println!("\nPending App Service Requests (to accept/reject):");
            if svc_requests.is_empty() {
                println!("  (none)");
            } else {
                for req in &svc_requests {
                    let cid = req.get("contractId").and_then(|v| v.as_str()).unwrap_or("?");
                    let seller = req
                        .pointer("/createArgument/seller")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let svc_desc = req
                        .pointer("/createArgument/serviceDescription")
                        .and_then(|v| v.as_str());

                    println!("  {}", cid);
                    println!("    Seller: {}", seller);
                    if let Some(desc) = svc_desc {
                        println!("    Description: {}", desc);
                    }
                }
            }
        }

        // List AppRewardCoupons for provider (earned from app transfers)
        let coupons =
            find_app_reward_coupons(&client, &api_url, &jwt, &party_provider).await?;

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
