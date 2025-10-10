//! URL utility functions for handling special localhost domains

use std::net::SocketAddr;

/// Creates a reqwest Client with proper .localhost domain resolution
/// Maps common .localhost domains to 127.0.0.1
pub fn create_client_with_localhost_resolution() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("billing-client/0.1")
        // Map all common .localhost domains to 127.0.0.1
        .resolve("scan.localhost", SocketAddr::from(([127, 0, 0, 1], 4000)))
        .resolve("canton.localhost", SocketAddr::from(([127, 0, 0, 1], 2000)))
        .resolve("canton.localhost", SocketAddr::from(([127, 0, 0, 1], 3000)))
        .resolve("canton.localhost", SocketAddr::from(([127, 0, 0, 1], 4000)))
        // Add port 3975 for direct API access
        .resolve("localhost", SocketAddr::from(([127, 0, 0, 1], 3975)))
        .resolve("localhost", SocketAddr::from(([127, 0, 0, 1], 2975)))
        .build()
}