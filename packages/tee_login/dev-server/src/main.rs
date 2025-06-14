#[macro_use]
extern crate rocket;

mod db;
mod encrypt;
mod logger;
mod login;
mod seed;
mod shamir;
mod solana;
mod sui;

use db::DBStore;
use logger::{ClientIP, RequestLogger, get_client_ip, log_login_error, log_login_success};
use login::process_login;
use login::{LoginRequest, LoginResponse};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::serde::json::Json;
use rocket::{Request, Response, State};
use std::fs;
use tracing::{error, info};
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

// Guard holder to keep file writers alive
pub struct LogGuards {
    _all_logs_guard: non_blocking::WorkerGuard,
    _error_logs_guard: non_blocking::WorkerGuard,
    _access_logs_guard: non_blocking::WorkerGuard,
}

#[rocket::options("/<_route_args..>")]
pub fn options(_route_args: Option<std::path::PathBuf>) {
    // Just to add CORS header via the fairing.
}

pub struct Cors;

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "Append CORS headers in responses",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _req: &'r Request<'_>, res: &mut Response<'r>) {
        res.set_raw_header("Access-Control-Allow-Origin", "*");
        res.set_raw_header(
            "Access-Control-Allow-Methods",
            "GET, POST, PUT, DELETE, OPTIONS",
        );
        res.set_raw_header("Access-Control-Allow-Headers", "*");
        res.set_raw_header("Access-Control-Allow-Credentials", "true");
    }
}

#[catch(404)]
fn not_found(req: &Request) -> String {
    let client_ip = get_client_ip(req);
    error!(
        event = "404_not_found",
        uri = %req.uri(),
        method = %req.method(),
        client_ip = %client_ip,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Page not found"
    );
    format!("Sorry, '{}' is not a valid path.", req.uri())
}

#[catch(500)]
fn internal_error(req: &Request) -> String {
    let client_ip = get_client_ip(req);
    error!(
        event = "500_internal_error",
        uri = %req.uri(),
        method = %req.method(),
        client_ip = %client_ip,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Internal server error"
    );
    "Internal server error".to_string()
}

#[catch(422)]
fn unprocessable_entity(req: &Request) -> String {
    let client_ip = get_client_ip(req);
    error!(
        event = "422_unprocessable_entity",
        uri = %req.uri(),
        method = %req.method(),
        client_ip = %client_ip,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Unprocessable entity"
    );
    "Unprocessable entity".to_string()
}

#[catch(400)]
fn bad_request(req: &Request) -> String {
    let client_ip = get_client_ip(req);
    error!(
        event = "400_bad_request",
        uri = %req.uri(),
        method = %req.method(),
        client_ip = %client_ip,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Bad request"
    );
    "Bad request".to_string()
}

fn setup_logging() -> Result<LogGuards, Box<dyn std::error::Error>> {
    // Create logs directory if it doesn't exist
    fs::create_dir_all("./logs")?;

    // Set up file appenders for different log levels
    let all_logs_appender = rolling::daily("./logs", "tee-wallet.log");
    let error_logs_appender = rolling::daily("./logs", "tee-wallet-errors.log");
    let access_logs_appender = rolling::daily("./logs", "tee-wallet-access.log");

    let (all_logs_writer, all_logs_guard) = non_blocking(all_logs_appender);
    let (error_logs_writer, error_logs_guard) = non_blocking(error_logs_appender);
    let (access_logs_writer, access_logs_guard) = non_blocking(access_logs_appender);

    // Create layers for different types of logs
    let all_logs_layer = fmt::layer()
        .json()
        .with_writer(all_logs_writer)
        .with_filter(EnvFilter::new("info"));

    let error_logs_layer = fmt::layer()
        .json()
        .with_writer(error_logs_writer)
        .with_filter(EnvFilter::new("error"));

    let access_logs_layer = fmt::layer()
        .json()
        .with_writer(access_logs_writer)
        .with_filter(EnvFilter::new("info"))
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            // Log only our application events with structured fields
            metadata.fields().field("event").is_some()
        }));

    // Console output for development - preserve Rocket's logs
    let console_layer = fmt::layer().compact().with_filter(
        EnvFilter::from_default_env()
            .add_directive("tee_wallet=info".parse()?)
            .add_directive("rocket=info".parse()?),
    );

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(all_logs_layer)
        .with(error_logs_layer)
        .with(access_logs_layer)
        .with(console_layer)
        .init();

    // Return guards to keep them alive
    Ok(LogGuards {
        _all_logs_guard: all_logs_guard,
        _error_logs_guard: error_logs_guard,
        _access_logs_guard: access_logs_guard,
    })
}

#[launch]
fn rocket() -> _ {
    // Initialize the logger with file output
    let _log_guards = match setup_logging() {
        Ok(guards) => guards,
        Err(e) => {
            eprintln!("Failed to initialize logging: {}", e);
            std::process::exit(1);
        }
    };

    info!("Starting TEE Wallet server...");

    // Initialize the database
    let db_store = match DBStore::open() {
        Ok(db) => {
            info!("Database initialized successfully");
            db
        }
        Err(e) => {
            error!("Failed to open database: {}", e);
            panic!("Failed to open database: {}", e);
        }
    };

    rocket::build()
        .manage(db_store) // Add database to Rocket's state management
        .manage(_log_guards) // Keep log guards alive by managing them in Rocket's state
        .mount("/", routes![options])
        .mount("/login", routes![login_route])
        .register(
            "/",
            catchers![not_found, internal_error, unprocessable_entity, bad_request],
        )
        .attach(Cors)
        .attach(RequestLogger)
}

#[post("/", data = "<login_request>")]
async fn login_route(
    login_request: Json<LoginRequest>,
    db: &State<DBStore>,
    client_ip: ClientIP,
) -> Json<LoginResponse> {
    let login_data = login_request.into_inner();

    info!(
        event = "login_attempt",
        client_ip = %client_ip.0,
        chain = login_data.chain,
        wallet = login_data.wallet,
        address = login_data.address,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Login attempt received"
    );

    let response = process_login(login_data.clone(), db).await;

    if response.success {
        log_login_success(
            client_ip.0,
            &login_data.chain,
            &login_data.wallet,
            &login_data.address,
        );
    } else {
        let error_msg = response.error.as_deref().unwrap_or("Unknown error");
        log_login_error(
            client_ip.0,
            &login_data.chain,
            &login_data.wallet,
            &login_data.address,
            error_msg,
        );
    }

    Json(response)
}
