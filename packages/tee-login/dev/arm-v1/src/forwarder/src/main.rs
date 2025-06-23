mod forwarder;

use std::env;
use std::thread;
use std::time::Duration;
use tracing::{error, info};

fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default configuration: info level for the application, warn for dependencies
        tracing_subscriber::EnvFilter::new("info,hyper=warn,h2=warn,tower=warn,reqwest=warn")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true) // Include the module path in logs
        .with_thread_ids(true) // Include thread IDs
        .with_level(true) // Include log level
        .with_file(true) // Include file name
        .with_line_number(true) // Include line number
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        eprintln!(
            "Usage: {} <local_ip> <local_port> <remote_cid> <remote_port>",
            args.get(0).map(|s| s.as_str()).unwrap_or("forwarder")
        );
        std::process::exit(1);
    }

    let local_ip = args[1].clone();
    let local_port: u16 = args[2]
        .parse()
        .unwrap_or_else(|e| fatal_parse("local_port", e));
    let remote_cid: u32 = args[3]
        .parse()
        .unwrap_or_else(|e| fatal_parse("remote_cid", e));
    let remote_port: u32 = args[4]
        .parse()
        .unwrap_or_else(|e| fatal_parse("remote_port", e));

    info!(
        "Starting forwarder on {}:{} -> CID {}:{}",
        local_ip, local_port, remote_cid, remote_port
    );

    // Keep restarting the server loop when it exits with an error.
    loop {
        if let Err(e) = forwarder::run_server(&local_ip, local_port, remote_cid, remote_port) {
            error!("Forwarder server exited: {}. Restarting in 1s", e);
            thread::sleep(Duration::from_secs(1));
        }
    }
}

fn fatal_parse<T: std::fmt::Display>(field: &str, err: T) -> ! {
    error!("Failed to parse {}: {}", field, err);
    std::process::exit(1);
}
