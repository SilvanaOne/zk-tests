use alloy_sol_types::sol;

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    struct PublicValuesStruct {
        uint32 old_sum;
        uint32 new_sum;
    }
}

/// Compute the sum of the value and the old_sum (wrapping around on overflows), using normal Rust code.
pub fn add_one(value: u32, old_sum: u32) -> u32 {
    old_sum.wrapping_add(value)
}

/// Compute the sum of the values and the old_sum (wrapping around on overflows), using normal Rust code.
pub fn add_many(values: &[u32], old_sum: u32) -> u32 {
    values
        .iter()
        .fold(old_sum, |sum, &value| sum.wrapping_add(value))
}
