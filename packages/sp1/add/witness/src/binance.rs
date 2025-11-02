use anyhow::Result;
use price_lib::{CertificateChain, CertificateInfo, PriceData};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_rustls::{rustls, TlsConnector};
use tracing::{debug, info};
use x509_parser::prelude::*;

#[derive(Deserialize)]
struct BinanceTicker {
    #[serde(rename = "lastPrice")]
    last_price: String,
}

const BINANCE_API_DOMAIN: &str = "api.binance.com";
const BINANCE_API_URL: &str = "https://api.binance.com/api/v3/ticker/24hr";
const BINANCE_FUTURES_API_DOMAIN: &str = "fapi.binance.com";
const BINANCE_FUTURES_API_URL: &str = "https://fapi.binance.com/fapi/v1/ticker/24hr";

/// Check if error is "Invalid symbol" from Binance API
fn is_invalid_symbol_error(error_text: &str) -> bool {
    error_text.contains("-1121") || error_text.contains("Invalid symbol")
}

/// API source for price data
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiSource {
    Spot,
    Futures,
}

/// Fetch price from Binance Spot API
async fn fetch_from_spot(symbol: &str) -> Result<(PriceData, ApiSource)> {
    debug!("Trying Spot API for {}", symbol);

    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .build()?;

    let response = client
        .get(BINANCE_API_URL)
        .query(&[("symbol", symbol)])
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("Spot API error: {}", error_text);
    }

    let ticker: BinanceTicker = response.json().await?;
    let timestamp_fetched = chrono::Utc::now().timestamp_millis() as u64;

    info!("✓ Price fetched from Spot API");

    Ok((
        PriceData {
            symbol: symbol.to_string(),
            price: ticker.last_price,
            timestamp_fetched,
        },
        ApiSource::Spot,
    ))
}

/// Fetch price from Binance Futures API
async fn fetch_from_futures(symbol: &str) -> Result<(PriceData, ApiSource)> {
    debug!("Trying Futures API for {}", symbol);

    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .build()?;

    let response = client
        .get(BINANCE_FUTURES_API_URL)
        .query(&[("symbol", symbol)])
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("Futures API error: {}", error_text);
    }

    let ticker: BinanceTicker = response.json().await?;
    let timestamp_fetched = chrono::Utc::now().timestamp_millis() as u64;

    info!("✓ Price fetched from Futures API");

    Ok((
        PriceData {
            symbol: symbol.to_string(),
            price: ticker.last_price,
            timestamp_fetched,
        },
        ApiSource::Futures,
    ))
}

/// Fetch price from Binance API with automatic Spot → Futures fallback
/// Returns (PriceData, ApiSource) to indicate which API was used
pub async fn fetch_price(symbol: &str) -> Result<(PriceData, ApiSource)> {
    info!("Fetching {} price from Binance", symbol);

    // Try Spot API first
    match fetch_from_spot(symbol).await {
        Ok(result) => Ok(result),
        Err(e) => {
            let error_msg = e.to_string();

            // If it's an invalid symbol error, try Futures API
            if is_invalid_symbol_error(&error_msg) {
                info!("Symbol not found on Spot, trying Futures API...");
                fetch_from_futures(symbol).await
            } else {
                // Other errors should be propagated
                Err(e)
            }
        }
    }
}

/// Verify Binance TLS certificate and capture the certificate chain
/// Verifies the certificate for the specific API source that was used
pub async fn verify_binance_certificate(source: ApiSource) -> Result<CertificateChain> {
    let (domain, endpoint) = match source {
        ApiSource::Spot => {
            info!("Verifying Binance Spot API TLS certificate chain");
            (BINANCE_API_DOMAIN, "/api/v3/ping")
        }
        ApiSource::Futures => {
            info!("Verifying Binance Futures API TLS certificate chain");
            (BINANCE_FUTURES_API_DOMAIN, "/fapi/v1/ping")
        }
    };
    let port = 443;

    // Install the default crypto provider (using aws-lc-rs or ring)
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Create a rustls configuration with our hardcoded root certificate
    let mut root_store = rustls::RootCertStore::empty();

    // Load only the DigiCert Global Root G2 certificate from our hardcoded constant
    let cert_pem = price_lib::certs::DIGICERT_GLOBAL_ROOT_G2_PEM;

    let mut reader = std::io::BufReader::new(cert_pem.as_bytes());
    let certs: Vec<_> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Failed to parse root certificate: {:?}", e))?;

    for cert in certs {
        root_store
            .add(cert)
            .map_err(|e| anyhow::anyhow!("Failed to add root certificate to store: {:?}", e))?;
    }

    debug!("Using DigiCert Global Root G2 certificate for verification");

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(std::sync::Arc::new(config));
    let server_name = rustls::pki_types::ServerName::try_from(domain)?;

    // Connect to the server
    let stream = TcpStream::connect(format!("{}:{}", domain, port)).await?;
    let mut tls_stream = connector.connect(server_name.clone(), stream).await?;

    // Send a simple HTTPS request to get the certificate
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        endpoint, domain
    );
    tls_stream.write_all(request.as_bytes()).await?;
    tls_stream.flush().await?;

    // Get the peer certificates
    let certs = tls_stream
        .get_ref()
        .1
        .peer_certificates()
        .ok_or_else(|| anyhow::anyhow!("No peer certificates found"))?;

    let mut certificates_der = Vec::new();
    let mut certificates_info = Vec::new();
    let mut leaf_fingerprint = String::new();
    let mut root_fingerprint = String::new();

    for (i, cert) in certs.iter().enumerate() {
        let cert_der = cert.as_ref();
        let (_, x509_cert) = X509Certificate::from_der(cert_der)
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate {}: {:?}", i, e))?;

        // Calculate SHA256 fingerprint
        let fingerprint = Sha256::digest(&cert_der);
        let fingerprint_hex = hex::encode(fingerprint);

        // Store fingerprints for leaf and root
        if i == 0 {
            leaf_fingerprint = fingerprint_hex.clone();

            // Verify the leaf certificate is for Binance
            let subject_str = x509_cert.subject().to_string();
            if !subject_str.contains("binance.com") {
                anyhow::bail!(
                    "Certificate subject does not match Binance: {}",
                    subject_str
                );
            }

            // Verify SANs (Subject Alternative Names) for the correct domain
            if let Ok(Some(san_ext)) = x509_cert.subject_alternative_name() {
                let mut found_binance = false;
                let expected_sans = match source {
                    ApiSource::Spot => vec!["*.binance.com", "api.binance.com"],
                    ApiSource::Futures => vec!["*.binance.com", "fapi.binance.com"],
                };

                for name in &san_ext.value.general_names {
                    if let GeneralName::DNSName(dns) = name {
                        if expected_sans.contains(&dns.as_ref()) {
                            found_binance = true;
                            debug!("Found valid SAN: {}", dns);
                            break;
                        }
                    }
                }
                if !found_binance {
                    anyhow::bail!(
                        "Certificate does not have valid SANs for {:?} API (expected: {:?})",
                        source,
                        expected_sans
                    );
                }
            }
        }

        // Last certificate is the root
        if i == certs.len() - 1 {
            root_fingerprint = fingerprint_hex.clone();
        }

        // Check if certificate is currently valid
        let now = ASN1Time::from_timestamp(chrono::Utc::now().timestamp())
            .map_err(|e| anyhow::anyhow!("Failed to get current time: {:?}", e))?;

        if !x509_cert.validity().is_valid_at(now) {
            anyhow::bail!("Certificate {} is not valid at current time", i);
        }

        certificates_der.push(cert_der.to_vec());
        certificates_info.push(CertificateInfo {
            subject: x509_cert.subject().to_string(),
            issuer: x509_cert.issuer().to_string(),
            valid_from: x509_cert.validity().not_before.to_string(),
            valid_until: x509_cert.validity().not_after.to_string(),
            sha256_fingerprint: fingerprint_hex,
        });

        debug!(
            "Certificate {}: {} (fingerprint: {})",
            i,
            x509_cert.subject(),
            certificates_info.last().unwrap().sha256_fingerprint
        );
    }

    Ok(CertificateChain {
        certificates_der,
        certificates_info,
        verified: true,
        leaf_fingerprint,
        root_fingerprint,
    })
}

/// Fetch price and verify TLS certificate in one operation
/// Ensures TLS verification is performed on the ACTUAL domain used to fetch the price
pub async fn fetch_and_verify_price(symbol: &str) -> Result<(PriceData, CertificateChain)> {
    let (price, source) = fetch_price(symbol).await?;
    let cert_chain = verify_binance_certificate(source).await?;

    info!(
        "Price fetched and verified from {:?} API: {} = {}",
        source, symbol, price.price
    );

    Ok((price, cert_chain))
}
