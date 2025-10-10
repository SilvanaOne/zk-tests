//! CLI module for the billing application

use crate::{context::ContractBlobsContext, pay::PaymentArgs, subscriptions, users};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

#[derive(Parser)]
#[command(name = "billing")]
#[command(about = "Silvana Billing CLI - Subscription and Payment Management on Canton", long_about = None)]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    /// Log level (trace, debug, info, warn, error)
    #[arg(long, global = true, env = "LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all available subscriptions
    Subscriptions,

    /// Manage users
    Users {
        #[command(subcommand)]
        subcommand: Option<UserCommands>,
    },

    /// Find user by email, name, or party substring
    User {
        /// Search query (matches email, name, or party)
        query: String,
    },

    /// Make a payment for a specific subscription and user
    Pay {
        /// Subscription name (e.g., "prover", "verifier")
        #[arg(long, short = 's')]
        subscription: String,

        /// User identifier (email, name, or party substring)
        #[arg(long, short = 'u')]
        user: String,

        /// Dry run - simulate payment without executing
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },

    /// Start automated payments for all users and subscriptions
    Start {
        /// Run once instead of continuous loop
        #[arg(long, default_value = "false")]
        once: bool,

        /// Override check interval in seconds (default: 60)
        #[arg(long, env = "CHECK_INTERVAL_SECS", default_value = "60")]
        interval: u64,

        /// Dry run - simulate payments without executing
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },

    /// Get details of a transaction update by ID
    Update {
        /// The update ID to fetch
        update_id: String,
    },

    /// Setup TransferPreapproval for the app
    Setup {
        /// Expiration time in minutes (default: 525600 = 1 year)
        #[arg(long, default_value = "525600")]
        expires_in_min: u64,

        /// Subscriber party (sender of funds)
        #[arg(long, env = "SUBSCRIBER_PARTY")]
        subscriber_party: String,
    },
}

#[derive(Subcommand)]
pub enum UserCommands {
    /// List all users with their subscriptions
    List,

    /// List users with a specific subscription
    WithSubscription {
        /// Subscription name
        name: String,
    },
}

/// Find a user by a flexible query (email, name, or party substring)
fn find_user_by_query(query: &str) -> anyhow::Result<&'static users::User> {
    let users_list = users::get_users();
    let query_lower = query.to_lowercase();

    // Try exact matches first
    if let Some(user) = users::find_user_by_email(query) {
        return Ok(user);
    }

    // Then try partial matches
    for user in users_list {
        if user.email.to_lowercase().contains(&query_lower)
            || user.name.to_lowercase().contains(&query_lower)
            || user.party.to_lowercase().contains(&query_lower)
        {
            return Ok(user);
        }
    }

    Err(anyhow::anyhow!("No user found matching query: {}", query))
}

/// Handle the subscriptions command
fn handle_subscriptions() -> anyhow::Result<()> {
    info!("Listing available subscriptions");
    subscriptions::list_subscriptions();
    Ok(())
}

/// Handle the users command
fn handle_users(subcommand: Option<UserCommands>) -> anyhow::Result<()> {
    match subcommand {
        None | Some(UserCommands::List) => {
            info!("Listing all users");
            users::list_users();
        }
        Some(UserCommands::WithSubscription { name }) => {
            info!(subscription = %name, "Listing users with subscription");
            users::list_users_with_subscription(&name);
        }
    }
    Ok(())
}

/// Handle the user search command
fn handle_user_search(query: &str) -> anyhow::Result<()> {
    info!(query = %query, "Searching for user");

    let users_list = users::get_users();
    let query_lower = query.to_lowercase();

    let mut found = Vec::new();

    for user in users_list {
        if user.email.to_lowercase().contains(&query_lower)
            || user.name.to_lowercase().contains(&query_lower)
            || user.party.to_lowercase().contains(&query_lower)
        {
            found.push(user);
        }
    }

    if found.is_empty() {
        warn!("No users found matching query: {}", query);
    } else {
        info!("Found {} user(s) matching query", found.len());
        for user in found {
            info!(
                id = %user.id,
                name = %user.name,
                email = %user.email,
                party = %user.party,
                subscriptions = %user.subscriptions_summary(),
                "User found"
            );
        }
    }

    Ok(())
}

/// Handle the payment command
async fn handle_payment(subscription: &str, user_query: &str, dry_run: bool) -> anyhow::Result<()> {
    // Find user
    let user = find_user_by_query(user_query)?;

    // Verify user has the subscription
    if !user.has_active_subscription(subscription) {
        return Err(anyhow::anyhow!(
            "User {} does not have active '{}' subscription",
            user.name,
            subscription
        ));
    }

    // Find subscription details
    let sub = subscriptions::find_subscription_by_name(subscription)
        .ok_or_else(|| anyhow::anyhow!("Subscription '{}' not found", subscription))?;

    info!(
        user = %user.name,
        subscription = %subscription,
        price = %sub.formatted_price(),
        dry_run = dry_run,
        "Executing payment"
    );

    // Create payment description
    let description = format!("{} subscription payment for {}", subscription, user.name);

    if dry_run {
        info!(
            user = %user.name,
            email = %user.email,
            party = %user.party,
            subscription = %subscription,
            amount = %sub.formatted_price(),
            description = %description,
            "DRY RUN: Would execute payment"
        );
        return Ok(());
    }

    // Execute actual payment
    info!(
        user = %user.name,
        email = %user.email,
        party = %user.party,
        subscription = %subscription,
        amount = %sub.formatted_price(),
        description = %description,
        "Executing payment"
    );

    let ctx = ContractBlobsContext::fetch().await?;

    // Use from_request with specific user's party and subscription details
    let payment_args =
        PaymentArgs::from_request(ctx, sub.price, user.party.clone(), description).await?;

    payment_args.execute_payment().await?;

    Ok(())
}

/// Payment scheduler for automated payments
pub struct PaymentScheduler {
    /// Track last payment time for each user+subscription combination
    last_payments: HashMap<String, Instant>,
    dry_run: bool,
}

impl PaymentScheduler {
    pub fn new(dry_run: bool) -> Self {
        Self {
            last_payments: HashMap::new(),
            dry_run,
        }
    }

    pub async fn run_once(&mut self) -> anyhow::Result<()> {
        info!("Running payment scheduler (single iteration)");
        self.process_all_payments().await
    }

    pub async fn run_continuous(&mut self, check_interval: Duration) -> anyhow::Result<()> {
        info!(?check_interval, "Starting continuous payment scheduler");

        let mut interval = tokio::time::interval(check_interval);
        interval.tick().await; // First tick happens immediately

        loop {
            if let Err(e) = self.process_all_payments().await {
                error!(error = %e, "Error processing payments");
            }

            interval.tick().await;
        }
    }

    async fn process_all_payments(&mut self) -> anyhow::Result<()> {
        let users_list = users::get_users();
        let subscriptions_list = subscriptions::get_subscriptions();

        debug!(
            users = users_list.len(),
            subscriptions = subscriptions_list.len(),
            "Processing payments"
        );

        let now = Instant::now();

        for user in users_list {
            for user_sub in &user.subscriptions {
                if !user_sub.is_active() {
                    debug!(
                        user = %user.name,
                        subscription = %user_sub.name,
                        "Skipping expired subscription"
                    );
                    continue;
                }

                // Find subscription details
                let sub = match subscriptions_list.iter().find(|s| s.name == user_sub.name) {
                    Some(s) => s,
                    None => {
                        warn!(
                            user = %user.name,
                            subscription = %user_sub.name,
                            "Subscription not found in catalog"
                        );
                        continue;
                    }
                };

                // Check if payment is due
                let key = format!("{}::{}", user.party, sub.name);
                let interval_secs = sub.interval_seconds().unwrap_or(300);
                let interval_duration = Duration::from_secs(interval_secs);

                let should_pay = match self.last_payments.get(&key) {
                    Some(last) => now.duration_since(*last) >= interval_duration,
                    None => true, // First payment
                };

                if should_pay {
                    // Create payment description
                    let description =
                        format!("{} subscription payment for {}", sub.name, user.name);

                    info!(
                        user = %user.name,
                        subscription = %sub.name,
                        amount = %sub.formatted_price(),
                        description = %description,
                        interval_secs,
                        dry_run = self.dry_run,
                        "Payment due"
                    );

                    if !self.dry_run {
                        // Execute payment
                        match self.execute_user_payment(user, sub).await {
                            Ok(_) => {
                                self.last_payments.insert(key, now);
                                info!(
                                    user = %user.name,
                                    subscription = %sub.name,
                                    amount = %sub.formatted_price(),
                                    description = %description,
                                    "Payment successful"
                                );
                            }
                            Err(e) => {
                                error!(
                                    user = %user.name,
                                    subscription = %sub.name,
                                    amount = %sub.formatted_price(),
                                    description = %description,
                                    error = %e,
                                    "Payment failed"
                                );
                            }
                        }
                    } else {
                        info!(
                            user = %user.name,
                            subscription = %sub.name,
                            amount = %sub.formatted_price(),
                            description = %description,
                            "DRY RUN: Would execute payment"
                        );
                        self.last_payments.insert(key, now);
                    }

                    // Add small delay between payments
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        Ok(())
    }

    async fn execute_user_payment(
        &self,
        user: &users::User,
        sub: &subscriptions::Subscription,
    ) -> anyhow::Result<()> {
        let ctx = ContractBlobsContext::fetch().await?;

        // Create payment description
        let description = format!("{} subscription payment for {}", sub.name, user.name);

        // Use from_request with specific user's party and subscription details
        let payment_args =
            PaymentArgs::from_request(ctx, sub.price, user.party.clone(), description).await?;

        payment_args.execute_payment().await?;
        Ok(())
    }
}

/// Handle the start command
async fn handle_start(once: bool, interval_secs: u64, dry_run: bool) -> anyhow::Result<()> {
    let mut scheduler = PaymentScheduler::new(dry_run);

    if once {
        info!(dry_run = dry_run, "Running payment scheduler once");
        scheduler.run_once().await
    } else {
        let interval = Duration::from_secs(interval_secs);
        info!(
            interval_secs,
            dry_run = dry_run,
            "Starting continuous payment scheduler"
        );
        scheduler.run_continuous(interval).await
    }
}

/// Handle the get update command
async fn handle_get_update(update_id: &str) -> anyhow::Result<()> {
    use crate::url::create_client_with_localhost_resolution;
    use serde_json::json;

    info!(update_id = %update_id, "Fetching update details");

    // Load environment variables
    let api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set in environment"))?;
    let jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set in environment"))?;
    let party_app = std::env::var("PARTY_APP")
        .map_err(|_| anyhow::anyhow!("PARTY_APP not set in environment"))?;

    debug!(update_id = %update_id, "Building request payload");

    // Build the request payload
    let payload = json!({
        "actAs": [&party_app],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_app: {}
                    },
                    "verbose": true
                },
                "transactionShape": "TRANSACTION_SHAPE_ACS_DELTA"
            }
        }
    });

    // Create HTTP client with localhost resolution
    let client = create_client_with_localhost_resolution()?;

    // Make the request to fetch update details
    let url = format!("{}v2/updates/update-by-id", api_url);
    let response = client
        .post(&url)
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let response_text = response.text().await?;

    if status.is_success() {
        // Try to parse as JSON for pretty printing
        match serde_json::from_str::<serde_json::Value>(&response_text) {
            Ok(json_value) => {
                // Extract and log key information
                if let Some(update) = json_value.pointer("/update/Transaction/value") {
                    let command_id = update
                        .pointer("/commandId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    let effective_at = update
                        .pointer("/effectiveAt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    let events_count = update
                        .pointer("/events")
                        .and_then(|v| v.as_array())
                        .map(|e| e.len())
                        .unwrap_or(0);

                    info!(
                        update_id = %update_id,
                        command_id = %command_id,
                        effective_at = %effective_at,
                        events_count = events_count,
                        "Update fetched successfully"
                    );

                    // Log event details
                    if let Some(events) = update.pointer("/events").and_then(|v| v.as_array()) {
                        for (idx, event) in events.iter().enumerate() {
                            if let Some(created_event) = event.get("CreatedEvent") {
                                if let Some(template_id) = created_event
                                    .pointer("/templateId")
                                    .and_then(|v| v.as_str())
                                {
                                    let template_name =
                                        template_id.split(':').last().unwrap_or(template_id);

                                    let amount = created_event
                                        .pointer("/createArgument/amount")
                                        .and_then(|a| {
                                            a.as_str().or_else(|| {
                                                a.pointer("/initialAmount").and_then(|v| v.as_str())
                                            })
                                        })
                                        .unwrap_or("N/A");

                                    debug!(
                                        event_index = idx,
                                        template = %template_name,
                                        amount = %amount,
                                        "Event details"
                                    );
                                }
                            }
                        }
                    }
                } else {
                    warn!("Update response does not contain expected transaction data");
                }

                // Pretty print the JSON response for user consumption
                // Note: We keep this println as it's for actual data output, not logging
                println!("{}", serde_json::to_string_pretty(&json_value)?);
            }
            Err(e) => {
                // If not JSON, print raw response
                warn!(error = %e, "Failed to parse response as JSON");
                // Note: We keep this println as it's for actual data output, not logging
                println!("{}", response_text);
            }
        }
    } else {
        error!(status = %status, "Failed to fetch update");

        // Try to parse error response
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(error_msg) = error_json.pointer("/error").and_then(|v| v.as_str()) {
                return Err(anyhow::anyhow!("API Error: {}", error_msg));
            }
        }

        return Err(anyhow::anyhow!(
            "Failed to fetch update: HTTP {} - {}",
            status,
            response_text
        ));
    }

    Ok(())
}

/// Handle setup preapproval command
async fn handle_setup_preapproval(expires_in_min: u64, subscriber_party: &str) -> anyhow::Result<()> {
    use crate::context::ContractBlobsContext;
    use crate::pay::PaymentArgs;
    use crate::url::create_client_with_localhost_resolution;
    use chrono::{Duration, Utc};
    use serde_json::json;

    info!("Setting up TransferPreapproval for the app");

    // Load contract blobs
    let ctx = ContractBlobsContext::fetch().await?;
    info!("Contract blobs loaded successfully");

    // Load environment variables
    let api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set in environment"))?;
    let jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set in environment"))?;
    let party_app = std::env::var("PARTY_APP")
        .map_err(|_| anyhow::anyhow!("PARTY_APP not set in environment"))?;

    // Calculate DSO party from synchronizer ID
    let dso_party = format!("DSO::{}", ctx.synchronizer_id.replace("global-domain::", ""));

    info!("Finding PARTY_APP Amulet contracts for preapproval fee");

    // Find an amulet for the app party to pay the fee
    let app_amulet = PaymentArgs::find_amulet(&party_app).await?;
    info!(amulet_cid = %app_amulet, "Found Amulet for fee payment");

    // Calculate expiration time
    let current_time = Utc::now();
    let expires_at = current_time + Duration::minutes(expires_in_min as i64);

    info!(
        current = %current_time.format("%Y-%m-%dT%H:%M:%SZ"),
        expires = %expires_at.format("%Y-%m-%dT%H:%M:%SZ"),
        minutes = expires_in_min,
        "Creating preapproval"
    );

    // Generate command ID
    let cmdid = format!("setup-preapproval-{}", current_time.timestamp());

    // Build the request payload for creating TransferPreapproval
    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.AmuletRules:AmuletRules",
                "contractId": ctx.amulet_rules_cid,
                "choice": "AmuletRules_CreateTransferPreapproval",
                "choiceArgument": {
                    "context": {
                        "amuletRules": ctx.amulet_rules_cid,
                        "context": {
                            "openMiningRound": ctx.open_mining_round_cid,
                            "issuingMiningRounds": [],
                            "validatorRights": [],
                            "featuredAppRight": null
                        }
                    },
                    "inputs": [{"tag": "InputAmulet", "value": app_amulet}],
                    "receiver": party_app.clone(),
                    "provider": party_app.clone(),
                    "expiresAt": expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                    "expectedDso": dso_party
                }
            }
        }],
        "disclosedContracts": [
            {
                "contractId": ctx.amulet_rules_cid.clone(),
                "contractIdActual": ctx.amulet_rules_cid.clone(),
                "blob": ctx.amulet_rules_blob.clone(),
                "createdEventBlob": ctx.amulet_rules_blob.clone(),
                "synchronizerId": ctx.synchronizer_id.clone(),
                "templateId": "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.AmuletRules:AmuletRules"
            },
            {
                "contractId": ctx.open_mining_round_cid.clone(),
                "contractIdActual": ctx.open_mining_round_cid.clone(),
                "blob": ctx.open_mining_round_blob.clone(),
                "createdEventBlob": ctx.open_mining_round_blob.clone(),
                "synchronizerId": ctx.synchronizer_id.clone(),
                "templateId": "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound"
            }
        ],
        "commandId": cmdid,
        "actAs": [subscriber_party, &party_app],
        "readAs": []
    });

    // Create HTTP client and execute the request
    let client = create_client_with_localhost_resolution()?;
    let url = format!("{}v2/commands/submit-and-wait", api_url);

    info!("Submitting TransferPreapproval creation command");
    let response = client
        .post(&url)
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let response_text = response.text().await?;

    if !status.is_success() {
        error!(status = %status, response = %response_text, "Failed to create TransferPreapproval");
        return Err(anyhow::anyhow!("Failed to create TransferPreapproval: {}", response_text));
    }

    // Parse response to get update ID
    let response_json: serde_json::Value = serde_json::from_str(&response_text)?;
    let update_id = response_json.get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    info!(update_id = %update_id, "TransferPreapproval created successfully");

    // Fetch the created TransferPreapproval contract ID
    info!("Fetching created TransferPreapproval contract");

    // Get the update details to find the created contract
    let update_payload = json!({
        "actAs": [&party_app],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_app: {}
                    },
                    "verbose": true
                },
                "transactionShape": "TRANSACTION_SHAPE_ACS_DELTA"
            }
        }
    });

    let update_response = client
        .post(&format!("{}v2/updates/update-by-id", api_url))
        .bearer_auth(&jwt)
        .json(&update_payload)
        .send()
        .await?;

    let update_text = update_response.text().await?;
    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract TransferPreapproval CID from the created events
    let mut preapproval_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains("TransferPreapproval") {
                        preapproval_cid = created.pointer("/contractId").and_then(|v| v.as_str());
                        break;
                    }
                }
            }
        }
    }

    if let Some(cid) = preapproval_cid {
        info!(
            cid = %cid,
            to = %party_app,
            provider = %party_app,
            expires = %expires_at.format("%Y-%m-%dT%H:%M:%SZ"),
            update_id = %update_id,
            "TransferPreapproval created successfully"
        );

        // Output the contract ID and .env instructions
        // Note: We keep these println statements as they output essential configuration data
        println!("\n✅ TransferPreapproval created successfully!");
        println!("\n📋 TransferPreapproval Contract ID:");
        println!("   {}", cid);
        println!("\nAdd to .env:");
        println!("   APP_TRANSFER_PREAPPROVAL_CID={}", cid);
    } else {
        warn!(
            update_id = %update_id,
            "Could not find TransferPreapproval contract in update response"
        );
        error!(
            update_id = %update_id,
            "TransferPreapproval creation appears successful but could not extract CID. Check the update details with: cargo run -- update {}",
            update_id
        );
    }

    Ok(())
}

/// Execute the CLI commands
pub async fn execute_cli(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Subscriptions => handle_subscriptions(),
        Commands::Users { subcommand } => handle_users(subcommand),
        Commands::User { query } => handle_user_search(&query),
        Commands::Pay {
            subscription,
            user,
            dry_run,
        } => handle_payment(&subscription, &user, dry_run).await,
        Commands::Start {
            once,
            interval,
            dry_run,
        } => handle_start(once, interval, dry_run).await,
        Commands::Update { update_id } => handle_get_update(&update_id).await,
        Commands::Setup {
            expires_in_min,
            subscriber_party,
        } => handle_setup_preapproval(expires_in_min, &subscriber_party).await,
    }
}
