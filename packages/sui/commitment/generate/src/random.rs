use bls12_381::Scalar;
use ff::Field;
use rand::rngs::OsRng;

fn main() {
    let s = Scalar::random(&mut OsRng); // uniform in [0,r-1]
    let r = Scalar::random(&mut OsRng);

    // reject low-entropy or forbidden values
    assert!(!bool::from(s.is_zero()) && s != Scalar::one());
    assert!(!bool::from(r.is_zero()) && r != Scalar::one() && r != s);

    // Scalar::to_bytes() already yields canonical **big‑endian** bytes.
    let s_be = s.to_bytes();
    let r_be = r.to_bytes();

    // Quick canonical check: first byte <= 0x73 (MSB of modulus r)
    assert!(s_be[0] <= 0x73, "S not < r");
    assert!(r_be[0] <= 0x73, "R not < r");

    println!("// Paste into Move (big‑endian canonical vectors)");
    println!(
        "const S_BYTES: vector<u8> = vector[{}];",
        s_be.iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "const R_BYTES: vector<u8> = vector[{}];",
        r_be.iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Extra hex debug
    println!("S (BE hex): 0x{}", hex::encode(&s_be));
    println!("R (BE hex): 0x{}", hex::encode(&r_be));
}
