mod attestation;
mod nitro_attestation;

fn main() {
    let attestation = include_str!("../data/release2.json");
    let result = attestation::verify_attestation(attestation);
    println!("{:?}", result);
}
