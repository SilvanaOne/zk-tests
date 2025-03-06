module mina::field;

const P: u256 = 28948022309329048855892746252171976963363056481941560715954676764349967630337;
// public struct Field has copy, drop {
//     value: u256,
// }

// // type Scalar = bigint;
// public struct Scalar has copy, drop {
//     value: u256,
// }

// public fun new_field(val: u256): Field {
//     Field { value: val }
// }

// public fun get_field_value(field: &Field): u256 {
//     field.value
// }

// public use fun get_field_value as Field.get_value;

// function mod(x: bigint, p: bigint) {
//   x = x % p;
//   if (x < 0) return x + p;
//   return x;
// }

public fun mod(x: u256, p: u256): u256 {
    x % p
}

/*
function isEven(x: bigint) {
  return !(mod(x, p) & 1n);
}
*/

public fun is_even(x: u256): bool {
    (mod(x, P) & 1) == 0
}

/*
function equal(x: bigint, y: bigint) {
  // We check if x and y are both in the range [0, p). If they are, can do a simple comparison. Otherwise, we need to reduce them to the proper canonical field range.
  let x_ = x >= 0n && x < p ? x : mod(x, p);
  let y_ = y >= 0n && y < p ? y : mod(y, p);
  return x_ === y_;
}
*/
public fun equal(x: u256, y: u256): bool {
    // We check if x and y are both in the range [0, p). If they are, can do a simple comparison.
    // Otherwise, we need to reduce them to the proper canonical field range.
    let x_ = if (x < P) { x } else { mod(x, P) };
    let y_ = if (y < P) { y } else { mod(y, P) };
    x_ == y_
}

/*
// modular exponentiation, a^n % p
function power(a: bigint, n: bigint) {
  a = mod(a, p);
  let x = 1n;
  for (; n > 0n; n >>= 1n) {
    if (n & 1n) x = mod(x * a, p);
    a = mod(a * a, p);
  }
  return x;
}
*/
public fun power(a: u256, n: u256): u256 {
    let a = mod(a, P);
    let mut x = 1;
    let mut n = n;
    let mut a = a;
    while (n > 0) {
        if ((n & 1) == 1) {
            x = mod(x * a, P);
        };
        a = mod(a * a, P);
        n = n >> 1;
    };
    x
}

/*
function add(x: bigint, y: bigint) {
  return mod(x + y, p);
}
*/

const U256_MAX: u256 = 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff;

public fun add(x: u256, y: u256): u256 {
    // First reduce inputs to be within field range to minimize chance of overflow
    let x = if (x >= P) { mod(x, P) } else { x };
    let y = if (y >= P) { mod(y, P) } else { y };

    mod(x + y, P)
}

/*
    #[inline]
    fn mul(&self, other: &Self) -> (Self, Self) {
        if self.is_zero() || other.is_zero() {
            let zero = Self::zero();
            return (zero, zero);
        }

        let mut r = crate::const_helpers::MulBuffer::<N>::zeroed();

        let mut carry = 0;

        for i in 0..N {
            for j in 0..N {
                r[i + j] = mac_with_carry!(r[i + j], self.0[i], other.0[j], &mut carry);
            }
            r.b1[i] = carry;
            carry = 0;
        }

        return (Self(r.b0), Self(r.b1));
    }

    /// Calculate a + (b * c) + carry, returning the least significant digit
/// and setting carry to the most significant digit.
#[inline(always)]
#[doc(hidden)]
pub fn mac_with_carry(a: u64, b: u64, c: u64, carry: &mut u64) -> u64 {
    let tmp = (a as u128) + widening_mul(b, c) + (*carry as u128);
    *carry = (tmp >> 64) as u64;
    tmp as u64
}

#[inline(always)]
#[doc(hidden)]
pub const fn widening_mul(a: u64, b: u64) -> u128 {
    #[cfg(not(target_family = "wasm"))]
    {
        a as u128 * b as u128
    }
    #[cfg(target_family = "wasm")]
    {
        let a_lo = a as u32 as u64;
        let a_hi = a >> 32;
        let b_lo = b as u32 as u64;
        let b_hi = b >> 32;

        let lolo = (a_lo * b_lo) as u128;
        let lohi = ((a_lo * b_hi) as u128) << 32;
        let hilo = ((a_hi * b_lo) as u128) << 32;
        let hihi = ((a_hi * b_hi) as u128) << 64;
        (lolo | hihi) + (lohi + hilo)
    }
}

macro_rules! const_modulo {
    ($a:expr, $divisor:expr) => {{
        // Stupid slow base-2 long division taken from
        // https://en.wikipedia.org/wiki/Division_algorithm
        assert!(!$divisor.const_is_zero());
        let mut remainder = Self::new([0u64; N]);
        let mut i = ($a.num_bits() - 1) as isize;
        let mut carry;
        while i >= 0 {
            (remainder, carry) = remainder.const_mul2_with_carry();
            remainder.0[0] |= $a.get_bit(i as usize) as u64;
            if remainder.const_geq($divisor) || carry {
                let (r, borrow) = remainder.const_sub_with_borrow($divisor);
                remainder = r;
                assert!(borrow == carry);
            }
            i -= 1;
        }
        remainder
    }};
}

*/

public fun mac_with_carry(a: u128, b: u128, c: u128, carry: &mut u128): u128 {
    let tmp = (a as u256) + (b as u256) * (c as u256) + (*carry as u256);
    *carry = ((tmp >> 64) & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF) as u128;
    tmp as u128
}

public fun mul_u256_v2(x: u256, y: u256): (u256, u256) {
    if (x == 0 || y == 0) {
        (0, 0)
    } else {
        // Break x and y into low and high 128-bit parts
        let x_lo = (x & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF) as u128;
        let x_hi = (x >> 128) as u128;
        let y_lo = (y & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF) as u128;
        let y_hi = (y >> 128) as u128;

        // for i in 0..N {
        //     for j in 0..N {
        //         r[i + j] = mac_with_carry!(r[i + j], self.0[i], other.0[j], &mut carry);
        //     }
        //     r.b1[i] = carry;
        //     carry = 0;
        // }

        let mut r = vector[0u128, 0u128, 0u128, 0u128];
        let mut carry: u128 = 0;

        let val = *vector::borrow(&r, 0);
        let result = mac_with_carry(val, x_lo, y_lo, &mut carry);
        vector::push_back(&mut r, result);

        let val = *vector::borrow(&r, 1);
        let result = mac_with_carry(val, x_lo, y_hi, &mut carry);
        vector::push_back(&mut r, result);

        let val = *vector::borrow(&r, 0);
        vector::push_back(&mut r, carry);
        carry = 0;

        let val = *vector::borrow(&r, 2);
        let result = mac_with_carry(val, x_hi, y_lo, &mut carry);
        vector::push_back(&mut r, result);

        let val = *vector::borrow(&r, 3);
        let result = mac_with_carry(val, x_hi, y_hi, &mut carry);
        vector::push_back(&mut r, result);

        *vector::borrow(&r, 1);
        vector::push_back(&mut r, carry);
        carry = 0;

        // for i in 0..2 {
        //     for j in 0..2 {
        //         r[i + j] = mac_with_carry(r[i + j], 0, 0, &mut carry);
        //     }
        //     r[i] = carry;
        //     carry = 0;
        // }

        (r[0] as u256 + (r[1] as u256) << 128, r[2] as u256 + (r[3] as u256) << 128)
    }
}

/// Multiplies two `u256` numbers and returns `(low, high)` as `u256` values
public fun mul_u256(x: u256, y: u256): (u256, u256) {
    if (x == 0 || y == 0) {
        (0, 0)
    } else {
        // Break x and y into low and high 128-bit parts
        let x_lo = x & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;
        let x_hi = x >> 128;
        let y_lo = y & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;
        let y_hi = y >> 128;

        // Compute partial products
        let lo_lo = x_lo * y_lo; // Low part
        let lo_hi = x_lo * y_hi; // Cross terms
        let hi_lo = x_hi * y_lo; // Cross terms
        let hi_hi = x_hi * y_hi; // High part

        // Combine results
        let mid1 = lo_hi + hi_lo;
        let mid1_carry = mid1 >> 128; // Carry from mid multiplication

        let low_result = lo_lo & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;
        let high_result = hi_hi + mid1_carry;

        (low_result, high_result)
    }
}

/*
function mul(x: bigint, y: bigint) {
  return mod(x * y, p);
}
*/

/*
type Group = { x: Field; y: Field };
type PublicKey = { x: Field; isOdd: Bool };
*/
// A non-zero point on the Pallas curve in affine form { x, y }
// public struct Group has copy, drop {
//     x: Field,
//     y: Field,
// }

// public struct PublicKey has copy, drop {
//     x: Field,
//     is_odd: bool,
// }

/*

type Signature = { r: Field; s: Scalar };
const projectiveZero = { x: 1n, y: 1n, z: 0n };

type GroupProjective = { x: bigint; y: bigint; z: bigint };
type PointAtInfinity = { x: bigint; y: bigint; infinity: true };
type FinitePoint = { x: bigint; y: bigint; infinity: false };
type GroupAffine = PointAtInfinity | FinitePoint;
*/

/// A point in projective coordinates
public struct GroupProjective has copy, drop {
    x: u256,
    y: u256,
    z: u256,
}

/// A point at infinity
public struct Point has copy, drop {
    x: u256,
    y: u256,
    infinity: bool,
}

/*
const PallasConstants = {
  name: "Pallas",
  modulus:
    28948022309329048855892746252171976963363056481941560715954676764349967630337n,
  order:
    28948022309329048855892746252171976963363056481941647379679742748393362948097n,
  cofactor: 1n,
  zero: { x: 1n, y: 1n, z: 0n },
  one: {
    x: 1n,
    y: 12418654782883325593414442427049395787963493412651469444558597405572177144507n,
    z: 1n,
  },
  hasEndomorphism: true,
  a: 0n,
  b: 5n,
  hasCofactor: false,
  p: 28948022309329048855892746252171976963363056481941560715954676764349967630337n,
  twoadicRoot:
    19814229590243028906643993866117402072516588566294623396325693409366934201135n,
  twoadicity: 32n,
  oddFactor:
    6739986666787659948666753771754907668419893943225396963757154709741n,
};
*/
/// Constants for the Pallas curve
// const MODULUS: u256 = 28948022309329048855892746252171976963363056481941560715954676764349967630337;
// const ORDER: u256 = 28948022309329048855892746252171976963363056481941647379679742748393362948097;
// const COFACTOR: u8 = 1;
// const A: u8 = 0;
// const B: u8 = 5;
// const TWOADIC_ROOT: u256 =
//     19814229590243028906643993866117402072516588566294623396325693409366934201135;
// const TWOADICITY: u8 = 32;
// const ODD_FACTOR: u256 = 6739986666787659948666753771754907668419893943225396963757154709741;

// /// Base point coordinates
// const BASE_X: u256 = 1;
// const BASE_Y: u256 = 12418654782883325593414442427049395787963493412651469444558597405572177144507;
// const BASE_Z: u8 = 1;

// /// Zero point coordinates
// const ZERO_X: u8 = 1;
// const ZERO_Y: u8 = 1;
// const ZERO_Z: u8 = 0;

// /// Curve properties
// const HAS_ENDOMORPHISM: bool = true;
// const HAS_COFACTOR: bool = false;
