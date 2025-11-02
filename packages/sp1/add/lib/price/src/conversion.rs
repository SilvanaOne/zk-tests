//! Price conversion utilities using Ethereum wei-style format
//!
//! Converts price strings to U256 using the formula: price_wei = price × 10^decimals
//! This preserves additivity: prices can be summed correctly in wei format.
//!
//! Example:
//! - "110231.50000000" (8 decimals) → 11023150000000 wei
//! - "0.25000000" (8 decimals) → 25000000 wei
//! - Sum: 11023175000000 wei = "110231.75000000" ✓

use alloy_sol_types::private::U256;

/// Binance uses 8 decimal places for prices
pub const PRICE_DECIMALS: u8 = 8;

/// Convert price string to U256 using wei-style format (price × 10^8)
///
/// # Examples
/// ```
/// use price_lib::conversion::parse_price_to_u256;
/// use alloy_sol_types::private::U256;
///
/// assert_eq!(
///     parse_price_to_u256("110231.50000000").unwrap(),
///     U256::from(11023150000000u128)
/// );
/// assert_eq!(
///     parse_price_to_u256("0.00000001").unwrap(),
///     U256::from(1u128)
/// );
/// ```
pub fn parse_price_to_u256(price: &str) -> Result<U256, String> {
    // Use alloy-primitives parse_units approach
    // This multiplies the decimal value by 10^PRICE_DECIMALS

    let mut amount = price.to_string();
    let dec_len = if let Some(di) = amount.find('.') {
        amount.remove(di);
        amount[di..].len()
    } else {
        0
    };

    let amount_str = amount.as_str();
    let exponent = PRICE_DECIMALS as usize;

    if dec_len > exponent {
        // Truncate decimal part if longer than exponent
        let amount_truncated = &amount_str[..(amount_str.len() - (dec_len - exponent))];
        U256::from_str_radix(amount_truncated, 10)
            .map_err(|e| format!("Failed to parse truncated price: {}", e))
    } else {
        // Multiply by 10^(exponent - dec_len) to shift decimal places
        let mut price_u256 = U256::from_str_radix(amount_str, 10)
            .map_err(|e| format!("Failed to parse price: {}", e))?;

        let multiplier = U256::from(10)
            .checked_pow(U256::from(exponent - dec_len))
            .ok_or("Overflow in price conversion")?;

        price_u256 = price_u256.checked_mul(multiplier)
            .ok_or("Overflow multiplying price")?;

        Ok(price_u256)
    }
}

/// Convert U256 price back to string with PRICE_DECIMALS precision
///
/// # Examples
/// ```
/// use price_lib::conversion::format_price_from_u256;
/// use alloy_sol_types::private::U256;
///
/// assert_eq!(
///     format_price_from_u256(U256::from(11023150000000u128)),
///     "110231.50000000"
/// );
/// assert_eq!(
///     format_price_from_u256(U256::from(1u128)),
///     "0.00000001"
/// );
/// ```
pub fn format_price_from_u256(wei: U256) -> String {
    let divisor = U256::from(10).pow(U256::from(PRICE_DECIMALS));

    let integer = wei / divisor;
    let decimals = wei % divisor;

    format!("{}.{:08}", integer, decimals)
}

/// Convert price string to bytes for Field conversion
pub fn price_to_bytes(price: &str) -> Result<[u8; 32], String> {
    let u256_value = parse_price_to_u256(price)?;
    Ok(u256_value.to_be_bytes())
}

/// Convert timestamp (u64) to bytes for Field conversion
pub fn timestamp_to_bytes(timestamp_ms: u64) -> [u8; 32] {
    let u256_value = U256::from(timestamp_ms);
    u256_value.to_be_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_price_integer_only() {
        // Integer with no decimals
        let result = parse_price_to_u256("110231").unwrap();
        let expected = U256::from(11023100000000u128); // 110231 × 10^8
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_price_with_decimals() {
        // Binance price format with 8 decimals
        let result = parse_price_to_u256("110231.50000000").unwrap();
        let expected = U256::from(11023150000000u128); // (110231 × 10^8) + 50000000
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_price_small_decimal() {
        // Smallest unit: 0.00000001
        let result = parse_price_to_u256("0.00000001").unwrap();
        assert_eq!(result, U256::from(1u128));
    }

    #[test]
    fn test_parse_price_less_than_8_decimals() {
        // Price with fewer decimals should be padded
        let result = parse_price_to_u256("123.45").unwrap();
        let expected = U256::from(12345000000u128); // 123.45 × 10^8
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_price_more_than_8_decimals() {
        // Should truncate to 8 decimals
        let result = parse_price_to_u256("123.456789012345").unwrap();
        let expected = U256::from(12345678901u128); // Truncated to 123.45678901
        assert_eq!(result, expected);
    }

    #[test]
    fn test_additivity_preserved() {
        // This is the KEY test - ensure additivity works!
        let price1 = parse_price_to_u256("110231.50000000").unwrap();
        let price2 = parse_price_to_u256("0.25000000").unwrap();
        let sum = price1 + price2;

        let expected_sum = parse_price_to_u256("110231.75000000").unwrap();
        assert_eq!(sum, expected_sum);

        // Verify the actual values
        assert_eq!(price1, U256::from(11023150000000u128));
        assert_eq!(price2, U256::from(25000000u128));
        assert_eq!(sum, U256::from(11023175000000u128));
    }

    #[test]
    fn test_format_price_basic() {
        let wei = U256::from(11023150000000u128);
        let formatted = format_price_from_u256(wei);
        assert_eq!(formatted, "110231.50000000");
    }

    #[test]
    fn test_format_price_smallest_unit() {
        let wei = U256::from(1u128);
        let formatted = format_price_from_u256(wei);
        assert_eq!(formatted, "0.00000001");
    }

    #[test]
    fn test_format_price_no_decimals() {
        let wei = U256::from(10000000000u128); // 100 × 10^8
        let formatted = format_price_from_u256(wei);
        assert_eq!(formatted, "100.00000000");
    }

    #[test]
    fn test_round_trip() {
        let original = "1163.56926418";
        let parsed = parse_price_to_u256(original).unwrap();
        let formatted = format_price_from_u256(parsed);

        // Should preserve first 8 decimals
        assert_eq!(formatted, "1163.56926418");
    }

    #[test]
    fn test_round_trip_small() {
        let original = "0.00000001";
        let parsed = parse_price_to_u256(original).unwrap();
        let formatted = format_price_from_u256(parsed);
        assert_eq!(formatted, "0.00000001");
    }

    #[test]
    fn test_real_binance_prices() {
        // Real BTC price example
        let btc_price = "110231.50000000";
        let btc_wei = parse_price_to_u256(btc_price).unwrap();
        assert_eq!(btc_wei, U256::from(11023150000000u128));
        assert_eq!(format_price_from_u256(btc_wei), btc_price);

        // Real ETH price example
        let eth_price = "3870.86000000";
        let eth_wei = parse_price_to_u256(eth_price).unwrap();
        assert_eq!(eth_wei, U256::from(387086000000u128));
        assert_eq!(format_price_from_u256(eth_wei), eth_price);
    }
}
