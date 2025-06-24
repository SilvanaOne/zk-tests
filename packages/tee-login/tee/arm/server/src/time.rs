use anyhow::{Result, anyhow};
use std::io::Read;
use tracing::{error, info, warn};
use vsock::VsockStream;

#[derive(Debug)]
pub enum TimeError {
    ClockError,
    InternalError,
    ConnectionError(String),
    ProtocolError(String),
}

impl std::fmt::Display for TimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeError::ClockError => write!(f, "Server clock error"),
            TimeError::InternalError => write!(f, "Server internal error"),
            TimeError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            TimeError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
        }
    }
}

impl std::error::Error for TimeError {}

/// Get current time from the VSOCK time server running in the AWS Nitro enclave
///
/// Connects to the time server on VSOCK port 5555 and retrieves the current
/// timestamp in milliseconds since Unix epoch.
///
/// # Returns
/// - `Ok(u64)` - Current timestamp in milliseconds
/// - `Err(TimeError)` - Various error conditions
///
/// # Protocol
/// The server responds with a 9-byte message:
/// - Byte 0: Status (0=Success, 1=ClockError, 2=InternalError)
/// - Bytes 1-8: Timestamp as big-endian u64 (only valid when status=0)
///
/// # Example
/// ```rust
/// match get_enclave_time() {
///     Ok(timestamp) => println!("Current time: {} ms", timestamp),
///     Err(e) => eprintln!("Failed to get time: {}", e),
/// }
/// ```
pub fn get_enclave_time() -> Result<u64, TimeError> {
    info!("Requesting time from VSOCK time server on port 5555");

    // Connect to the parent instance (CID 3) on port 5555
    let mut stream = VsockStream::connect_with_cid_port(3, 5555).map_err(|e| {
        error!("Failed to connect to VSOCK time server: {}", e);
        TimeError::ConnectionError(format!("Failed to connect: {}", e))
    })?;

    info!("Connected to VSOCK time server");

    // Read the 9-byte response
    let mut response = [0u8; 9];
    stream.read_exact(&mut response).map_err(|e| {
        error!("Failed to read response from time server: {}", e);
        TimeError::ConnectionError(format!("Failed to read response: {}", e))
    })?;

    // Parse the response according to the protocol
    let status = response[0];
    match status {
        0 => {
            // Success - extract timestamp from bytes 1-8
            let timestamp_bytes: [u8; 8] = response[1..9]
                .try_into()
                .map_err(|_| TimeError::ProtocolError("Invalid timestamp bytes".to_string()))?;

            let timestamp = u64::from_be_bytes(timestamp_bytes);
            info!("Successfully received timestamp: {} ms", timestamp);
            Ok(timestamp)
        }
        1 => {
            warn!("Time server reported clock error");
            Err(TimeError::ClockError)
        }
        2 => {
            warn!("Time server reported internal error");
            Err(TimeError::InternalError)
        }
        _ => {
            error!("Unknown status code from time server: {}", status);
            Err(TimeError::ProtocolError(format!(
                "Unknown status code: {}",
                status
            )))
        }
    }
}

/// Convenience function that returns the current time as a Result<u64>
/// for easier integration with existing error handling patterns
pub fn get_current_time_ms() -> Result<u64> {
    get_enclave_time().map_err(|e| anyhow!("Time service error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_error_display() {
        assert_eq!(format!("{}", TimeError::ClockError), "Server clock error");
        assert_eq!(
            format!("{}", TimeError::InternalError),
            "Server internal error"
        );
        assert_eq!(
            format!("{}", TimeError::ConnectionError("test".to_string())),
            "Connection error: test"
        );
        assert_eq!(
            format!("{}", TimeError::ProtocolError("test".to_string())),
            "Protocol error: test"
        );
    }
}
