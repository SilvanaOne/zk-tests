mod auth;
mod client;
mod config;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, Subcommand};
use colored::Colorize;

use auth::generate_jwt_token;
use client::LedgerClient;
use client::proto::com::daml::ledger::api::v2::value;
use config::Config;

#[derive(Parser)]
#[command(name = "scan")]
#[command(about = "Canton Ledger Scanner - Query transactions and balances", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(long, env = "LEDGER_HOST", default_value = "localhost")]
    host: Option<String>,
    
    #[arg(long, env = "LEDGER_PORT")]
    port: Option<u16>,
    
    #[arg(long, env = "PARTY_ID")]
    party: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show wallet balance
    Balance,

    /// Show all transactions
    Transactions {
        #[arg(long, default_value = "100")]
        limit: usize,
    },

    /// Show transactions from the last hour
    Hour,

    /// Show active contracts
    Contracts,

    /// Show configuration
    Config,

    /// Show ledger version and status
    Status,

    /// Show all information
    All,

    /// Generate JWT token for authentication
    Jwt {
        /// User to generate token for (defaults to JWT_USER from env)
        #[arg(long)]
        user: Option<String>,
    },

    /// Scan all localnet participants for balances
    ScanAll,

    /// Debug: Show all contract templates and sample data
    Debug {
        /// Port to connect to (defaults to 2901)
        #[arg(long, default_value = "2901")]
        port: u16,
    },
}


#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    // Load config from environment
    let mut config = Config::from_env()?;
    
    // Override with CLI arguments
    if let Some(host) = cli.host {
        config.ledger_host = host;
    }
    if let Some(port) = cli.port {
        config.ledger_port = port;
    }
    if let Some(party) = cli.party {
        config.party_id = party;
    }
    
    match cli.command {
        Commands::Balance => show_balance(&config).await?,
        Commands::Transactions { limit } => show_transactions(&config, limit).await?,
        Commands::Hour => show_hour_transactions(&config).await?,
        Commands::Contracts => show_contracts(&config).await?,
        Commands::Config => show_config(&config),
        Commands::Status => show_status(&config).await?,
        Commands::All => show_all(&config).await?,
        Commands::Jwt { user } => generate_and_show_jwt(&config, user)?,
        Commands::ScanAll => scan_all_participants().await?,
        Commands::Debug { port } => debug_contracts(&config, port).await?,
    }
    
    Ok(())
}

async fn show_balance(config: &Config) -> Result<()> {
    println!("{}", "📊 Wallet Balances for All Participants".bold().blue());
    println!("{}", "═".repeat(70).blue());

    // Define all known participants
    let participants = vec![
        ("app-user", 2901, 2903, "http://wallet.localhost:2000"),
        ("app-provider", 3901, 3903, "http://wallet.localhost:3000"),
    ];

    for (name, ledger_port, validator_port, web_url) in participants {
        println!("\n{}", format!("▶ {} (Port {})", name, ledger_port).bright_cyan().bold());
        println!("  Web UI: {}", web_url.bright_black());

        // Create config for this participant
        let mut participant_config = config.clone();
        participant_config.ledger_port = ledger_port;
        participant_config.validator_port = validator_port;

        // Try to get users and their parties first
        match LedgerClient::new(participant_config.clone()).await {
            Ok(client) => {
                // Get users for this participant
                match client.get_users().await {
                    Ok(users) => {
                        let mut found_any_balance = false;

                        for user in users {
                            if let Some(party_id) = user.get("primary_party").and_then(|p| p.as_str()) {
                                let user_id = user.get("id").and_then(|id| id.as_str()).unwrap_or("unknown");

                                // Skip admin users
                                if user_id == "participant_admin" {
                                    continue;
                                }

                                println!("\n  User: {} ", user_id.green());
                                println!("  Party: {}", &party_id[..50.min(party_id.len())].bright_black());

                                // Update config with this party ID and use the correct user
                                participant_config.party_id = party_id.to_string();
                                // Use the actual user ID (app-user or app-provider) for JWT generation
                                participant_config.jwt_user = user_id.to_string();

                                // Create a new client with the updated config for balance requests
                                let balance_client = LedgerClient::new(participant_config.clone()).await?;

                                // Try to get balance from validator API
                                match balance_client.get_balance().await {
                                    Ok(balance) if !balance.is_null() && balance.get("error").is_none() => {
                                        if let Some(round) = balance.get("round") {
                                            println!("    Round: {}", round.to_string().green());
                                            found_any_balance = true;
                                        }
                                        if let Some(unlocked) = balance.get("effective_unlocked_qty") {
                                            println!("    Unlocked AMT: {}", unlocked.to_string().yellow().bold());
                                            found_any_balance = true;
                                        }
                                        if let Some(locked) = balance.get("effective_locked_qty") {
                                            println!("    Locked AMT: {}", locked.to_string().yellow());
                                            found_any_balance = true;
                                        }
                                        if let Some(fees) = balance.get("total_holding_fees") {
                                            println!("    Holding Fees: {}", fees.to_string().red());
                                        }
                                    }
                                    _ => {
                                        // Try to find balance in contracts for this party
                                        let client_for_party = LedgerClient::new(participant_config.clone()).await?;
                                        match client_for_party.get_active_contracts().await {
                                            Ok(contracts) => {
                                                let mut balance_found = false;

                                                for c in &contracts {
                                                    if let Some(event) = &c.created_event {
                                                        let template_name = event.template_id.as_ref()
                                                            .map(|t| format!("{}:{}", t.module_name, t.entity_name))
                                                            .unwrap_or_default();

                                                        if template_name.to_lowercase().contains("wallet") ||
                                                           template_name.to_lowercase().contains("balance") ||
                                                           template_name.to_lowercase().contains("account") ||
                                                           template_name.to_lowercase().contains("amulet") {

                                                            if !balance_found {
                                                                println!("    Balance from contracts:");
                                                                balance_found = true;
                                                                found_any_balance = true;
                                                            }

                                                            if let Some(record) = &event.create_arguments {
                                                                for field in &record.fields {
                                                                    let label = &field.label;
                                                                    if label.to_lowercase().contains("balance") ||
                                                                       label.to_lowercase().contains("amount") ||
                                                                       label.to_lowercase().contains("qty") {
                                                                        if let Some(value) = &field.value {
                                                                            use client::proto::com::daml::ledger::api::v2::value;
                                                                            let value_str = match &value.sum {
                                                                                Some(value::Sum::Numeric(n)) => n.clone(),
                                                                                Some(value::Sum::Int64(n)) => n.to_string(),
                                                                                _ => continue,
                                                                            };
                                                                            println!("      {}: {}", label, value_str.yellow());
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                if !balance_found {
                                                    println!("    No balance data available");
                                                }
                                            }
                                            Err(_) => {
                                                println!("    Unable to fetch contracts");
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if !found_any_balance {
                            println!("\n  No balance data found for any user");
                        }
                    }
                    Err(e) => {
                        println!("  Error fetching users: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("  Failed to connect: {}", e);
            }
        }
    }

    println!("\n{}", "═".repeat(70).blue());

    Ok(())
}

async fn show_transactions(config: &Config, limit: usize) -> Result<()> {
    println!("{}", "📜 Recent Transactions".bold().blue());
    println!("{}", "═".repeat(70).blue());
    
    let client = LedgerClient::new(config.clone()).await?;
    let transactions = client.get_transactions(0, None).await?;
    
    println!("\n▶ Recent Transactions (via gRPC UpdateService)");
    
    for tx in transactions.iter().take(limit) {
        println!("{}", "─".repeat(40).bright_black());
        let time_str = tx.effective_at.as_ref()
            .and_then(|t| DateTime::<Utc>::from_timestamp(t.seconds, t.nanos as u32))
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
            .unwrap_or_else(|| "N/A".to_string());
        println!("Time: {}", time_str.green());
        println!("Offset: {}", tx.offset.to_string().yellow());
        
        let command = if tx.command_id.is_empty() {
            "N/A".to_string()
        } else {
            let parts: Vec<&str> = tx.command_id.split('_').collect();
            let cmd = parts.first().unwrap_or(&tx.command_id.as_str()).to_string();
            let parts: Vec<&str> = cmd.split('.').collect();
            parts.last().unwrap_or(&cmd.as_str()).to_string()
        };
        println!("Command: {}", command.cyan());
        
        println!("Events: {} event(s)", tx.events.len());
        
        // Show event details
        for event in &tx.events {
            use client::proto::com::daml::ledger::api::v2::event::Event;
            match &event.event {
                Some(Event::Created(c)) => {
                    let template = format!("{}:{}",
                        c.template_id.as_ref().map(|t| t.module_name.as_str()).unwrap_or("?"),
                        c.template_id.as_ref().map(|t| t.entity_name.as_str()).unwrap_or("?"));
                    
                    // Extract some key fields from the record if available
                    let mut details = Vec::new();
                    if let Some(record) = &c.create_arguments {
                        for field in &record.fields {
                            let label = &field.label;
                                    if label == "endUserName" || label == "user" || label == "validator" {
                                        if let Some(value) = &field.value {
                                            if let Some(value::Sum::Text(text)) = &value.sum {
                                                let truncated = if text.len() > 20 { 
                                                    format!("{}...", &text[..20])
                                                } else { 
                                                    text.clone() 
                                                };
                                                details.push(format!("{}={}", label, truncated));
                                            } else if let Some(value::Sum::Party(party)) = &value.sum {
                                                let truncated = if party.len() > 20 { 
                                                    format!("{}...", &party[..20])
                                                } else { 
                                                    party.clone() 
                                                };
                                                details.push(format!("{}={}", label, truncated));
                                            }
                                        }
                                    }
                            }
                    }
                    
                    let detail_str = if details.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", details.join(", "))
                    };
                    
                    println!("  {} CREATE: {}{}", "✓".green(), template, detail_str);
                },
                Some(Event::Archived(a)) => {
                    let template = format!("{}:{}",
                        a.template_id.as_ref().map(|t| t.module_name.as_str()).unwrap_or("?"),
                        a.template_id.as_ref().map(|t| t.entity_name.as_str()).unwrap_or("?"));
                    println!("  {} ARCHIVE: {}", "✗".red(), template);
                },
                _ => {
                    println!("  {} Unknown event", "?".yellow());
                }
            }
        }
    }
    
    if transactions.is_empty() {
        println!("No transactions found");
    }
    
    Ok(())
}

async fn show_hour_transactions(config: &Config) -> Result<()> {
    println!("{}", "⏰ Transactions from Last Hour".bold().blue());
    println!("{}", "═".repeat(50).blue());
    
    let client = LedgerClient::new(config.clone()).await?;
    let transactions = client.get_transactions(0, None).await?;
    
    let one_hour_ago = Utc::now() - Duration::hours(1);
    
    let recent_txs: Vec<_> = transactions
        .into_iter()
        .filter(|tx| {
            if let Some(ts) = &tx.effective_at {
                if let Some(time) = DateTime::<Utc>::from_timestamp(ts.seconds, ts.nanos as u32) {
                    return time > one_hour_ago;
                }
            }
            false
        })
        .collect();
    
    println!("Found {} transactions in the last hour\n", recent_txs.len());
    
    for tx in recent_txs.iter().take(20) {
        println!("{}", "─".repeat(40).bright_black());
        let time_str = tx.effective_at.as_ref()
            .and_then(|t| DateTime::<Utc>::from_timestamp(t.seconds, t.nanos as u32))
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
            .unwrap_or_else(|| "N/A".to_string());
        println!("Time: {}", time_str.green());
        println!("Offset: {}", tx.offset.to_string().yellow());
        
        let command = if tx.command_id.is_empty() {
            "N/A".to_string()
        } else {
            tx.command_id.clone()
        };
        println!("Command: {}", command.cyan());
        
        println!("Events: {} event(s)", tx.events.len());
        
        // Show event details
        for event in &tx.events {
            use client::proto::com::daml::ledger::api::v2::event::Event;
            match &event.event {
                Some(Event::Created(c)) => {
                    let template = format!("{}:{}",
                        c.template_id.as_ref().map(|t| t.module_name.as_str()).unwrap_or("?"),
                        c.template_id.as_ref().map(|t| t.entity_name.as_str()).unwrap_or("?"));
                    
                    // Extract some key fields from the record if available
                    let mut details = Vec::new();
                    if let Some(record) = &c.create_arguments {
                        for field in &record.fields {
                            let label = &field.label;
                                    if label == "endUserName" || label == "user" || label == "validator" {
                                        if let Some(value) = &field.value {
                                            if let Some(value::Sum::Text(text)) = &value.sum {
                                                let truncated = if text.len() > 20 { 
                                                    format!("{}...", &text[..20])
                                                } else { 
                                                    text.clone() 
                                                };
                                                details.push(format!("{}={}", label, truncated));
                                            } else if let Some(value::Sum::Party(party)) = &value.sum {
                                                let truncated = if party.len() > 20 { 
                                                    format!("{}...", &party[..20])
                                                } else { 
                                                    party.clone() 
                                                };
                                                details.push(format!("{}={}", label, truncated));
                                            }
                                        }
                                    }
                            }
                    }
                    
                    let detail_str = if details.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", details.join(", "))
                    };
                    
                    println!("  {} CREATE: {}{}", "✓".green(), template, detail_str);
                },
                Some(Event::Archived(a)) => {
                    let template = format!("{}:{}",
                        a.template_id.as_ref().map(|t| t.module_name.as_str()).unwrap_or("?"),
                        a.template_id.as_ref().map(|t| t.entity_name.as_str()).unwrap_or("?"));
                    println!("  {} ARCHIVE: {}", "✗".red(), template);
                },
                _ => {
                    println!("  {} Unknown event", "?".yellow());
                }
            }
        }
    }
    
    Ok(())
}

async fn show_contracts(config: &Config) -> Result<()> {
    println!("{}", "📄 Active Contracts".bold().blue());
    println!("{}", "═".repeat(50).blue());
    
    let client = LedgerClient::new(config.clone()).await?;
    let contracts = client.get_active_contracts().await?;
    
    if contracts.is_empty() {
        println!("No active contracts found");
    } else {
        println!("\nFound {} active contracts\n", contracts.len());
        
        for (i, c) in contracts.iter().take(20).enumerate() {
            if let Some(event) = &c.created_event {
                println!("{}", "─".repeat(40).bright_black());
                println!("Contract #{}", i + 1);
                
                let template = format!("{}:{}",
                    event.template_id.as_ref().map(|t| t.module_name.as_str()).unwrap_or("?"),
                    event.template_id.as_ref().map(|t| t.entity_name.as_str()).unwrap_or("?"));
                println!("Template: {}", template.cyan());
                
                let cid = if event.contract_id.len() > 50 {
                    format!("{}...", &event.contract_id[..50])
                } else {
                    event.contract_id.clone()
                };
                println!("Contract ID: {}", cid.yellow());
                
                if let Some(created_at) = &event.created_at {
                    let time = DateTime::<Utc>::from_timestamp(created_at.seconds, created_at.nanos as u32)
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    println!("Created At: {}", time.green());
                }
                
                // Show some contract details if available
                if let Some(record) = &event.create_arguments {
                    let mut shown_fields = 0;
                    for field in &record.fields {
                        if shown_fields >= 3 { break; }  // Limit to 3 fields
                        let label = &field.label;
                            if let Some(value) = &field.value {
                                let value_str = match &value.sum {
                                    Some(value::Sum::Text(text)) => text.clone(),
                                    Some(value::Sum::Party(party)) => {
                                        if party.len() > 30 {
                                            format!("{}...", &party[..30])
                                        } else {
                                            party.clone()
                                        }
                                    },
                                    Some(value::Sum::Int64(n)) => n.to_string(),
                                    Some(value::Sum::Bool(b)) => b.to_string(),
                                    _ => "...".to_string(),
                                };
                                println!("  {}: {}", label, value_str);
                                shown_fields += 1;
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn show_config(config: &Config) {
    println!("{}", "⚙️  Configuration".bold().blue());
    println!("{}", "═".repeat(50).blue());
    
    println!("Ledger Host: {}", config.ledger_host.green());
    println!("Ledger Port: {}", config.ledger_port.to_string().green());
    println!("Validator Port: {}", config.validator_port.to_string().green());
    println!("JWT Audience: {}", config.jwt_audience.yellow());
    println!("JWT User: {}", config.jwt_user.yellow());
    println!("Party ID: {}", &config.party_id[..50.min(config.party_id.len())].cyan());
    println!("Use TLS: {}", config.use_tls.to_string().cyan());
}

async fn show_status(config: &Config) -> Result<()> {
    println!("{}", "🔍 Ledger Status".bold().blue());
    println!("{}", "═".repeat(50).blue());
    
    let client = LedgerClient::new(config.clone()).await?;
    
    match client.get_version().await {
        Ok(version) => println!("API Version: {}", version.green()),
        Err(e) => println!("API Version: {} {}", "Error:".red(), e),
    }
    
    match client.get_ledger_end().await {
        Ok(offset) => println!("Ledger Offset: {}", offset.yellow()),
        Err(e) => println!("Ledger Offset: {} {}", "Error:".red(), e),
    }
    
    Ok(())
}

async fn show_all(config: &Config) -> Result<()> {
    show_status(config).await?;
    println!();
    show_balance(config).await?;
    println!();
    show_transactions(config, 10).await?;
    println!();
    show_contracts(config).await?;

    Ok(())
}

fn generate_and_show_jwt(config: &Config, user: Option<String>) -> Result<()> {
    let jwt_user = user.unwrap_or_else(|| config.jwt_user.clone());

    println!("{}", "🔐 JWT Token Generator".bold().blue());
    println!("{}", "═".repeat(50).blue());

    // Generate token
    let token = generate_jwt_token(&config.jwt_secret, &config.jwt_audience, &jwt_user)?;

    println!("User: {}", jwt_user.green());
    println!("Audience: {}", config.jwt_audience.yellow());
    println!();
    println!("{}", "Token:".bold());
    println!("{}", token.cyan());
    println!();
    println!("{}", "Usage with grpcurl:".bold());
    println!("export TOKEN=\"{}\"", token);
    println!("grpcurl -H \"Authorization: Bearer $TOKEN\" -plaintext localhost:{} com.daml.ledger.api.v2.admin.UserManagementService/ListUsers",
            config.ledger_port);
    println!();
    println!("{}", "Or use directly:".bold());
    println!("grpcurl -H \"Authorization: Bearer {}\" -plaintext localhost:{} com.daml.ledger.api.v2.admin.UserManagementService/ListUsers",
            token, config.ledger_port);

    Ok(())
}

async fn debug_contracts(config: &Config, port: u16) -> Result<()> {
    println!("{}", "🔍 Debug: Contract Analysis".bold().blue());
    println!("{}", "═".repeat(70).blue());

    let mut debug_config = config.clone();
    debug_config.ledger_port = port;

    let client = LedgerClient::new(debug_config.clone()).await?;

    // Get all users first
    println!("\n{}", "Users on this participant:".yellow());
    match client.get_users().await {
        Ok(users) => {
            for user in &users {
                let user_id = user.get("id").and_then(|id| id.as_str()).unwrap_or("unknown");
                let party_id = user.get("primary_party").and_then(|p| p.as_str()).unwrap_or("");

                if !party_id.is_empty() {
                    println!("  User: {}, Party: {}", user_id.green(), &party_id[..50.min(party_id.len())].bright_black());

                    // Get contracts for this party
                    debug_config.party_id = party_id.to_string();
                    let party_client = LedgerClient::new(debug_config.clone()).await?;

                    match party_client.get_active_contracts().await {
                        Ok(contracts) => {
                            if !contracts.is_empty() {
                                println!("\n    {} Active contracts for {}:", contracts.len(), user_id);

                                // Group contracts by template
                                let mut template_map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();

                                for c in &contracts {
                                    if let Some(event) = &c.created_event {
                                        let template_name = event.template_id.as_ref()
                                            .map(|t| format!("{}:{}", t.module_name, t.entity_name))
                                            .unwrap_or_else(|| "Unknown".to_string());

                                        template_map.entry(template_name).or_insert(Vec::new()).push(event.create_arguments.clone());
                                    }
                                }

                                // Show each template type and sample data
                                for (template, instances) in template_map.iter() {
                                    println!("\n      {} {} ({}x)", "▶".cyan(), template.bold(), instances.len());

                                    // Show first instance's fields
                                    if let Some(Some(record)) = instances.first() {
                                        let mut shown = 0;
                                        for field in &record.fields {
                                            if shown >= 10 { // Show only first 10 fields
                                                println!("        ... and {} more fields", record.fields.len() - 10);
                                                break;
                                            }

                                            let value_str = if let Some(value) = &field.value {
                                                use client::proto::com::daml::ledger::api::v2::value;
                                                match &value.sum {
                                                    Some(value::Sum::Text(text)) => {
                                                        if text.len() > 50 {
                                                            format!("\"{}...\"", &text[..50])
                                                        } else {
                                                            format!("\"{}\"", text)
                                                        }
                                                    },
                                                    Some(value::Sum::Party(party)) => {
                                                        if party.len() > 40 {
                                                            format!("Party({}...)", &party[..40])
                                                        } else {
                                                            format!("Party({})", party)
                                                        }
                                                    },
                                                    Some(value::Sum::Int64(n)) => n.to_string(),
                                                    Some(value::Sum::Numeric(n)) => format!("{} (numeric)", n),
                                                    Some(value::Sum::Bool(b)) => b.to_string(),
                                                    Some(value::Sum::ContractId(id)) => {
                                                        if id.len() > 20 {
                                                            format!("Contract({}...)", &id[..20])
                                                        } else {
                                                            format!("Contract({})", id)
                                                        }
                                                    },
                                                    Some(value::Sum::Timestamp(ts)) => format!("Timestamp({})", ts),
                                                    Some(value::Sum::Date(d)) => format!("Date({})", d),
                                                    Some(value::Sum::Record(_)) => "[Record]".to_string(),
                                                    Some(value::Sum::List(_)) => "[List]".to_string(),
                                                    Some(value::Sum::GenMap(_)) => "[Map]".to_string(),
                                                    Some(value::Sum::Optional(_)) => "[Optional]".to_string(),
                                                    _ => "[Complex]".to_string(),
                                                }
                                            } else {
                                                "[None]".to_string()
                                            };

                                            println!("        {}: {}", field.label.yellow(), value_str);
                                            shown += 1;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("    Error getting contracts: {}", e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("  Error: {}", e);
        }
    }

    println!("\n{}", "═".repeat(70).blue());
    Ok(())
}

async fn scan_all_participants() -> Result<()> {
    println!("{}", "🔍 Scanning All Localnet Participants".bold().blue());
    println!("{}", "═".repeat(70).blue());

    // Define participant configurations
    let participants = vec![
        ("app-user", 2901, 2903, "app_user_localnet-localparty-1::1220fa85169dbaad11c0c74ed5b25d6a0c8572d7752639f470cc687d63457832a5c5"),
        ("app-provider", 3901, 3903, "app_provider_localnet-localparty-1::122047631b9f7d279838384bfa3bfef3d1d8e35808707e1acc0f02355077aaab9eb7"),
    ];

    for (name, ledger_port, validator_port, party_id) in participants {
        println!("\n{}", format!("━━━ {} (Port {}) ━━━", name, ledger_port).bright_cyan().bold());

        // Create config for this participant
        let mut config = Config::default();
        config.ledger_port = ledger_port;
        config.validator_port = validator_port;
        config.party_id = party_id.to_string();
        config.jwt_user = "ledger-api-user".to_string();

        // Try to connect and get balance
        match LedgerClient::new(config.clone()).await {
            Ok(client) => {
                // Try validator API first
                match client.get_balance().await {
                    Ok(balance) if !balance.is_null() && !balance.get("error").is_some() => {
                        println!("  {} Balance from validator API:", "✓".green());
                        if let Some(round) = balance.get("round") {
                            println!("    Round: {}", round.to_string().green());
                        }
                        if let Some(unlocked) = balance.get("effective_unlocked_qty") {
                            println!("    Unlocked AMT: {}", unlocked.to_string().yellow());
                        }
                        if let Some(locked) = balance.get("effective_locked_qty") {
                            println!("    Locked AMT: {}", locked.to_string().yellow());
                        }
                    }
                    _ => {
                        // Fallback to searching contracts
                        println!("  Searching contracts for balance information...");

                        match client.get_active_contracts().await {
                            Ok(contracts) => {
                                let mut found_balance = false;
                                let mut balance_contracts = Vec::new();

                                for c in &contracts {
                                    if let Some(event) = &c.created_event {
                                        let template_name = event.template_id.as_ref()
                                            .map(|t| format!("{}:{}", t.module_name, t.entity_name))
                                            .unwrap_or_default();

                                        // Look for balance-related contracts
                                        if template_name.to_lowercase().contains("wallet") ||
                                           template_name.to_lowercase().contains("balance") ||
                                           template_name.to_lowercase().contains("account") ||
                                           template_name.to_lowercase().contains("amulet") ||
                                           template_name.to_lowercase().contains("featuredappright") {
                                            balance_contracts.push((template_name.clone(), event.create_arguments.clone()));
                                            found_balance = true;
                                        }
                                    }
                                }

                                if found_balance {
                                    println!("  {} Found {} balance-related contracts", "✓".green(), balance_contracts.len());

                                    // Show unique contract types and their counts
                                    let mut contract_counts = std::collections::HashMap::new();
                                    for (template, _) in &balance_contracts {
                                        *contract_counts.entry(template.clone()).or_insert(0) += 1;
                                    }

                                    for (template, count) in contract_counts {
                                        println!("    {} ({} contract{})",
                                            template.cyan(),
                                            count,
                                            if count > 1 { "s" } else { "" });
                                    }

                                    // Show sample balance data from first relevant contract
                                    for (template, args) in balance_contracts.iter().take(1) {
                                        if let Some(record) = args {
                                            println!("\n    Sample data from {}:", template.cyan());
                                            for field in &record.fields {
                                                let label = &field.label;
                                                if label.to_lowercase().contains("balance") ||
                                                   label.to_lowercase().contains("amount") ||
                                                   label.to_lowercase().contains("qty") ||
                                                   label.to_lowercase().contains("round") ||
                                                   label.to_lowercase().contains("amulet") ||
                                                   label == "owner" {
                                                    if let Some(value) = &field.value {
                                                        use client::proto::com::daml::ledger::api::v2::value;
                                                        let value_str = match &value.sum {
                                                            Some(value::Sum::Text(text)) => text.clone(),
                                                            Some(value::Sum::Party(party)) => party.clone(),
                                                            Some(value::Sum::Int64(n)) => n.to_string(),
                                                            Some(value::Sum::Numeric(n)) => n.clone(),
                                                            Some(value::Sum::Bool(b)) => b.to_string(),
                                                            _ => continue,
                                                        };
                                                        println!("      {}: {}", label, value_str.yellow());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    println!("  {} No balance contracts found", "✗".red());
                                }

                                println!("  Total contracts: {}", contracts.len());
                            }
                            Err(e) => println!("  {} Error getting contracts: {}", "✗".red(), e),
                        }
                    }
                }
            }
            Err(e) => println!("  {} Failed to connect: {}", "✗".red(), e),
        }
    }

    println!("\n{}", "═".repeat(70).blue());
    println!("Scan complete!");

    Ok(())
}
