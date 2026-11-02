use std::{
    env,
    fs::{self, File},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use sp1_build::{BuildArgs, build_program_with_args};

/// How long to wait for a guest build left running by a killed build, before going ahead anyway.
const LEFTOVER_BUILD_WAIT: Duration = Duration::from_secs(15 * 60);

fn main() {
    // Guests build inside SP1's Docker image, so no local SP1 toolchain is needed and their ELFs
    // and verifying keys are reproducible.
    let args = BuildArgs {
        docker: true,
        locked: true,
        ..Default::default()
    };
    let _guest_build = (!guest_build_is_skipped()).then(|| {
        let image = env::var("SP1_DOCKER_IMAGE")
            .unwrap_or_else(|_| format!("ghcr.io/succinctlabs/sp1:{}", args.tag));
        let lock = lock_guest_build();
        ensure_image(&image);
        wait_for_leftover_builds(&image);
        lock
    });
    build_program_with_args("program", args);

    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let version = Command::new(rustc)
        .arg("--version")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=HOST_RUSTC_VERSION={version}");
}

/// Makes concurrent builds of the guests take turns, until the returned file is dropped.
///
/// Cargo locks each profile separately, so `cargo run --release` and an editor's `cargo check`
/// through rust-analyzer run this script at the same time. Both write to the same guest target
/// directory, and each can then fail on artifacts the other is replacing.
fn lock_guest_build() -> Option<File> {
    fs::create_dir_all("program/target").ok()?;
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open("program/target/guest-build.lock")
        .ok()?;
    file.lock().ok()?;
    Some(file)
}

/// The cases where sp1-build compiles no guest, so no image is needed.
fn guest_build_is_skipped() -> bool {
    env::var("SP1_SKIP_PROGRAM_BUILD").is_ok_and(|value| value.eq_ignore_ascii_case("true"))
        || env::var("RUSTC_WORKSPACE_WRAPPER").is_ok_and(|value| value.contains("clippy-driver"))
}

/// Waits for guest builds whose build script was killed, by Ctrl-C or by an editor restarting its
/// check. That frees the lock, but their container keeps writing to the guest target directory.
fn wait_for_leftover_builds(image: &str) {
    let filter = format!("ancestor={image}");
    let started = Instant::now();
    while started.elapsed() < LEFTOVER_BUILD_WAIT {
        let running = Command::new("docker")
            .args(["ps", "-q", "--filter", &filter])
            .output()
            .is_ok_and(|output| output.status.success() && !output.stdout.is_empty());
        if !running {
            return;
        }
        thread::sleep(Duration::from_secs(2));
    }
    println!("cargo:warning=A container from {image} is still running, so building anyway");
}

/// Pulls the image if it is missing. The image is public, so this uses no credentials: the pull
/// that `docker run` would do for itself sends any stored `ghcr.io` login, and fails if the registry
/// rejects it. If this pull fails, the build carries on and `docker run` reports the problem.
fn ensure_image(image: &str) {
    if env::var_os("SP1_DOCKER_IMAGE").is_some() {
        return;
    }
    let present = Command::new("docker")
        .args(["image", "inspect", image])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    let Some(docker) = find_docker().filter(|_| !present) else {
        return;
    };

    // Docker takes credentials from DOCKER_AUTH_CONFIG, its config, or a helper it finds on PATH. An
    // empty directory for the last two leaves it none. It also loses the current context, so that
    // is passed on.
    let empty = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("empty");
    fs::create_dir_all(&empty).expect("creating a directory under OUT_DIR");
    let mut pull = Command::new(docker);
    pull.args(["pull", "--platform", "linux/amd64", image])
        .env("DOCKER_CONFIG", &empty)
        .env("PATH", &empty)
        .env_remove("DOCKER_AUTH_CONFIG")
        .env_remove("DOCKER_CONTEXT");
    if let Some(host) = docker_host() {
        pull.env("DOCKER_HOST", host);
    }
    match pull.output() {
        Ok(output) if output.status.success() => {
            println!("cargo:warning=Pulled {image} without credentials");
        }
        Ok(output) => println!(
            "cargo:warning=Could not pull {image} without credentials: {}",
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .replace('\n', " ")
        ),
        Err(error) => println!("cargo:warning=Could not run docker pull: {error}"),
    }
}

fn find_docker() -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join("docker"))
        .find(|path| path.is_file())
}

/// The daemon address of the current Docker context.
fn docker_host() -> Option<String> {
    let output = Command::new("docker")
        .args([
            "context",
            "inspect",
            "--format",
            "{{.Endpoints.docker.Host}}",
        ])
        .output()
        .ok()?;
    let host = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !host.is_empty()).then_some(host)
}
