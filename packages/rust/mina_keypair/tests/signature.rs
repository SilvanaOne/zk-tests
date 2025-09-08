#[test]
fn verify_signature() {
    // Given values
    let public_key = "B62qoTW7V4AgqAKPnRWrJckhCnrGjDvmS3E9RzdQ6RQq7LRnwRebCb6";
    let signature_b58 = "7mX2R4nTMBrPMxzkDx5wS5jDzXrwzbcKJN748tfPTBYhDXssMBAKgqzCeqXve6oKujb4iZtgREso4A1yxKLMAhzDouRQ9NFr";

    use mina_signer::BaseField;
    let fields = vec![
        BaseField::from(1u64),
        BaseField::from(2u64),
        BaseField::from(3u64),
    ];
    let ok2 = mina_keypair::signature::verify_signature(signature_b58, public_key, &fields);
    println!("verified_with_fields={}", ok2);
    assert!(ok2);
}
