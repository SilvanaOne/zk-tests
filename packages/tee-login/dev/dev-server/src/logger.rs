use rocket::Response;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::request::{FromRequest, Outcome, Request};
use std::net::IpAddr;
use tracing::{error, info};

pub struct RequestLogger;

#[rocket::async_trait]
impl Fairing for RequestLogger {
    fn info(&self) -> Info {
        Info {
            name: "Request Logger",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _: &mut rocket::Data<'_>) {
        let method = req.method();
        let uri = req.uri();
        let client_ip = get_client_ip(req);

        info!(
            event = "request_received",
            method = %method,
            uri = %uri,
            client_ip = %client_ip,
            timestamp = %chrono::Utc::now().to_rfc3339(),
            "Request received"
        );
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let status = res.status();
        let client_ip = get_client_ip(req);

        if status.code >= 200 && status.code < 300 {
            // Log successful responses
            let method = req.method();
            let uri = req.uri();

            info!(
                event = "response_success",
                status = status.code,
                method = %method,
                uri = %uri,
                client_ip = %client_ip,
                timestamp = %chrono::Utc::now().to_rfc3339(),
                "Successful response"
            );
        } else {
            // Log error responses
            error!(
                event = "response_error",
                status = status.code,
                reason = status.reason(),
                method = %req.method(),
                uri = %req.uri(),
                client_ip = %client_ip,
                timestamp = %chrono::Utc::now().to_rfc3339(),
                "Error response"
            );
        }
    }
}

// Custom request guard to extract client IP
pub struct ClientIP(pub IpAddr);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientIP {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let ip = get_client_ip(req);
        Outcome::Success(ClientIP(ip))
    }
}

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

pub fn get_client_ip(req: &Request) -> IpAddr {
    // Try to get real IP from headers (for reverse proxy setups)
    if let Some(forwarded_for) = req.headers().get_one("X-Forwarded-For") {
        if let Some(first_ip) = forwarded_for.split(',').next() {
            if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }

    if let Some(real_ip) = req.headers().get_one("X-Real-IP") {
        if let Ok(ip) = real_ip.parse::<IpAddr>() {
            return ip;
        }
    }

    // Fallback to rocket's client IP
    req.client_ip()
        .unwrap_or_else(|| "127.0.0.1".parse().unwrap())
}
