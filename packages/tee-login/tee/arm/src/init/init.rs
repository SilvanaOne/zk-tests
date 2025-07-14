// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use aws::{get_entropy, init_platform};
use std::env;
use std::process::Command;
use system::{dmesg, freopen, mount, reboot, seed_entropy};

// Referenced from: https://git.distrust.co/public/enclaveos/src/branch/master/src/init/init.rs
// Mount common filesystems with conservative permissions
fn init_rootfs() {
    println!("=== ROOTFS: Starting filesystem mounts ===");
    use libc::{MS_NODEV, MS_NOEXEC, MS_NOSUID};
    let no_dse = MS_NODEV | MS_NOSUID | MS_NOEXEC;
    let no_se = MS_NOSUID | MS_NOEXEC;
    let args = [
        ("devtmpfs", "/dev", "devtmpfs", no_se, "mode=0755"),
        ("devpts", "/dev/pts", "devpts", no_se, ""),
        ("shm", "/dev/shm", "tmpfs", no_dse, "mode=0755"),
        ("proc", "/proc", "proc", no_dse, "hidepid=2"),
        ("tmpfs", "/run", "tmpfs", no_dse, "mode=0755"),
        ("tmpfs", "/tmp", "tmpfs", no_dse, ""),
        ("sysfs", "/sys", "sysfs", no_dse, ""),
        (
            "cgroup_root",
            "/sys/fs/cgroup",
            "tmpfs",
            no_dse,
            "mode=0755",
        ),
    ];
    for (src, target, fstype, flags, data) in args {
        println!("=== ROOTFS: Processing mount {} -> {} ===", src, target);

        if std::fs::exists(target).unwrap_or(false) {
            println!("=== ROOTFS: Mount point {} already exists ===", target);
        } else {
            match std::fs::create_dir_all(target) {
                Ok(()) => {
                    dmesg(format!("Created mount point {}", target));
                    println!("=== ROOTFS: Created mount point {} ===", target);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    println!(
                        "=== ROOTFS: Failed to create mount point {}: {} ===",
                        target, e
                    );
                }
            }
        }

        match mount(src, target, fstype, flags, data) {
            Ok(()) => {
                dmesg(format!("Mounted {}", target));
                println!("=== ROOTFS: Successfully mounted {} ===", target);
            }
            Err(e) => {
                eprintln!("{}", e);
                println!("=== ROOTFS: Failed to mount {}: {} ===", target, e);
            }
        }
    }
    println!("=== ROOTFS: All filesystem mounts completed ===");
}

// Initialize console with stdin/stdout/stderr
fn init_console() {
    let args = [
        ("/dev/console", "r", 0),
        ("/dev/console", "w", 1),
        ("/dev/console", "w", 2),
    ];
    for (filename, mode, file) in args {
        match freopen(filename, mode, file) {
            Ok(()) => {}
            Err(e) => eprintln!("{}", e),
        }
    }
}

fn boot() {
    println!("=== BOOT: Starting rootfs init ===");
    init_rootfs();
    println!("=== BOOT: Rootfs init completed ===");

    println!("=== BOOT: Starting console init ===");
    init_console();
    println!("=== BOOT: Console init completed ===");

    println!("=== BOOT: Starting platform init ===");
    init_platform();
    println!("=== BOOT: Platform init completed ===");

    println!("=== BOOT: Starting entropy seeding ===");
    match seed_entropy(4096, get_entropy) {
        Ok(size) => {
            dmesg(format!("Seeded kernel with entropy: {}", size));
            println!("=== BOOT: Entropy seeding completed ({} bytes) ===", size);
        }
        Err(e) => {
            eprintln!("{}", e);
            println!("=== BOOT: Entropy seeding failed: {} ===", e);
        }
    };
    println!("=== BOOT: All boot steps completed ===");
}

fn main() {
    println!("=== INIT PROCESS STARTING ===");

    println!("=== STARTING BOOT SEQUENCE ===");
    boot();

    println!("=== BOOT SEQUENCE COMPLETED ===");
    dmesg("EnclaveOS Booted".to_string());

    println!("=== SETTING ENVIRONMENT VARIABLES ===");
    // Set the SSL_CERT_FILE environment variable
    env::set_var("SSL_CERT_FILE", "/ca-certificates.crt");
    env::set_var("PATH", "/bin:/sbin:/usr/bin:/usr/sbin:/");
    println!("SSL_CERT_FILE set to ca-certificates.crt");

    println!("=== CHECKING FILE SYSTEM ===");
    println!("Current working directory: {:?}", env::current_dir());

    // Check if critical files exist
    let files_to_check = ["/sh", "/run.sh", "/nsm.ko", "/ca-certificates.crt"];
    for file in &files_to_check {
        match std::fs::metadata(file) {
            Ok(metadata) => println!("File {} exists (size: {} bytes)", file, metadata.len()),
            Err(e) => println!("File {} NOT found: {}", file, e),
        }
    }

    println!("=== LISTING ROOT DIRECTORY ===");
    match std::fs::read_dir("/") {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => println!("  {}", entry.path().display()),
                    Err(e) => println!("  Error reading entry: {}", e),
                }
            }
        }
        Err(e) => println!("Error reading root directory: {}", e),
    }

    println!("=== ATTEMPTING TO SPAWN RUN.SH ===");
    match Command::new("/sh").arg("/run.sh").spawn() {
        Ok(mut child) => {
            dmesg("Spawned run.sh script".to_string());
            println!("=== RUN.SH SPAWNED SUCCESSFULLY ===");
            // Wait for the child process to finish
            match child.wait() {
                Ok(status) => {
                    dmesg(format!("run.sh exited with status: {}", status));
                    println!("=== RUN.SH COMPLETED WITH STATUS: {} ===", status);
                }
                Err(e) => {
                    eprintln!("Error waiting for run.sh: {}", e);
                    println!("=== ERROR WAITING FOR RUN.SH: {} ===", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to execute run.sh: {}", e);
            println!("=== FAILED TO EXECUTE RUN.SH: {} ===", e);
        }
    }

    println!("=== INIT PROCESS REBOOTING ===");
    reboot();
}
