use num_bigint::BigUint;
use num_traits::{One, Zero};
use std::fs;
use std::path::Path;
use std::str::FromStr;

/// BLS12-381 scalar field modulus
/// r = 52435875175126190479447740508185965837690552500527637822603658699938581184513
const BLS12_381_SCALAR_MODULUS: &str =
    "52435875175126190479447740508185965837690552500527637822603658699938581184513";

/// R constant from Move code  
/// 0x149fa8c209ab655fd480a3aff7d16dc72b6a3943e4b95fcf7909f42d9c17a552
const R_HEX: &str = "149fa8c209ab655fd480a3aff7d16dc72b6a3943e4b95fcf7909f42d9c17a552";

/// Convert BigUint to 32-byte big-endian hex string for Move constants
fn to_move_hex(value: &BigUint) -> String {
    let bytes = value.to_bytes_be();
    let mut padded = vec![0u8; 32];
    let start = 32 - bytes.len().min(32);
    padded[start..].copy_from_slice(&bytes[..bytes.len().min(32)]);

    format!("x\"{}\"", hex::encode(padded))
}

/// Modular exponentiation: base^exp mod modulus
fn mod_pow(base: &BigUint, exp: &BigUint, modulus: &BigUint) -> BigUint {
    if exp.is_zero() {
        return BigUint::one();
    }
    if exp.is_one() {
        return base % modulus;
    }

    let mut result = BigUint::one();
    let mut base = base % modulus;
    let mut exp = exp.clone();

    while !exp.is_zero() {
        if &exp % 2u32 == BigUint::one() {
            result = (&result * &base) % modulus;
        }
        base = (&base * &base) % modulus;
        exp >>= 1;
    }

    result
}

/// Generate TABLE0: R^i for i = 0..1023
fn generate_table0(r: &BigUint, modulus: &BigUint) -> Vec<String> {
    println!("Generating TABLE0: R^i for i = 0..1023");
    let mut table = Vec::with_capacity(1024);
    let mut power = BigUint::one(); // Start with R^0 = 1

    for i in 0..1024 {
        table.push(to_move_hex(&power));
        if i < 1023 {
            power = (&power * r) % modulus; // Multiply by R for next power
        }

        if i % 100 == 0 {
            println!("  Generated {} entries", i + 1);
        }
    }

    table
}

/// Generate TABLE1: R^(1024*i) for i = 0..1023  
fn generate_table1(r: &BigUint, modulus: &BigUint) -> Vec<String> {
    println!("Generating TABLE1: R^(1024*i) for i = 0..1023");
    let mut table = Vec::with_capacity(1024);

    // First compute R^1024
    let r_1024 = mod_pow(r, &BigUint::from(1024u32), modulus);
    let mut power = BigUint::one(); // Start with R^0 = 1

    for i in 0..1024 {
        table.push(to_move_hex(&power));
        if i < 1023 {
            power = (&power * &r_1024) % modulus; // Multiply by R^1024 for next power
        }

        if i % 100 == 0 {
            println!("  Generated {} entries", i + 1);
        }
    }

    table
}

/// Generate TABLE2: R^(1024^2*i) for i = 0..1023
fn generate_table2(r: &BigUint, modulus: &BigUint) -> Vec<String> {
    println!("Generating TABLE2: R^(1024^2*i) for i = 0..1023");
    let mut table = Vec::with_capacity(1024);

    // First compute R^(1024^2) = R^1048576
    let r_1024_squared = mod_pow(r, &BigUint::from(1024u32 * 1024u32), modulus);
    let mut power = BigUint::one(); // Start with R^0 = 1

    for i in 0..1024 {
        table.push(to_move_hex(&power));
        if i < 1023 {
            power = (&power * &r_1024_squared) % modulus; // Multiply by R^(1024^2) for next power
        }

        if i % 100 == 0 {
            println!("  Generated {} entries", i + 1);
        }
    }

    table
}

/// Format table for Move constant declaration
fn format_table_for_move(table_name: &str, table: &[String]) -> String {
    let mut result = format!(
        "/// {}: Lookup table for efficient scalar exponentiation\n",
        match table_name {
            "TABLE0_BYTES" => "R^i for i = 0..1023 (base powers)",
            "TABLE1_BYTES" => "R^(1024*i) for i = 0..1023 (powers of R^1024)",
            "TABLE2_BYTES" => "R^(1024^2*i) for i = 0..1023 (powers of R^(1024^2))",
            _ => "Lookup table",
        }
    );
    result.push_str(&format!(
        "const {}: vector<vector<u8>> = vector[\n",
        table_name
    ));

    for (i, hex_str) in table.iter().enumerate() {
        if i == 0 {
            result.push_str(&format!("    {}, // R^0 = 1\n", hex_str));
        } else if i == 1 && table_name.contains("TABLE0") {
            result.push_str(&format!("    {}, // R^1 = R\n", hex_str));
        } else {
            let comment = match table_name {
                s if s.contains("TABLE0") => format!("R^{}", i),
                s if s.contains("TABLE1") => format!("R^{}", i * 1024),
                s if s.contains("TABLE2") => format!("R^{}", i * 1024 * 1024),
                _ => format!("entry {}", i),
            };
            result.push_str(&format!("    {}, // {}\n", hex_str, comment));
        }
    }

    result.push_str("];\n\n");
    result
}

/// Generate the complete constants.move file
fn generate_constants_move_file(table0: &[String], table1: &[String], table2: &[String]) -> String {
    let mut content = String::new();

    // File header
    content.push_str(&format!(
        r#"/// Auto-generated lookup tables for optimized scalar exponentiation
/// Generated using R = 0x{}
/// BLS12-381 scalar field modulus: {}
/// 
/// This module provides 3 lookup tables for O(1) exponentiation:
/// - TABLE0_BYTES: R^i for i = 0..1023 (base powers)
/// - TABLE1_BYTES: R^(1024*i) for i = 0..1023 (powers of R^1024)
/// - TABLE2_BYTES: R^(1024^2*i) for i = 0..1023 (powers of R^(1024^2))
///
/// Usage: exp = i0 + 1024*i1 + 1024^2*i2
///        R^exp = TABLE2[i2] * TABLE1[i1] * TABLE0[i0]
///
/// Maximum supported exponent: 1024^3 - 1 = 1,073,741,823
/// Total storage: 96 KiB (3 * 1024 * 32 bytes)

module commitment::constants;

use sui::bls12381::{{Scalar, scalar_from_bytes}};
use sui::group_ops::Element;

/// The R constant used for exponentiation
const R_BYTES: vector<u8> = x"{}";

/// Get the R constant as Element<Scalar>
public fun get_r(): Element<Scalar> {{
    let r_bytes = R_BYTES;
    scalar_from_bytes(&r_bytes)
}}

"#,
        R_HEX, BLS12_381_SCALAR_MODULUS, R_HEX
    ));

    // Add getter functions
    content.push_str(&generate_getter_functions());

    // Add all three tables
    content.push_str(&format_table_for_move("TABLE0_BYTES", table0));
    content.push_str(&format_table_for_move("TABLE1_BYTES", table1));
    content.push_str(&format_table_for_move("TABLE2_BYTES", table2));

    content
}

/// Generate getter functions that return Element<Scalar> directly
fn generate_getter_functions() -> String {
    r#"/// Get entry from TABLE0 (R^i for i = 0..1023)
public fun get_table0_entry(index: u32): Element<Scalar> {
    assert!(index < 1024, 0);
    let table = TABLE0_BYTES;
    let bytes = vector::borrow(&table, index as u64);
    scalar_from_bytes(bytes)
}

/// Get entry from TABLE1 (R^(1024*i) for i = 0..1023)
public fun get_table1_entry(index: u32): Element<Scalar> {
    assert!(index < 1024, 1);
    let table = TABLE1_BYTES;
    let bytes = vector::borrow(&table, index as u64);
    scalar_from_bytes(bytes)
}

/// Get entry from TABLE2 (R^(1024^2*i) for i = 0..1023)
public fun get_table2_entry(index: u32): Element<Scalar> {
    assert!(index < 1024, 2);
    let table = TABLE2_BYTES;
    let bytes = vector::borrow(&table, index as u64);
    scalar_from_bytes(bytes)
}
"#
    .to_string()
}

/// Format table for TypeScript constant declaration
fn format_table_for_typescript(table_name: &str, table: &[String]) -> String {
    let mut result = format!(
        "// {}: Lookup table for efficient scalar exponentiation\n",
        match table_name {
            "TABLE0" => "R^i for i = 0..1023 (base powers)",
            "TABLE1" => "R^(1024*i) for i = 0..1023 (powers of R^1024)",
            "TABLE2" => "R^(1024^2*i) for i = 0..1023 (powers of R^(1024^2))",
            _ => "Lookup table",
        }
    );
    result.push_str(&format!("const {}: readonly bigint[] = [\n", table_name));

    for (i, hex_str) in table.iter().enumerate() {
        // Remove the 'x"' prefix and '"' suffix from Move hex format
        let hex_without_quotes = hex_str.trim_start_matches("x\"").trim_end_matches("\"");

        if i == 0 {
            result.push_str(&format!("  0x{}n, // R^0 = 1\n", hex_without_quotes));
        } else if i == 1 && table_name.contains("TABLE0") {
            result.push_str(&format!("  0x{}n, // R^1 = R\n", hex_without_quotes));
        } else {
            let comment = match table_name {
                s if s.contains("TABLE0") => format!("R^{}", i),
                s if s.contains("TABLE1") => format!("R^{}", i * 1024),
                s if s.contains("TABLE2") => format!("R^{}", i * 1024 * 1024),
                _ => format!("entry {}", i),
            };
            result.push_str(&format!("  0x{}n, // {}\n", hex_without_quotes, comment));
        }
    }

    result.push_str("] as const;\n\n");
    result
}

/// Generate the complete constants.ts file
fn generate_constants_typescript_file(
    table0: &[String],
    table1: &[String],
    table2: &[String],
) -> String {
    let mut content = String::new();

    // File header
    let r_hex_without_quotes = R_HEX;
    content.push_str(&format!(
        r#"/// Auto-generated lookup tables for optimized scalar exponentiation
/// Generated using R = 0x{}
/// BLS12-381 scalar field modulus: {}
/// 
/// This module provides 3 lookup tables for O(1) exponentiation:
/// - TABLE0: R^i for i = 0..1023 (base powers)
/// - TABLE1: R^(1024*i) for i = 0..1023 (powers of R^1024)
/// - TABLE2: R^(1024^2*i) for i = 0..1023 (powers of R^(1024^2))
///
/// Usage: exp = i0 + 1024*i1 + 1024^2*i2
///        R^exp = TABLE2[i2] * TABLE1[i1] * TABLE0[i0]
///
/// Maximum supported exponent: 1024^3 - 1 = 1,073,741,823
/// Total storage: 96 KiB (3 * 1024 * 32 bytes)

import {{ createForeignField }} from "o1js";

export {{ Fr, BLS_FR, TABLE0, TABLE1, TABLE2 }};

// BLS12‑381 scalar field prime
const BLS_FR = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001n;
const Fr = createForeignField(BLS_FR);

// Use the proper types from the foreign field system
type CanonicalElement = InstanceType<typeof Fr.Canonical>;

/// The R constant used for exponentiation (raw bigint value)
const R_VALUE: bigint = 0x{}n;

/// Get the R constant as CanonicalElement
export function getR(): CanonicalElement {{
  return Fr.from(R_VALUE);
}}

"#,
        r_hex_without_quotes, BLS12_381_SCALAR_MODULUS, r_hex_without_quotes
    ));

    // Add getter functions
    content.push_str(&generate_typescript_getter_functions());

    // Add all three tables
    content.push_str(&format_table_for_typescript("TABLE0", table0));
    content.push_str(&format_table_for_typescript("TABLE1", table1));
    content.push_str(&format_table_for_typescript("TABLE2", table2));

    content
}

/// Generate TypeScript getter functions
fn generate_typescript_getter_functions() -> String {
    r#"/// Get entry from TABLE0 (R^i for i = 0..1023)
export function getTable0Entry(index: number): CanonicalElement {
  if (index >= 1024) throw new Error("TABLE0 index out of bounds");
  return Fr.from(TABLE0[index]);
}

/// Get entry from TABLE1 (R^(1024*i) for i = 0..1023)
export function getTable1Entry(index: number): CanonicalElement {
  if (index >= 1024) throw new Error("TABLE1 index out of bounds");
  return Fr.from(TABLE1[index]);
}

/// Get entry from TABLE2 (R^(1024^2*i) for i = 0..1023)
export function getTable2Entry(index: number): CanonicalElement {
  if (index >= 1024) throw new Error("TABLE2 index out of bounds");
  return Fr.from(TABLE2[index]);
}
"#
    .to_string()
}

pub fn main() {
    println!("Generating BLS12-381 scalar exponentiation lookup tables for R");
    println!("R = {}", R_HEX);
    println!();

    // Parse constants
    let modulus = BigUint::from_str(BLS12_381_SCALAR_MODULUS)
        .expect("Failed to parse BLS12-381 scalar modulus");
    let r = BigUint::parse_bytes(R_HEX.as_bytes(), 16).expect("Failed to parse R constant");

    println!("Parsed modulus: {}", modulus);
    println!("Parsed R: {}", r);
    println!();

    // Generate all three tables
    let table0 = generate_table0(&r, &modulus);
    println!("TABLE0 generated with {} entries\n", table0.len());

    let table1 = generate_table1(&r, &modulus);
    println!("TABLE1 generated with {} entries\n", table1.len());

    let table2 = generate_table2(&r, &modulus);
    println!("TABLE2 generated with {} entries\n", table2.len());

    // Generate the complete Move file
    let move_file_content = generate_constants_move_file(&table0, &table1, &table2);

    // Write to constants.move file
    let constants_path = Path::new("../sources/constants.move");
    match fs::write(constants_path, &move_file_content) {
        Ok(_) => {
            println!("✓ Successfully generated ../sources/constants.move");
            println!("  File size: {} bytes", move_file_content.len());
        }
        Err(e) => {
            eprintln!("✗ Failed to write constants.move: {}", e);
            println!("\n{}", "=".repeat(80));
            println!("MOVE FILE CONTENT (copy manually to ../sources/constants.move):");
            println!("{}", "=".repeat(80));
            println!("{}", move_file_content);
        }
    }

    // Generate the complete TypeScript file
    let typescript_file_content = generate_constants_typescript_file(&table0, &table1, &table2);

    // Write to constants.ts file (exp.ts is manually maintained)
    let constants_ts_path = Path::new("../mina/src/constants.ts");

    match fs::write(constants_ts_path, &typescript_file_content) {
        Ok(_) => {
            println!("✓ Successfully generated ../mina/src/constants.ts");
            println!("  File size: {} bytes", typescript_file_content.len());
        }
        Err(e) => {
            eprintln!("✗ Failed to write constants.ts: {}", e);
            println!("\n{}", "=".repeat(80));
            println!("TYPESCRIPT FILE CONTENT (copy manually to ../mina/src/constants.ts):");
            println!("{}", "=".repeat(80));
            println!("{}", typescript_file_content);
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("Generation complete!");
    println!("Generated: constants.ts (exp.ts is manually maintained)");
    println!(
        "Total storage: {} bytes (3 * 1024 * 32 bytes)",
        3 * 1024 * 32
    );
    println!(
        "Maximum supported exponent: {} (2^30 - 1)",
        1024u64.pow(3) - 1
    );
    println!("{}", "=".repeat(80));
}
