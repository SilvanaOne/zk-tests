use axum::body::Body;
use axum::http::Request;
use std::net::IpAddr;
use tracing::{error, info};

pub fn log_login_success(ip: IpAddr, chain: &str, wallet: &str, address: &str) {
    info!(
        event = "login_success",
        client_ip = %ip,
        chain = chain,
        wallet = wallet,
        address = address,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Login successful"
    );
}

pub fn log_login_error(ip: IpAddr, chain: &str, wallet: &str, address: &str, error: &str) {
    error!(
        event = "login_error",
        client_ip = %ip,
        chain = chain,
        wallet = wallet,
        address = address,
        error = error,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Login failed"
    );
}

pub fn log_database_error(operation: &str, error: &str) {
    error!(
        event = "database_error",
        operation = operation,
        error = error,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Database error occurred"
    );
}

pub fn log_verification_error(chain: &str, address: &str, error: &str) {
    error!(
        event = "verification_error",
        chain = chain,
        address = address,
        error = error,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Signature verification failed"
    );
}

pub fn log_encryption_error(error: &str) {
    error!(
        event = "encryption_error",
        error = error,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "Encryption error occurred"
    );
}

pub fn get_client_ip(req: &Request<Body>) -> IpAddr {
    // Try to get real IP from headers (for reverse proxy setups)
    if let Some(forwarded_for) = req.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded_for.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }

    if let Some(real_ip) = req.headers().get("X-Real-IP") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            if let Ok(ip) = real_ip_str.parse::<IpAddr>() {
                return ip;
            }
        }
    }

    // Fallback to localhost
    "127.0.0.1".parse().unwrap()
}
