mod context;
mod list;
mod payment;
mod request;
mod service;
mod url;

use anyhow::Result;
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "advanced-payment-cli")]
#[command(about = "CLI for AdvancedPayment contracts on Canton Network", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List amulets and advanced payment contracts
    List {
        /// Filter by party: "user" or "provider" (default: both)
        #[arg(short, long)]
        party: Option<String>,
    },
    /// AdvancedPaymentRequest commands
    #[command(subcommand)]
    Request(RequestCommands),
    /// AdvancedPayment commands
    #[command(subcommand)]
    Payment(PaymentCommands),
    /// AppService management commands
    #[command(subcommand)]
    Service(ServiceCommands),
}

#[derive(Subcommand)]
enum ServiceCommands {
    /// Create an AppServiceRequest (app action)
    Create,
    /// List pending AppServiceRequest contracts
    ListRequests,
    /// Accept an AppServiceRequest (provider action)
    Accept {
        /// Contract ID of the AppServiceRequest
        #[arg(short, long)]
        request_cid: String,
    },
    /// Reject an AppServiceRequest (provider action)
    Reject {
        /// Contract ID of the AppServiceRequest
        #[arg(short, long)]
        request_cid: String,
    },
    /// List active AppService contracts
    List,
    /// Terminate an AppService (provider action)
    Terminate {
        /// Contract ID of the AppService
        #[arg(short, long)]
        service_cid: String,
    },
}

#[derive(Subcommand)]
enum RequestCommands {
    /// Create a new AdvancedPaymentRequest via AppService (app action)
    Create {
        /// Contract ID of the AppService to use
        #[arg(short, long)]
        service_cid: String,
        /// Amount to lock (in CC)
        #[arg(short, long)]
        amount: String,
        /// Minimum amount to keep locked (in CC)
        #[arg(short, long)]
        minimum: String,
        /// Expiry time (ISO 8601 format, e.g., "2024-12-31T23:59:59Z"). Default: 1 day from now
        #[arg(short, long)]
        expires: Option<String>,
    },
    /// Accept an AdvancedPaymentRequest (owner action)
    Accept {
        /// Contract ID of the AdvancedPaymentRequest
        #[arg(short, long)]
        request_cid: String,
        /// Comma-separated list of Amulet contract IDs to use as funds
        #[arg(short, long, value_delimiter = ',')]
        amulet_cids: Vec<String>,
    },
    /// Decline an AdvancedPaymentRequest (owner action)
    Decline {
        /// Contract ID of the AdvancedPaymentRequest
        #[arg(short, long)]
        request_cid: String,
    },
    /// Cancel an AdvancedPaymentRequest (provider action)
    Cancel {
        /// Contract ID of the AdvancedPaymentRequest
        #[arg(short, long)]
        request_cid: String,
    },
}

#[derive(Subcommand)]
enum PaymentCommands {
    /// Withdraw amount from AdvancedPayment (provider action)
    Withdraw {
        /// Contract ID of the AdvancedPayment
        #[arg(short, long)]
        payment_cid: String,
        /// Amount to withdraw (in CC)
        #[arg(short, long)]
        amount: String,
    },
    /// Unlock partial amount from AdvancedPayment (owner action)
    Unlock {
        /// Contract ID of the AdvancedPayment
        #[arg(short, long)]
        payment_cid: String,
        /// Amount to unlock (in CC)
        #[arg(short, long)]
        amount: String,
    },
    /// Cancel AdvancedPayment and return funds to owner (provider action)
    Cancel {
        /// Contract ID of the AdvancedPayment
        #[arg(short, long)]
        payment_cid: String,
    },
    /// Expire AdvancedPayment after lock expiry (owner action)
    Expire {
        /// Contract ID of the AdvancedPayment
        #[arg(short, long)]
        payment_cid: String,
    },
    /// Top up AdvancedPayment with additional funds (owner action)
    Topup {
        /// Contract ID of the AdvancedPayment
        #[arg(short, long)]
        payment_cid: String,
        /// Amount to add (in CC)
        #[arg(short, long)]
        amount: String,
        /// New expiry time (must be after existing expiry)
        #[arg(short, long)]
        new_expires: String,
        /// Comma-separated list of Amulet contract IDs to use as funds
        #[arg(short = 'c', long, value_delimiter = ',')]
        amulet_cids: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::List { party } => list::handle_list(party).await?,
        Commands::Request(cmd) => match cmd {
            RequestCommands::Create {
                service_cid,
                amount,
                minimum,
                expires,
            } => {
                let expires = expires.unwrap_or_else(|| {
                    let one_day_from_now = Utc::now() + Duration::days(1);
                    one_day_from_now.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                });
                request::handle_create_request(service_cid, amount, minimum, expires).await?
            }
            RequestCommands::Accept {
                request_cid,
                amulet_cids,
            } => request::handle_accept_request(request_cid, amulet_cids).await?,
            RequestCommands::Decline { request_cid } => {
                request::handle_decline_request(request_cid).await?
            }
            RequestCommands::Cancel { request_cid } => {
                request::handle_cancel_request(request_cid).await?
            }
        },
        Commands::Payment(cmd) => match cmd {
            PaymentCommands::Withdraw {
                payment_cid,
                amount,
            } => payment::handle_withdraw(payment_cid, amount).await?,
            PaymentCommands::Unlock {
                payment_cid,
                amount,
            } => payment::handle_unlock(payment_cid, amount).await?,
            PaymentCommands::Cancel { payment_cid } => payment::handle_cancel(payment_cid).await?,
            PaymentCommands::Expire { payment_cid } => payment::handle_expire(payment_cid).await?,
            PaymentCommands::Topup {
                payment_cid,
                amount,
                new_expires,
                amulet_cids,
            } => payment::handle_topup(payment_cid, amount, new_expires, amulet_cids).await?,
        },
        Commands::Service(cmd) => match cmd {
            ServiceCommands::Create => service::handle_create_service_request().await?,
            ServiceCommands::ListRequests => service::handle_list_service_requests().await?,
            ServiceCommands::Accept { request_cid } => {
                service::handle_accept_service_request(request_cid).await?
            }
            ServiceCommands::Reject { request_cid } => {
                service::handle_reject_service_request(request_cid).await?
            }
            ServiceCommands::List => service::handle_list_services().await?,
            ServiceCommands::Terminate { service_cid } => {
                service::handle_terminate_service(service_cid).await?
            }
        },
    }

    Ok(())
}
