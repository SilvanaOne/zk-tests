use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::timeout;
use tonic::transport::Channel;
use tonic::{metadata::MetadataValue, Request};

use crate::auth::generate_jwt_token;
use crate::config::Config;

// Proto imports will be generated in OUT_DIR
pub mod proto {
    // Include google types first
    pub mod google {
        pub mod rpc {
            include!(concat!(env!("OUT_DIR"), "/google.rpc.rs"));
        }
    }
    
    pub mod com {
        pub mod daml {
            pub mod ledger {
                pub mod api {
                    pub mod v2 {
                        include!(concat!(env!("OUT_DIR"), "/com.daml.ledger.api.v2.rs"));
                        
                        pub mod admin {
                            include!(concat!(env!("OUT_DIR"), "/com.daml.ledger.api.v2.admin.rs"));
                        }
                    }
                }
            }
        }
    }
}

use proto::com::daml::ledger::api::v2::*;

pub struct LedgerClient {
    channel: Channel,
    token: String,
    config: Config,
}

impl LedgerClient {
    pub async fn new(config: Config) -> Result<Self> {
        let token = generate_jwt_token(&config.jwt_secret, &config.jwt_audience, &config.jwt_user)?;
        
        let channel = if config.use_tls {
            timeout(Duration::from_secs(10), 
                Channel::from_shared(format!("https://{}", config.ledger_endpoint()))?
                    .connect_timeout(Duration::from_secs(5))
                    .timeout(Duration::from_secs(30))
                    .connect()
            ).await??
        } else {
            timeout(Duration::from_secs(10),
                Channel::from_shared(format!("http://{}", config.ledger_endpoint()))?
                    .connect_timeout(Duration::from_secs(5))
                    .timeout(Duration::from_secs(30))
                    .connect()
            ).await??
        };

        Ok(Self {
            channel,
            token,
            config,
        })
    }

    fn add_auth_header<T>(&self, mut request: Request<T>) -> Request<T> {
        let token = format!("Bearer {}", self.token);
        request.metadata_mut().insert(
            "authorization",
            MetadataValue::from_str(&token).unwrap(),
        );
        request
    }

    pub async fn get_version(&self) -> Result<String> {
        let mut client = version_service_client::VersionServiceClient::new(self.channel.clone());
        let request = self.add_auth_header(Request::new(GetLedgerApiVersionRequest {}));
        let response = client.get_ledger_api_version(request).await?;
        Ok(response.into_inner().version)
    }

    pub async fn get_ledger_end(&self) -> Result<String> {
        let mut client = state_service_client::StateServiceClient::new(self.channel.clone());
        let request = self.add_auth_header(Request::new(GetLedgerEndRequest {}));
        let response = client.get_ledger_end(request).await?;
        Ok(response.into_inner().offset.to_string())
    }

    pub async fn get_transactions(
        &self,
        begin_offset: i64,
        end_offset: Option<i64>,
    ) -> Result<Vec<Transaction>> {
        let mut client = update_service_client::UpdateServiceClient::new(self.channel.clone());
        
        // Get ledger end if no end offset specified
        let actual_end_offset = match end_offset {
            Some(offset) => Some(offset),
            None => {
                let ledger_end = self.get_ledger_end().await?;
                Some(ledger_end.parse::<i64>().unwrap_or(0))
            }
        };
        
        let update_format = Some(UpdateFormat {
            include_transactions: Some(TransactionFormat {
                event_format: Some(EventFormat {
                    filters_by_party: std::collections::HashMap::from([(
                        self.config.party_id.clone(),
                        Filters::default(),
                    )]),
                    filters_for_any_party: None,
                    verbose: true,
                }),
                transaction_shape: TransactionShape::AcsDelta as i32,
            }),
            include_reassignments: None,
            include_topology_events: None,
        });

        let request = GetUpdatesRequest {
            begin_exclusive: begin_offset,
            end_inclusive: actual_end_offset,
            filter: None,
            verbose: false,
            update_format,
        };

        let request = self.add_auth_header(Request::new(request));
        let mut stream = client.get_updates(request).await?.into_inner();
        
        let mut transactions = Vec::new();
        while let Some(response) = stream.message().await? {
            if let Some(update) = response.update {
                if let get_updates_response::Update::Transaction(tx) = update {
                    transactions.push(tx);
                }
            }
        }
        
        Ok(transactions)
    }

    pub async fn get_active_contracts(&self) -> Result<Vec<ActiveContract>> {
        let mut client = state_service_client::StateServiceClient::new(self.channel.clone());
        
        let event_format = EventFormat {
            filters_by_party: std::collections::HashMap::from([(
                self.config.party_id.clone(),
                Filters::default(),
            )]),
            filters_for_any_party: None,
            verbose: true,
        };

        let request = GetActiveContractsRequest {
            filter: None,
            verbose: false,
            active_at_offset: 0,
            event_format: Some(event_format),
        };

        let request = self.add_auth_header(Request::new(request));
        let mut stream = client.get_active_contracts(request).await?.into_inner();
        
        let mut contracts = Vec::new();
        let stream_timeout = Duration::from_secs(30); // 30 second timeout for the entire stream
        let message_timeout = Duration::from_secs(5); // 5 second timeout per message
        
        let result = timeout(stream_timeout, async {
            while let Some(response) = timeout(message_timeout, stream.message()).await?? {
                if let Some(contract_entry) = response.contract_entry {
                    use get_active_contracts_response::ContractEntry;
                    if let ContractEntry::ActiveContract(active_contract) = contract_entry {
                        contracts.push(active_contract);
                    }
                }
                
                // Limit the number of contracts to prevent memory issues
                if contracts.len() >= 10000 {
                    break;
                }
            }
            Ok::<_, anyhow::Error>(())
        }).await;
        
        match result {
            Ok(_) => Ok(contracts),
            Err(_) => {
                tracing::warn!("Active contracts stream timed out, returning {} contracts collected so far", contracts.len());
                Ok(contracts)
            }
        }
    }

    pub async fn get_balance(&self) -> Result<serde_json::Value> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let token = generate_jwt_token(&self.config.jwt_secret, &self.config.jwt_audience, &self.config.jwt_user)?;
        
        let response = timeout(Duration::from_secs(15), async {
            client
                .get(format!("{}/api/validator/v0/wallet/balance", self.config.validator_endpoint()))
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?
                .json::<serde_json::Value>()
                .await
        }).await??;
        
        Ok(response)
    }
}