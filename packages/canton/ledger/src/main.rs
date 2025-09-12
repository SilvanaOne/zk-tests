mod auth;
mod client;
mod config;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, Subcommand};
use colored::Colorize;

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
    }
    
    Ok(())
}

async fn show_balance(config: &Config) -> Result<()> {
    println!("{}", "📊 Wallet Balance".bold().blue());
    println!("{}", "═".repeat(50).blue());
    
    let client = LedgerClient::new(config.clone()).await?;
    let balance = client.get_balance().await?;
    
    if let Some(round) = balance.get("round") {
        println!("Round: {}", round.to_string().green());
    }
    if let Some(unlocked) = balance.get("effective_unlocked_qty") {
        println!("Unlocked AMT: {}", unlocked.to_string().yellow());
    }
    if let Some(locked) = balance.get("effective_locked_qty") {
        println!("Locked AMT: {}", locked.to_string().yellow());
    }
    if let Some(fees) = balance.get("total_holding_fees") {
        println!("Holding Fees: {}", fees.to_string().red());
    }
    
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
