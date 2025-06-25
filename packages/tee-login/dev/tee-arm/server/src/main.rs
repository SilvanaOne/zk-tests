fn main() {
    println!("Hello, world!");
    let hex = hex::encode(b"Hello, world!");
    println!("Hex: {}", hex);
    std::thread::sleep(std::time::Duration::from_secs(10));
    println!("Exiting...");
}
