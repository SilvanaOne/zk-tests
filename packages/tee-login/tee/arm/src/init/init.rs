// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

fn main() {
    println!("=== MINIMAL TEST INIT STARTING ===");
    eprintln!("=== MINIMAL TEST INIT STARTING ===");

    // Try to write to console directly
    use std::fs::OpenOptions;
    use std::io::Write;

    if let Ok(mut console) = OpenOptions::new().write(true).open("/dev/console") {
        let _ = console.write_all(b"=== INIT: Writing to /dev/console ===\n");
        let _ = console.flush();
    }

    // Try to write to stderr
    use std::io::{self, Write as _};
    let _ = writeln!(io::stderr(), "=== INIT: Writing to stderr ===");

    // Simple loop to keep the init process alive and show it's working
    for i in 1..=10 {
        println!("=== INIT: Loop iteration {} ===", i);
        eprintln!("=== INIT: Loop iteration {} ===", i);

        if let Ok(mut console) = OpenOptions::new().write(true).open("/dev/console") {
            let _ = console.write_all(format!("=== INIT: Console write {} ===\n", i).as_bytes());
            let _ = console.flush();
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    println!("=== INIT: Test completed, sleeping forever ===");
    eprintln!("=== INIT: Test completed, sleeping forever ===");

    // Keep the process alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        println!("=== INIT: Still alive ===");
    }
}
