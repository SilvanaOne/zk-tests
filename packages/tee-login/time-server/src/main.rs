use anyhow::Result;
use log::{Log, error, info, warn};
use signal_hook::{consts::signal::*, iterator::Signals};
use simplelog::{ConfigBuilder, LevelFilter, WriteLogger};
use std::{
    fs::OpenOptions,
    io::Write,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use vsock::{VsockListener, get_local_cid};

#[derive(Debug)]
#[allow(dead_code)]
enum TimeResponse {
    Success(u64),  // timestamp in milliseconds
    ClockError,    // system clock failure
    InternalError, // other internal errors
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum ResponseStatus {
    Success = 0,
    ClockError = 1,
    InternalError = 2,
}

impl TimeResponse {
    /// Serialize the response to bytes for sending over VSOCK
    ///
    /// PROTOCOL SPECIFICATION (fixed 9 bytes):
    /// ┌─────────────┬─────────────────────────────────────────────────┐
    /// │   Byte 0    │                  Bytes 1-8                      │
    /// │ Status Flag │            Timestamp (big-endian u64)           │
    /// └─────────────┴─────────────────────────────────────────────────┘
    ///
    /// Status Flag values:
    /// - 0 = Success: Bytes 1-8 contain valid timestamp in milliseconds
    /// - 1 = ClockError: System clock failure, bytes 1-8 are zero
    /// - 2 = InternalError: Other server error, bytes 1-8 are zero
    ///
    /// Receiver parsing example:
    /// ```
    /// let status = response[0];
    /// match status {
    ///     0 => {
    ///         let timestamp = u64::from_be_bytes(response[1..9].try_into().unwrap());
    ///         // Use timestamp
    ///     }
    ///     1 => {
    ///         // Handle clock error
    ///     }
    ///     2 => {
    ///         // Handle internal error  
    ///     }
    ///     _ => {
    ///         // Unknown status
    ///     }
    /// }
    /// ```
    fn to_bytes(&self) -> [u8; 9] {
        let mut bytes = [0u8; 9];

        match self {
            TimeResponse::Success(timestamp) => {
                bytes[0] = ResponseStatus::Success as u8;
                bytes[1..9].copy_from_slice(&timestamp.to_be_bytes());
            }
            TimeResponse::ClockError => {
                bytes[0] = ResponseStatus::ClockError as u8;
                // bytes[1..9] remain 0 (timestamp field unused for errors)
            }
            TimeResponse::InternalError => {
                bytes[0] = ResponseStatus::InternalError as u8;
                // bytes[1..9] remain 0 (timestamp field unused for errors)
            }
        }

        bytes
    }
}

fn main() {
    // Initialize file logging
    setup_file_logging().unwrap_or_else(|e| {
        eprintln!(
            "Failed to setup logging: {}. Continuing without file logging.",
            e
        );
    });

    // Register signal handlers to log and exit gracefully
    if let Err(e) = setup_signal_handlers() {
        // If registering signals fails, log and continue without it.
        warn!("Failed to set up signal handlers: {}", e);
    }

    info!("Starting VSOCK time server...");

    // Main server loop - never exit
    loop {
        if let Err(e) = run_server() {
            error!(
                "Server encountered a critical error: {}. Restarting in 5 seconds...",
                e
            );
            thread::sleep(Duration::from_secs(5));
        }
    }
}

fn setup_file_logging() -> Result<()> {
    // Create or open the log file
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("vsock-time-server.log")?;

    // Configure logging with timestamps and thread info
    let config = ConfigBuilder::new()
        .set_time_format_rfc3339()
        .set_thread_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        .build();

    // Initialize the logger
    WriteLogger::init(LevelFilter::Info, config, log_file)?;

    Ok(())
}

fn run_server() -> Result<()> {
    // Retry loop for binding to VSOCK
    let listener = loop {
        match setup_listener() {
            Ok(listener) => {
                info!("Successfully bound to VSOCK");
                break listener;
            }
            Err(e) => {
                error!(
                    "Failed to setup VSOCK listener: {}. Retrying in 3 seconds...",
                    e
                );
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        }
    };

    // Accept connections loop - handle each connection individually
    info!("Server ready, accepting connections...");
    for stream_result in listener.incoming() {
        match stream_result {
            Ok(mut stream) => {
                if let Err(e) = handle_connection(&mut stream) {
                    warn!(
                        "Error handling connection: {}. Continuing with next connection...",
                        e
                    );
                }
            }
            Err(e) => {
                error!("Error accepting connection: {}. Continuing...", e);
                // Small delay to prevent rapid error loops
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    Ok(())
}

fn setup_listener() -> Result<VsockListener> {
    // Get local CID with retry logic
    let cid = get_local_cid()?;
    info!("Local VSOCK CID: {}", cid);

    // Bind to port 5555
    let listener = VsockListener::bind_with_cid_port(cid, 5555)?;
    info!("Listening on VSOCK CID {} port {}", cid, 5555);

    Ok(listener)
}

fn handle_connection(stream: &mut dyn Write) -> Result<()> {
    info!("New connection accepted");

    // Get current time and create appropriate response
    let response = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let timestamp = duration.as_millis() as u64;
            info!("Generated timestamp: {} ms", timestamp);
            TimeResponse::Success(timestamp)
        }
        Err(e) => {
            warn!("Clock error occurred: {}", e);
            TimeResponse::ClockError
        }
    };

    // Send structured response
    let response_bytes = response.to_bytes();
    stream.write_all(&response_bytes)?;

    match response {
        TimeResponse::Success(ts) => info!("Sent successful timestamp: {} ms", ts),
        TimeResponse::ClockError => info!("Sent clock error response"),
        TimeResponse::InternalError => info!("Sent internal error response"),
    }

    Ok(())
}

/// Register handlers for termination signals so we can log when the process is asked to stop.
fn setup_signal_handlers() -> Result<()> {
    // We create a signal iterator over common termination signals.
    let mut signals = Signals::new([SIGTERM, SIGINT, SIGQUIT])?;

    // Spawn a thread dedicated to waiting for signals.
    thread::spawn(move || {
        for sig in signals.forever() {
            match sig {
                SIGTERM => info!("Received SIGTERM. Shutting down."),
                SIGINT => info!("Received SIGINT (Ctrl+C). Shutting down."),
                SIGQUIT => info!("Received SIGQUIT. Shutting down."),
                other => info!("Received signal {}. Shutting down.", other),
            }

            // Flush any buffered log records before exiting.
            log::logger().flush();

            // Exit with success status so supervisor can handle restarts as needed.
            std::process::exit(0);
        }
    });

    Ok(())
}
