use nix::{Result, ioctl_read};
use std::{fs::File, os::unix::io::AsRawFd};

/// Mirrors `struct ptp_clock_time` from `<linux/ptp_clock.h>`
#[repr(C)]
#[derive(Debug)]
struct PtpClockTime {
    sec: i64,  // seconds since the UNIX epoch
    nsec: u32, // nanoseconds
    _reserved: i32,
}

// Define the PTP_CLOCK_GETTIME ioctl: _IOR('T', 1, struct ptp_clock_time)
ioctl_read!(ptp_clk_gettime, b'T', 1, PtpClockTime);

fn main() -> Result<()> {
    // 1. Open the PTP device
    let file = File::open("/dev/ptp0").expect("failed to open /dev/ptp0");
    let fd = file.as_raw_fd();

    // 2. Prepare the struct to hold the timestamp
    let mut ts = PtpClockTime {
        sec: 0,
        nsec: 0,
        _reserved: 0,
    };

    // 3. Issue the ioctl to get the PTP time
    unsafe {
        ptp_clk_gettime(fd, &mut ts)?;
    }

    // 4. Print it out
    println!("PTP clock reports: {}.{:09} UTC", ts.sec, ts.nsec);
    Ok(())
}
