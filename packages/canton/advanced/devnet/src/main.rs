mod context;
mod interactive;
mod list;
mod payment;
mod request;
mod service;
mod signing;

use anyhow::Result;
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "advanced-payment-devnet-cli")]
#[command(about = "CLI for AdvancedPayment contracts on Canton Network (Devnet)", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List amulets and advanced payment contracts
    List {
        /// Filter by party: "user", "app", or "provider" (default: all except user)
        #[arg(short, long)]
        party: Option<String>,
        /// User party ID to show user assets (optional)
        #[arg(long)]
        user_party: Option<String>,
    },
    /// AppService management commands
    #[command(subcommand)]
    Service(ServiceCommands),
    /// AdvancedPaymentRequest commands
    #[command(subcommand)]
    Request(RequestCommands),
    /// AdvancedPayment commands
    #[command(subcommand)]
    Payment(PaymentCommands),
}

#[derive(Subcommand)]
enum ServiceCommands {
    /// Create a new AppServiceRequest (app requests service from provider)
    Create {
        /// Description of service relationship
        #[arg(short = 'd', long)]
        service_description: Option<String>,
    },
    /// List pending AppServiceRequest contracts
    ListRequests,
    /// Accept an AppServiceRequest (provider action) - creates AppService
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
        /// Reason for rejection
        #[arg(long)]
        reason: Option<String>,
    },
    /// Cancel an AppServiceRequest (app action)
    Cancel {
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
        /// Description of payment purpose
        #[arg(short = 'd', long)]
        description: Option<String>,
        /// External reference (invoice, order ID)
        #[arg(long)]
        reference: Option<String>,
        /// Owner party ID (user who will fund the payment)
        #[arg(short, long)]
        user: String,
    },
    /// Accept an AdvancedPaymentRequest (owner action)
    Accept {
        /// Contract ID of the AdvancedPaymentRequest
        #[arg(short, long)]
        request_cid: String,
        /// Comma-separated list of Amulet contract IDs to use as funds (required)
        #[arg(short, long, value_delimiter = ',', required = true)]
        amulet_cids: Vec<String>,
        /// Party ID of the user
        #[arg(long)]
        party_id: String,
        /// Base58-encoded private key
        #[arg(long)]
        private_key: String,
    },
    /// Reject an AdvancedPaymentRequest (owner action)
    Reject {
        /// Contract ID of the AdvancedPaymentRequest
        #[arg(short, long)]
        request_cid: String,
        /// Reason for rejection
        #[arg(long)]
        reason: Option<String>,
        /// Party ID of the user
        #[arg(long)]
        party_id: String,
        /// Base58-encoded private key
        #[arg(long)]
        private_key: String,
    },
    /// Cancel an AdvancedPaymentRequest (app action)
    Cancel {
        /// Contract ID of the AdvancedPaymentRequest
        #[arg(short, long)]
        request_cid: String,
    },
}

#[derive(Subcommand)]
enum PaymentCommands {
    /// Withdraw amount from AdvancedPayment (app action)
    Withdraw {
        /// Contract ID of the AdvancedPayment
        #[arg(short, long)]
        payment_cid: String,
        /// Amount to withdraw (in CC)
        #[arg(short, long)]
        amount: String,
        /// Reason for withdrawal (service description)
        #[arg(long)]
        reason: Option<String>,
    },
    /// Unlock partial amount from AdvancedPayment (owner action)
    Unlock {
        /// Contract ID of the AdvancedPayment
        #[arg(short, long)]
        payment_cid: String,
        /// Amount to unlock (in CC)
        #[arg(short, long)]
        amount: String,
        /// Party ID of the user
        #[arg(long)]
        party_id: String,
        /// Base58-encoded private key
        #[arg(long)]
        private_key: String,
    },
    /// Cancel AdvancedPayment and return funds to owner (app action)
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
        /// Party ID of the user
        #[arg(long)]
        party_id: String,
        /// Base58-encoded private key
        #[arg(long)]
        private_key: String,
    },
    /// Top up AdvancedPayment with additional funds (owner action)
    Topup {
        /// Contract ID of the AdvancedPayment
        #[arg(short, long)]
        payment_cid: String,
        /// Amount to add (in CC)
        #[arg(short, long)]
        amount: String,
        /// New expiry time (must be after existing expiry). Default: 1 day from now
        #[arg(short, long)]
        new_expires: Option<String>,
        /// Comma-separated list of Amulet contract IDs to use as funds
        #[arg(short = 'c', long, value_delimiter = ',')]
        amulet_cids: Vec<String>,
        /// Party ID of the user
        #[arg(long)]
        party_id: String,
        /// Base58-encoded private key
        #[arg(long)]
        private_key: String,
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
        Commands::List { party, user_party } => list::handle_list(party, user_party).await?,
        Commands::Service(cmd) => match cmd {
            ServiceCommands::Create { service_description } => {
                service::handle_create_service_request(service_description).await?
            }
            ServiceCommands::ListRequests => service::handle_list_service_requests().await?,
            ServiceCommands::Accept { request_cid } => {
                service::handle_accept_service_request(request_cid).await?
            }
            ServiceCommands::Reject { request_cid, reason } => {
                service::handle_reject_service_request(request_cid, reason).await?
            }
            ServiceCommands::Cancel { request_cid } => {
                service::handle_cancel_service_request(request_cid).await?
            }
            ServiceCommands::List => service::handle_list_services().await?,
            ServiceCommands::Terminate { service_cid } => {
                service::handle_terminate_service(service_cid).await?
            }
        },
        Commands::Request(cmd) => match cmd {
            RequestCommands::Create {
                service_cid,
                amount,
                minimum,
                expires,
                description,
                reference,
                user,
            } => {
                let expires = expires.unwrap_or_else(|| {
                    let one_day_from_now = Utc::now() + Duration::days(1);
                    one_day_from_now.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                });
                request::handle_create_request(service_cid, amount, minimum, expires, description, reference, user).await?
            }
            RequestCommands::Accept {
                request_cid,
                amulet_cids,
                party_id,
                private_key,
            } => request::handle_accept_request(request_cid, amulet_cids, party_id, private_key).await?,
            RequestCommands::Reject { request_cid, reason, party_id, private_key } => {
                request::handle_reject_request(request_cid, reason, party_id, private_key).await?
            }
            RequestCommands::Cancel { request_cid } => {
                request::handle_cancel_request(request_cid).await?
            }
        },
        Commands::Payment(cmd) => match cmd {
            PaymentCommands::Withdraw {
                payment_cid,
                amount,
                reason,
            } => payment::handle_withdraw(payment_cid, amount, reason).await?,
            PaymentCommands::Unlock {
                payment_cid,
                amount,
                party_id,
                private_key,
            } => payment::handle_unlock(payment_cid, amount, party_id, private_key).await?,
            PaymentCommands::Cancel { payment_cid } => payment::handle_cancel(payment_cid).await?,
            PaymentCommands::Expire { payment_cid, party_id, private_key } => {
                payment::handle_expire(payment_cid, party_id, private_key).await?
            }
            PaymentCommands::Topup {
                payment_cid,
                amount,
                new_expires,
                amulet_cids,
                party_id,
                private_key,
            } => {
                let new_expires = new_expires.unwrap_or_else(|| {
                    let one_day_from_now = Utc::now() + Duration::days(1);
                    one_day_from_now.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                });
                payment::handle_topup(payment_cid, amount, new_expires, amulet_cids, party_id, private_key).await?
            }
        },
    }

    Ok(())
}
