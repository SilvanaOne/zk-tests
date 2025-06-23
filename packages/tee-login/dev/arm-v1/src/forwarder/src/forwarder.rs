use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::FromRawFd;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

pub fn run_server(
    local_ip: &str,
    local_port: u16,
    remote_cid: u32,
    remote_port: u32,
) -> io::Result<()> {
    let listener = TcpListener::bind((local_ip, local_port))?;
    listener.set_nonblocking(true)?;
    info!("Listening on {}:{}", local_ip, local_port);

    for stream in listener.incoming() {
        match stream {
            Ok(client) => {
                let remote_cid = remote_cid;
                let remote_port = remote_port;
                debug!("Accepted connection from {:?}", client.peer_addr());
                thread::spawn(move || {
                    if let Err(e) = handle_connection(client, remote_cid, remote_port) {
                        warn!("Connection handler exited with error: {}", e);
                    }
                });
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No pending connection, avoid busy loop.
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => {
                warn!("Failed to accept connection: {}", e);
            }
        }
    }
    Ok(())
}

fn handle_connection(mut client: TcpStream, remote_cid: u32, remote_port: u32) -> io::Result<()> {
    let mut server = match vsock_connect(remote_cid, remote_port) {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "Failed to connect to vsock {}:{} => {}",
                remote_cid, remote_port, e
            );
            return Err(e);
        }
    };

    let mut server_clone = server.try_clone()?;
    let mut client_clone = client.try_clone()?;

    // Client -> Server
    let t1 = thread::spawn(move || copy_stream(&mut client, &mut server_clone));
    // Server -> Client
    let t2 = thread::spawn(move || copy_stream(&mut server, &mut client_clone));

    // Wait for both directions.
    let _ = t1.join();
    let _ = t2.join();
    Ok(())
}

fn copy_stream(src: &mut TcpStream, dst: &mut TcpStream) {
    let mut buf = [0u8; 16 * 1024];
    loop {
        match src.read(&mut buf) {
            Ok(0) => {
                debug!("EOF reached on read; shutting down write side");
                let _ = dst.shutdown(std::net::Shutdown::Write);
                break;
            }
            Ok(n) => {
                if let Err(e) = dst.write_all(&buf[..n]) {
                    warn!("Write error ({} bytes): {}", n, e);
                    break;
                }
            }
            Err(e) => {
                if e.kind() != io::ErrorKind::Interrupted {
                    warn!("Read error: {}", e);
                    break;
                }
            }
        }
    }
}

fn vsock_connect(cid: u32, port: u32) -> io::Result<TcpStream> {
    use libc::sa_family_t;
    use libc::{AF_VSOCK, SOCK_STREAM, c_int, sockaddr, sockaddr_vm};
    unsafe {
        // Create socket
        let fd: c_int = libc::socket(AF_VSOCK, SOCK_STREAM, 0);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // Prepare address
        let mut addr: sockaddr_vm = std::mem::zeroed();
        addr.svm_family = AF_VSOCK as sa_family_t;
        addr.svm_cid = cid;
        addr.svm_port = port;

        let ret = libc::connect(
            fd,
            &addr as *const _ as *const sockaddr,
            std::mem::size_of::<sockaddr_vm>() as u32,
        );
        if ret < 0 {
            let err = io::Error::last_os_error();
            libc::close(fd);
            return Err(err);
        }
        Ok(TcpStream::from_raw_fd(fd))
    }
}
