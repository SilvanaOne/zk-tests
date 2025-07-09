use futures::stream::StreamExt;
use parking_lot::Mutex;
use reqwest::StatusCode;
use serde_json::json;
use shared_crypto::intent::Intent;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::time::Instant;
use sui_keys::keystore::{AccountKeystore, InMemKeystore};
use sui_sdk::rpc_types::{
    Coin, SuiMoveStruct, SuiMoveValue, SuiObjectDataOptions, SuiParsedData,
    SuiTransactionBlockResponseOptions,
};
use sui_sdk::types::{
    Identifier,
    base_types::ObjectID,
    digests::TransactionDigest,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{Argument, CallArg, Command, ObjectArg, Transaction, TransactionData},
};
use sui_sdk::types::{base_types::SuiAddress, crypto::SuiKeyPair, object::Owner};
use sui_sdk::{SuiClient, SuiClientBuilder};

const MIN_BALANCE: u64 = 100_000_000;
const MAX_BALANCE: u64 = 200_000_000;
const MIN_FAUCET_COIN_BALANCE: u64 = 5_000_000_000;

static SUI_CLIENT: OnceLock<SuiClient> = OnceLock::new();

pub async fn get_sui_client() -> Result<&'static SuiClient, anyhow::Error> {
    if let Some(client) = SUI_CLIENT.get() {
        Ok(client)
    } else {
        let client = SuiClientBuilder::default().build_devnet().await?;
        let _ = SUI_CLIENT.set(client.clone());
        let client = SUI_CLIENT.get();
        match client {
            Some(client) => Ok(client),
            None => Err(anyhow::anyhow!("Sui client not found")),
        }
    }
}

#[derive(Debug)]
pub struct RequestData {
    pub nonce: u64,
    pub agent: String,
    pub action: String,
    pub request: String,
}

const REQUEST_OBJECT_ID: &str =
    "0xca06480ff08a05c51a0aa80e8d74a655533c879370004e9e59b56f81bcb4ba3f"; // devnet
// "0x904a847618f0a6724e3a8894286310190c4e53aa81d8ac61ddd1f073c6881a15"; testnet

const PACKAGE_ID: &str = "0xa34907868de25ec7e2bbb8e22021a3e702eb408bf87ec2bc3141a4c6b498ca01"; // devnet
const RESPONSE_OBJECT_ID: &str =
    "0x0a1ed77d28c4aa78ecf871c8ac817eb4763d03ed39289f5123f374a4f9d31318"; // devnet

pub async fn get_request() -> Result<RequestData, Box<dyn std::error::Error>> {
    let sui_client = get_sui_client().await?;
    let response = sui_client
        .read_api()
        .get_object_with_options(
            REQUEST_OBJECT_ID.parse()?,
            sui_sdk::rpc_types::SuiObjectDataOptions::new().with_content(),
        )
        .await?;

    // Extract fields from the response
    let content = response.data.unwrap().content.unwrap();
    if let SuiParsedData::MoveObject(move_obj) = content {
        // Handle SuiMoveStruct properly
        if let SuiMoveStruct::WithFields(fields_map) = move_obj.fields {
            // Extract the fields from the map
            let nonce = match fields_map.get("nonce") {
                Some(SuiMoveValue::String(s)) => s.clone(),
                _ => String::default(),
            };

            let agent = match fields_map.get("name") {
                Some(SuiMoveValue::String(s)) => s.clone(),
                _ => String::default(),
            };

            let action = match fields_map.get("action") {
                Some(SuiMoveValue::String(s)) => s.clone(),
                _ => String::default(),
            };

            let request = match fields_map.get("request") {
                Some(SuiMoveValue::String(s)) => s.clone(),
                _ => String::default(),
            };

            Ok(RequestData {
                nonce: nonce.parse::<u64>()?,
                agent,
                action,
                request,
            })
        } else {
            Err("Object fields not in expected format".into())
        }
    } else {
        Err("Invalid object format".into())
    }
}

/// Sends a response transaction to the Sui blockchain
/// This function mirrors the coordination function from coordinate.ts
///
/// # Arguments
/// * `sui_client` - The Sui client instance
/// * `private_key` - Hex-encoded private key for signing
/// * `agent` - Agent identifier
/// * `action` - Action being performed  
/// * `data` - Response data
///
/// # Returns
/// Transaction digest on success
///
/// # Example usage (from coordinate.ts):
/// ```typescript
/// const tx = new Transaction();
/// const args = [
///   tx.object(responseObjectID),
///   tx.pure.string(agent),
///   tx.pure.string(action),
///   tx.pure.string(data),
/// ];
/// tx.moveCall({
///   package: packageID,
///   module: "agent",
///   function: "agent_response",
///   arguments: args,
/// });
/// ```
pub async fn reply_to_request(
    private_key: &str,
    agent: &str,
    action: &str,
    data: &str,
    gas_budget: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    let sui_client = get_sui_client().await?;

    let mut keystore = InMemKeystore::default();
    let keypair = match SuiKeyPair::decode(private_key) {
        Ok(kp) => kp,
        Err(e) => {
            println!("Error parsing private key: {}", e);
            return Err("Invalid private key".into());
        }
    };
    let sender = SuiAddress::from(&keypair.public());
    keystore.add_key(Some("sender".to_string()), keypair)?;

    let mut ptb = ProgrammableTransactionBuilder::new();

    let package_id = ObjectID::from_hex_literal(PACKAGE_ID)?;
    let response_object_id = ObjectID::from_hex_literal(RESPONSE_OBJECT_ID)?;
    let object = sui_client
        .read_api()
        .get_object_with_options(
            response_object_id,
            SuiObjectDataOptions {
                show_type: true,
                show_owner: true,
                show_previous_transaction: false,
                show_display: false,
                show_content: false,
                show_bcs: false,
                show_storage_rebate: false,
            },
        )
        .await?;
    //println!("Object: {:?}", object);
    let owner = object.owner().clone().unwrap();
    let object_ref = object.data.unwrap().object_ref();

    let obj_arg = match owner {
        Owner::AddressOwner(_) => ObjectArg::ImmOrOwnedObject(object_ref),
        Owner::Immutable => ObjectArg::ImmOrOwnedObject(object_ref),
        Owner::Shared {
            initial_shared_version,
        }
        | Owner::ConsensusV2 {
            start_version: initial_shared_version,
            ..
        } => ObjectArg::SharedObject {
            id: object_ref.0,
            initial_shared_version,
            mutable: true,
        },
        Owner::ObjectOwner(_) => {
            println!("Cannot use an object-owned object as an argument");
            //release_coin_lock(gas_coin_id); // Release lock before returning error (removed per RAII guard)
            return Err("Cannot use an object-owned object as an argument".into());
        }
    };
    let response_obj_arg = CallArg::Object(obj_arg);

    let tx_executed_start = Instant::now();

    let response_arg = ptb.input(response_obj_arg)?;

    let agent_input = CallArg::Pure(bcs::to_bytes(agent)?);
    let agent_arg = ptb.input(agent_input)?;

    let action_input = CallArg::Pure(bcs::to_bytes(action)?);
    let action_arg = ptb.input(action_input)?;

    let data_input = CallArg::Pure(bcs::to_bytes(data)?);
    let data_arg = ptb.input(data_input)?;

    let module = Identifier::new("agent")?;
    let function = Identifier::new("agent_response")?;

    ptb.command(Command::move_call(
        package_id,
        module,
        function,
        vec![], // no type arguments
        vec![response_arg, agent_arg, action_arg, data_arg],
    ));

    let builder = ptb.finish();
    let gas_price = sui_client.read_api().get_reference_gas_price().await?;
    let (gas_coin, _coin_guard) = match fetch_coin(sui_client, &sender, gas_budget).await? {
        Some((coin, _coin_guard)) => (coin, _coin_guard),
        None => {
            println!("No unlocked coins available with sufficient balance, using faucet...");
            check_gas_coins(private_key, true, true).await?;
            match fetch_coin(sui_client, &sender, gas_budget).await? {
                Some((coin, _coin_guard)) => (coin, _coin_guard),
                None => {
                    println!("No unlocked coins available with sufficient balance");
                    return Err("No unlocked coins available with sufficient balance".into());
                }
            }
        }
    };

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        builder,
        gas_budget,
        gas_price,
    );

    // 7) Sign transaction
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::sui_transaction())?;

    let tx_sending_start = Instant::now();

    // Execute transaction and handle both success and error cases for coin unlocking
    let result = sui_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            SuiTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await;

    // Always release the coin lock, regardless of transaction success/failure (removed per RAII guard)
    //release_coin_lock(gas_coin_id);

    match result {
        Ok(transaction_response) => {
            println!(
                "Transaction digest: {}, executed in {:?} ms, sent in {:?} ms",
                transaction_response.digest,
                tx_executed_start.elapsed().as_millis(),
                tx_sending_start.elapsed().as_millis()
            );

            // Print events if any
            // if let Some(events) = transaction_response.events {
            //     for event in events.data {
            //         println!("Event: {:?}", event.parsed_json);
            //     }
            // }

            Ok(transaction_response.digest.to_string())
        }
        Err(e) => {
            println!("Transaction failed: {}", e);
            Err(e.into())
        }
    }
}

async fn fetch_coin(
    sui: &SuiClient,
    sender: &SuiAddress,
    min_balance: u64,
) -> Result<Option<(Coin, CoinLockGuard)>, anyhow::Error> {
    const MAX_RETRIES: u32 = 6;
    const RETRY_DELAY_MS: u64 = 500;
    let lock_manager = get_coin_lock_manager();

    for attempt in 1..=MAX_RETRIES {
        let coin_type = "0x2::sui::SUI".to_string();
        let coins_stream = sui
            .coin_read_api()
            .get_coins_stream(*sender, Some(coin_type));

        // Iterate through coins to find one that meets balance requirements and is not locked
        let mut coins = coins_stream.boxed();

        while let Some(coin) = coins.next().await {
            if (coin.balance >= min_balance) && (coin.balance < MIN_FAUCET_COIN_BALANCE) {
                let coin_id = coin.coin_object_id;

                // Try to lock this coin atomically
                if let Some(guard) = lock_manager.try_lock_coin(coin_id) {
                    return Ok(Some((coin, guard)));
                } else {
                    continue;
                }
            }
        }

        if attempt < MAX_RETRIES {
            tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
        }
    }

    println!(
        "No unlocked coins found with sufficient balance after {} attempts",
        MAX_RETRIES
    );
    Ok(None)
}

// Coin lock manager to prevent concurrent usage
#[derive(Clone)]
pub struct CoinLockManager {
    locks: Arc<Mutex<HashMap<ObjectID, Instant>>>,
    lock_timeout: Duration,
}

/// RAII‐style guard returned by `try_lock_coin`.  
/// When this guard is dropped, the coin lock is automatically released.
struct CoinLockGuard {
    manager: CoinLockManager,
    coin_id: ObjectID,
}

impl Drop for CoinLockGuard {
    fn drop(&mut self) {
        self.manager.release_coin(self.coin_id);
    }
}

impl CoinLockManager {
    fn new(lock_timeout_seconds: u64) -> Self {
        Self {
            locks: Arc::new(Mutex::new(HashMap::new())),
            lock_timeout: Duration::from_secs(lock_timeout_seconds),
        }
    }

    /// Attempts to lock a coin for exclusive use.
    /// Returns `Some(CoinLockGuard)` if the coin was successfully locked; `None` otherwise.
    fn try_lock_coin(&self, coin_id: ObjectID) -> Option<CoinLockGuard> {
        let mut locks = self.locks.lock();

        // Clean up expired locks first
        let now = Instant::now();
        locks.retain(|_, lock_time| now.duration_since(*lock_time) < self.lock_timeout);

        use std::collections::hash_map::Entry;
        match locks.entry(coin_id) {
            Entry::Occupied(_) => None, // already locked
            Entry::Vacant(entry) => {
                entry.insert(now);
                Some(CoinLockGuard {
                    manager: self.clone(),
                    coin_id,
                })
            }
        }
    }

    /// Releases a coin lock
    fn release_coin(&self, coin_id: ObjectID) {
        let mut locks = self.locks.lock();
        locks.remove(&coin_id);
    }

    /// Checks if a coin is currently locked
    #[allow(dead_code)]
    fn is_locked(&self, coin_id: ObjectID) -> bool {
        let mut locks = self.locks.lock();

        // Clean up expired locks first
        let now = Instant::now();
        locks.retain(|_, lock_time| now.duration_since(*lock_time) < self.lock_timeout);

        locks.contains_key(&coin_id)
    }
}

// Global coin lock manager instance

static COIN_LOCK_MANAGER: OnceLock<CoinLockManager> = OnceLock::new();

fn get_coin_lock_manager() -> &'static CoinLockManager {
    COIN_LOCK_MANAGER.get_or_init(|| CoinLockManager::new(30)) // 30 seconds timeout
}

async fn faucet(address: &SuiAddress) -> Result<(), anyhow::Error> {
    request_tokens_from_faucet(address, "https://faucet.devnet.sui.io/v2/gas".to_string()).await?;
    Ok(())
}

#[derive(serde::Deserialize, Debug)]
struct FaucetResponse {
    error: Option<String>,
}

async fn request_tokens_from_faucet(
    address: &SuiAddress,
    url: String,
) -> Result<(), anyhow::Error> {
    let address_str = address.to_string();
    let json_body = json![{
        "FixedAmountRequest": {
            "recipient": &address_str
        }
    }];

    // make the request to the faucet JSON RPC API for coin
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(http::header::USER_AGENT, "sui-faucet-client")
        .json(&json_body)
        .send()
        .await?;

    match resp.status() {
        StatusCode::ACCEPTED | StatusCode::CREATED | StatusCode::OK => {
            let faucet_resp: FaucetResponse = resp.json().await?;
            println!("Faucet response: {:?}", faucet_resp);

            if let Some(err) = faucet_resp.error {
                return Err(anyhow::anyhow!("Faucet request was unsuccessful: {err}").into());
            } else {
                println!(
                    "Request successful. It can take up to 1 minute to get the coin. Run sui client gas to check your gas coins."
                );
            }
        }
        StatusCode::BAD_REQUEST => {
            let faucet_resp: FaucetResponse = resp.json().await?;
            if let Some(err) = faucet_resp.error {
                return Err(anyhow::anyhow!("Faucet request was unsuccessful. {err}").into());
            }
        }
        StatusCode::TOO_MANY_REQUESTS => {
            return Err(anyhow::anyhow!("Faucet service received too many requests from this IP address. Please try again after 60 minutes.").into());
        }
        StatusCode::SERVICE_UNAVAILABLE => {
            return Err(anyhow::anyhow!(
                "Faucet service is currently overloaded or unavailable. Please try again later."
            )
            .into());
        }
        status_code => {
            return Err(anyhow::anyhow!("Faucet request was unsuccessful: {status_code}").into());
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct GasCoins {
    pub faucet_coin: Option<Coin>,
    pub gas_coins: usize,
}

async fn get_gas_coins(
    sui: &SuiClient,
    sender: &SuiAddress,
    min_balance: u64,
    max_balance: u64,
    min_faucet_coin_balance: u64,
) -> Result<GasCoins, anyhow::Error> {
    let coin_type = "0x2::sui::SUI".to_string();
    let mut gas_coins = 0;
    let mut faucet_coin: Option<Coin> = None;
    let coins_stream = sui
        .coin_read_api()
        .get_coins_stream(*sender, Some(coin_type));

    // Iterate through coins
    let mut coins = coins_stream.boxed();
    let lock_manager = get_coin_lock_manager();

    while let Some(coin) = coins.next().await {
        if coin.balance >= min_balance && coin.balance <= max_balance {
            if lock_manager.is_locked(coin.coin_object_id) {
                continue;
            }
            gas_coins += 1;
        } else if coin.balance >= min_faucet_coin_balance {
            faucet_coin = Some(coin);
        }
    }

    Ok(GasCoins {
        faucet_coin,
        gas_coins,
    })
}

async fn split_gas_coins(
    sui: &SuiClient,
    new_gas_coins: usize,
    new_coin_balance: u64,
    sender: &SuiAddress,
    keystore: &InMemKeystore,
    faucet_coin: Coin,
) -> Result<TransactionDigest, anyhow::Error> {
    // set the maximum gas budget
    let max_gas_budget = 50_000_000;
    let coin_balance = faucet_coin.balance;
    if new_gas_coins as u64 * new_coin_balance + max_gas_budget > coin_balance {
        return Err(anyhow::anyhow!("Not enough gas coin balance to split").into());
    }

    // get the reference gas price from the network
    let gas_price = sui.read_api().get_reference_gas_price().await?;

    // now we programmatically build the transaction through several commands
    let mut ptb = ProgrammableTransactionBuilder::new();
    // first, we want to split the coin, and we specify how much SUI (in MIST) we want
    // for the new coin
    let split_coin_amounts = vec![ptb.pure(new_coin_balance)?; new_gas_coins];
    let rec_arg = ptb.pure(sender).unwrap();
    let result = ptb.command(Command::SplitCoins(Argument::GasCoin, split_coin_amounts));
    let Argument::Result(result) = result else {
        return Err(anyhow::anyhow!(
            "self.command should always give a Argument::Result"
        ));
    };
    ptb.command(Command::TransferObjects(
        (0..new_gas_coins)
            .map(|i| Argument::NestedResult(result, i as u16))
            .collect(),
        rec_arg,
    ));

    // we finished constructing our PTB and we need to call finish
    let builder = ptb.finish();

    // using the PTB that we just constructed, create the transaction data
    // that we will submit to the network
    let tx_data = TransactionData::new_programmable(
        *sender,
        vec![faucet_coin.object_ref()],
        builder,
        max_gas_budget,
        gas_price,
    );

    let signature = keystore.sign_secure(sender, &tx_data, Intent::sui_transaction())?;

    let transaction_response = sui
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            SuiTransactionBlockResponseOptions::new(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    Ok(transaction_response.digest)
}

static CHECKING_GAS_COINS: AtomicBool = AtomicBool::new(false);

struct CheckGasCoinsLockGuard {}

impl CheckGasCoinsLockGuard {
    fn new() -> Self {
        CHECKING_GAS_COINS.store(true, Ordering::SeqCst);
        Self {}
    }
}

impl Drop for CheckGasCoinsLockGuard {
    fn drop(&mut self) {
        CHECKING_GAS_COINS.store(false, Ordering::SeqCst);
    }
}

pub async fn check_gas_coins(
    private_key: &str,
    wait: bool,
    force: bool,
) -> Result<(), anyhow::Error> {
    println!("Checking gas coins, wait: {}", wait);
    if CHECKING_GAS_COINS.load(Ordering::SeqCst) {
        if wait == false {
            return Ok(());
        }
        loop {
            if !CHECKING_GAS_COINS.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        return Ok(());
    }
    let _guard = CheckGasCoinsLockGuard::new();
    println!("Starting faucet, wait: {}", wait);
    let mut keystore = InMemKeystore::default();
    let keypair = match SuiKeyPair::decode(private_key) {
        Ok(kp) => kp,
        Err(e) => {
            println!("Error parsing private key: {}", e);
            return Err(anyhow::anyhow!("Invalid private key").into());
        }
    };
    let sender = SuiAddress::from(&keypair.public());
    keystore.add_key(Some("sender".to_string()), keypair)?;
    let sui = get_sui_client().await?;
    let mut gas_coins = get_gas_coins(
        sui,
        &sender,
        MIN_BALANCE,
        MAX_BALANCE,
        MIN_FAUCET_COIN_BALANCE,
    )
    .await?;
    println!("Gas coins: {:?}", gas_coins);
    if !force && gas_coins.gas_coins > 10 {
        println!("Enough gas coins");
        return Ok(());
    }
    if gas_coins.faucet_coin.is_none() {
        println!("No faucet coin found, requesting from faucet");
        faucet(&sender)
            .await
            .map_err(|e| anyhow::anyhow!("Faucet request failed: {}", e))?;
        gas_coins = get_gas_coins(
            sui,
            &sender,
            MIN_BALANCE,
            MAX_BALANCE,
            MIN_FAUCET_COIN_BALANCE,
        )
        .await?;
        println!("New gas coins: {:?}", gas_coins);
    }
    match gas_coins.faucet_coin {
        Some(coin) => match split_gas_coins(sui, 20, MAX_BALANCE, &sender, &keystore, coin).await {
            Ok(digest) => {
                println!("Split gas coins: {:?}", digest);
                Ok(())
            }
            Err(e) => {
                return Err(e);
            }
        },
        None => {
            println!("No faucet coin found, cannot split");
            return Ok(());
        }
    }
}
