mod attestation;

fn main() {
    let attestation = include_str!("../release.json");
    let result = attestation::verify_attestation(attestation);
    println!("{}", result);
}
