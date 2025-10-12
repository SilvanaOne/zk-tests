//! Root command handler - Calculate indexed merkle map root from key:value pairs

use anyhow::{Context, Result};
use tracing::info;

use crate::merkle;

/// Parse a key:value string into (i64, i64) tuple
///
/// # Arguments
/// * `pair_str` - String in format "key:value" (e.g., "1:20", "3:45")
///
/// # Returns
/// Tuple of (key, value) as i64 integers
fn parse_pair(pair_str: &str) -> Result<(i64, i64)> {
    let parts: Vec<&str> = pair_str.split(':').collect();

    if parts.len() != 2 {
        anyhow::bail!("Invalid pair format '{}'. Expected 'key:value'", pair_str);
    }

    let key = parts[0].parse::<i64>()
        .with_context(|| format!("Failed to parse key '{}' as integer", parts[0]))?;

    let value = parts[1].parse::<i64>()
        .with_context(|| format!("Failed to parse value '{}' as integer", parts[1]))?;

    Ok((key, value))
}

/// Handle the root command
///
/// # Arguments
/// * `pair_strings` - Vector of "key:value" strings (e.g., ["1:20", "3:45", "67:5685"])
///
/// # Returns
/// Result indicating success or failure
pub async fn handle_root(pair_strings: Vec<String>) -> Result<()> {
    // Handle empty list case
    if pair_strings.is_empty() {
        info!("Calculating root for empty indexed merkle map");
        let root = merkle::empty_root();
        println!("{}", root);
        return Ok(());
    }

    // Parse all key:value pairs
    let mut pairs = Vec::new();
    for pair_str in &pair_strings {
        let pair = parse_pair(pair_str)?;
        pairs.push(pair);
    }

    info!("Calculating root for {} key:value pairs", pairs.len());
    info!("Pairs: {:?}", pairs);

    // Calculate root
    let root = merkle::calculate_root(&pairs)?;

    // Print result
    println!("{}", root);

    Ok(())
}
