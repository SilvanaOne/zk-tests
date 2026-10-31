// End-to-end runs of the mclean binary in scratch trees. PATH names only a directory of fake docker,
// tmutil and df, so the real ones cannot run. Fixtures are built at run time: a real mclean run from
// above this repo must find nothing here to plan.
#![cfg(target_os = "macos")]

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

const INSPECT: &str = "docker context inspect --format {{.Name}} {{.Endpoints.docker.Host}}";
const SYSTEM_DF: &str = "docker --context desktop-linux system df";
const SYSTEM_PRUNE: &str = "docker --context desktop-linux system prune -a -f";
const BUILDER_PRUNE: &str =
    "docker --context desktop-linux builder prune -a -f --builder desktop-linux";
const LIST: &str = "tmutil listlocalsnapshots /";
const THIN: &str = "tmutil thinlocalsnapshots / 99999999999 4";
const DF: &str = "df -kP .";
const PROMPT: &[u8] = b"Proceed? [y/N] ";

// Each fake logs its arguments first; these bodies use shell builtins only, because PATH holds
// nothing else. FAKE_* variables choose how they fail.
const FAKE_DOCKER: &str = r#"
case "$*" in
  "context inspect --format {{.Name}} {{.Endpoints.docker.Host}}")
    echo "${FAKE_CONTEXT:-desktop-linux unix:///var/run/docker.sock}" ;;
  "--context desktop-linux system df")
    [ -n "$FAKE_SYSTEM_DF_FAILS" ] && { echo "daemon down" >&2; exit 1; }
    printf 'TYPE SIZE\nImages 1.2GB\n' ;;
  "--context desktop-linux system prune -a -f")
    [ -n "$FAKE_PRUNE_FAILS" ] && { echo "prune failed" >&2; exit 1; }
    printf 'Deleted Images:\nuntagged: x\n\nTotal reclaimed space: 1.2GB\n' ;;
  "--context desktop-linux builder prune -a -f --builder desktop-linux")
    printf 'ID\tRECLAIMABLE\tSIZE\nTotal:\t0B\n' ;;
  *) echo "unexpected docker arguments: $*" >&2; exit 9 ;;
esac
"#;
const FAKE_TMUTIL: &str = r#"
case "$*" in
  "listlocalsnapshots /")
    [ -n "$FAKE_TMUTIL_FAILS" ] && { echo "tmutil failed" >&2; exit 1; }
    printf 'Snapshots for disk /:\ncom.apple.TimeMachine.2026-10-31-150000.local\n' ;;
  "thinlocalsnapshots / 99999999999 4")
    printf 'Thinned local snapshots:\n2026-10-31-150000\n' ;;
  *) echo "unexpected tmutil arguments: $*" >&2; exit 9 ;;
esac
"#;
// 1 MiB free while FAKE_DF_MARK exists, 2 MiB once it is gone
const FAKE_DF: &str = r#"
[ -n "$FAKE_DF_FAILS" ] && { echo "df failed" >&2; exit 1; }
available=1024
[ -n "$FAKE_DF_MARK" ] && [ ! -e "$FAKE_DF_MARK" ] && available=2048
printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/disk3s5 4096 1024 %s 50%% /\n' "$available"
"#;

// A scratch directory: bin/ with the fakes, tree/ to run in, outside/ for what must survive, and
// the fakes' log. All of it goes when the test ends, also when it fails.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(test: &str) -> Scratch {
        // Canonical: temp_dir() is below the symlink /var, and mclean prints the real path
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = temp.join(format!("mclean-cli-{}-{test}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let scratch = Scratch { root };
        for dir in ["bin", "tree", "outside"] {
            fs::create_dir(scratch.root.join(dir)).unwrap();
        }
        scratch.fake("docker", FAKE_DOCKER);
        scratch.fake("tmutil", FAKE_TMUTIL);
        scratch.fake("df", FAKE_DF);
        scratch
    }

    fn fake(&self, name: &str, body: &str) {
        let path = self.root.join("bin").join(name);
        let log = self.log();
        let script = format!(
            "#!/bin/bash\necho \"{name} $*\" >> '{}'\n{body}",
            log.display()
        );
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn log(&self) -> PathBuf {
        self.root.join("calls.log")
    }

    fn tree(&self, path: &str) -> PathBuf {
        self.root.join("tree").join(path)
    }

    fn outside(&self, path: &str) -> PathBuf {
        self.root.join("outside").join(path)
    }

    // mclean in tree/, with stdin closed and PATH always set: libc falls back to /usr/bin:/bin
    // without it, which holds the real tmutil
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mclean"));
        command
            .env_clear()
            .env("PATH", self.root.join("bin"))
            .current_dir(self.root.join("tree"))
            .stdin(Stdio::null());
        command
    }

    // The same, once the preflight has shown the fakes are what runs
    fn mclean(&self) -> Command {
        preflight();
        self.command()
    }

    fn calls(&self) -> Vec<String> {
        match fs::read_to_string(self.log()) {
            Ok(calls) => calls.lines().map(String::from).collect(),
            Err(_) => Vec::new(),
        }
    }

    // Every path in tree/ and outside/, links not followed
    fn listing(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let mut dirs = vec![self.root.join("tree"), self.root.join("outside")];
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    dirs.push(entry.path());
                }
                paths.push(entry.path());
            }
        }
        paths.sort();
        paths
    }

    // A tree with something of every kind: a run removes BUILD_DIRS and OLD_LOGS, and keeps OLD_KEPT
    // and the rest
    fn project_tree(&self) {
        for path in [
            "js/package.json",
            "js/node_modules/a/index.js",
            "js/.next/cache/x",
            "rs/Cargo.toml",
            "rs/target/debug/app",
            "lone/node_modules/keep.js",
            "lone/.next/keep",
            "lone/target/keep",
            "svc/logs/devnet/agent/agent.2026-10-31",
        ] {
            put(&self.tree(path));
        }
        for path in OLD_LOGS.iter().chain(OLD_KEPT) {
            put_old(&self.tree(path));
        }
        put(&self.outside("package.json"));
        put(&self.outside("node_modules/o.js"));
        put_old(&self.outside("logs/devnet/old.2026-01-01"));
        symlink(self.outside(""), self.tree("svc/link-out")).unwrap();
        symlink(
            self.outside("node_modules/o.js"),
            self.tree("js/node_modules/out.js"),
        )
        .unwrap();
    }

    fn assert_planned_items_gone(&self) {
        for path in BUILD_DIRS.iter().chain(OLD_LOGS) {
            assert!(!self.tree(path).exists(), "{path} is still there");
        }
    }

    fn assert_the_rest_stays(&self) {
        for path in OLD_KEPT {
            assert!(self.tree(path).exists(), "{path} was removed");
        }
        for path in [
            "js/package.json",
            "rs/Cargo.toml",
            "lone/node_modules/keep.js",
            "lone/.next/keep",
            "lone/target/keep",
            "svc/logs/devnet/agent/agent.2026-10-31",
        ] {
            assert!(self.tree(path).exists(), "{path} was removed");
        }
        assert!(self.tree("svc/link-out").is_symlink());
        assert!(self.outside("node_modules/o.js").exists());
        assert!(self.outside("logs/devnet/old.2026-01-01").exists());
    }
}

const BUILD_DIRS: &[&str] = &["js/node_modules", "js/.next", "rs/target"];
const OLD_LOGS: &[&str] = &[
    "svc/logs/devnet/agent/agent.2026-01-01",
    "svc/logs/mainnet/sub/deep/node.2026-01-01",
    "svc/logs/testnet/x.2026-01-01.pre-rebuild-073743",
    "svc/logs/mainnet/holdings.txt",
    "svc/logs/mainnet/.DS_Store",
];
const OLD_KEPT: &[&str] = &[
    "svc/logs/mainnet/.git/old.2026-01-01",
    "svc/logs/other/old.2026-01-01",
    "svc/logs/loose.2026-01-01",
    "svc/logs/rpc/testnet/old.2026-01-01",
    "svc/devnet/old.2026-01-01",
    "svc/.git/logs/devnet/old.2026-01-01",
];

impl Drop for Scratch {
    fn drop(&mut self) {
        // Make locked directories removable. set_permissions follows links, and fixtures hold links
        // out by design, so only real directories are touched.
        let mut dirs = vec![self.root.clone()];
        while let Some(dir) = dirs.pop() {
            let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o755));
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    dirs.push(entry.path());
                }
            }
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

// PATH is the only thing between these tests and the real tmutil and docker: it holds only while
// mclean calls them by bare name. So the first run is a dry one, whose probes only read, and every
// other test waits for it to show that the fakes answered.
fn preflight() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        let scratch = Scratch::new("preflight");
        let status = scratch.command().arg("--dry-run").status().unwrap();
        assert!(status.success());
        let calls = scratch.calls();
        let fakes_ran = [INSPECT, LIST, SYSTEM_DF]
            .iter()
            .all(|call| calls.iter().any(|logged| logged == call));
        assert!(fakes_ran, "the fakes did not answer: {calls:?}");
    });
}

fn put(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "data").unwrap();
}

// A file last modified 60 days ago
fn put_old(path: &Path) {
    put(path);
    let old = SystemTime::now() - Duration::from_hours(60 * 24);
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(old)
        .unwrap();
}

// Runs command with answer on stdin. A whole line: a stdin pipe's EOF cannot be relied on, because a
// parallel test's child can inherit the write end.
fn answering(command: &mut Command, answer: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(answer.as_bytes()).unwrap();
    drop(stdin);
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn assert_has(output: &str, wanted: &str) {
    assert!(output.contains(wanted), "no {wanted:?} in:\n{output}");
}

fn assert_lacks(output: &str, unwanted: &str) {
    assert!(!output.contains(unwanted), "{unwanted:?} in:\n{output}");
}

// A locked directory stops this user: not so for root, or on a filesystem that ignores modes
fn modes_are_enforced(scratch: &Scratch) -> bool {
    let probe = scratch.root.join("probe");
    fs::create_dir(&probe).unwrap();
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o555)).unwrap();
    let enforced = File::create(probe.join("x")).is_err();
    if !enforced {
        let _ = writeln!(
            io::stderr(),
            "skipped: directory modes are not enforced here"
        );
    }
    enforced
}

#[test]
fn bad_arguments_exit_2_and_touch_nothing() {
    let scratch = Scratch::new("bad-arguments");
    scratch.project_tree();
    let before = scratch.listing();
    for bad in ["--bogus", "somepath", "--dryrun", "-ny", ""] {
        let output = scratch.mclean().arg(bad).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{bad:?}");
        assert!(output.stdout.is_empty(), "{bad:?}");
        assert_has(&text(&output.stderr), "Usage: mclean");
    }
    assert_eq!(scratch.listing(), before);
    assert!(scratch.calls().is_empty());
}

#[test]
fn version_and_help_run_nothing() {
    let scratch = Scratch::new("version");
    scratch.project_tree();
    let before = scratch.listing();
    let output = scratch.mclean().arg("--version").output().unwrap();
    assert!(output.status.success());
    let version = format!("mclean {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(text(&output.stdout), version);
    let output = scratch.mclean().arg("-h").output().unwrap();
    assert!(output.status.success());
    assert_has(&text(&output.stdout), "Usage: mclean");
    assert_eq!(scratch.listing(), before);
    assert!(scratch.calls().is_empty());
}

#[test]
fn a_dry_run_wins_over_yes_and_only_reads() {
    let scratch = Scratch::new("dry-run");
    scratch.project_tree();
    let before = scratch.listing();
    let output = scratch
        .mclean()
        .args(["--dry-run", "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(scratch.listing(), before);
    assert_eq!(scratch.calls(), [INSPECT, SYSTEM_DF, LIST]);
    let stdout = text(&output.stdout);
    for line in [
        format!("Remove target: {}", scratch.tree("rs/target").display()),
        format!("Remove .next: {}", scratch.tree("js/.next").display()),
        format!(
            "Remove node_modules: {}",
            scratch.tree("js/node_modules").display()
        ),
        format!(
            "Remove 1 file(s) older than 30 days (4.00 KiB) in {}",
            scratch.tree("svc/logs/devnet").display()
        ),
        format!(
            "Remove 3 file(s) older than 30 days (12.00 KiB) in {}",
            scratch.tree("svc/logs/mainnet").display()
        ),
        "Run: docker --context desktop-linux system prune -a -f".to_string(),
        "Run: docker --context desktop-linux builder prune -a -f --builder desktop-linux"
            .to_string(),
        "context desktop-linux at unix:///var/run/docker.sock".to_string(),
        "Time Machine: 1 local snapshot(s) of /".to_string(),
        "No thinning: this plan removes less than 1.00 GiB".to_string(),
        "Dry run: nothing was removed.".to_string(),
    ] {
        assert_has(&stdout, &line);
    }
    assert_lacks(&stdout, "Proceed?");
}

#[test]
fn declining_removes_nothing() {
    let scratch = Scratch::new("declining");
    scratch.project_tree();
    let before = scratch.listing();
    for answer in ["n\n", "\n", "yess\n"] {
        let output = answering(&mut scratch.mclean(), answer);
        assert_eq!(output.status.code(), Some(1), "{answer:?}");
        assert_has(&text(&output.stdout), "Proceed? [y/N] ");
        assert_has(&text(&output.stdout), "Aborted: nothing was removed.");
    }
    // EOF
    let output = scratch.mclean().output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(scratch.listing(), before);
    let probes_only = scratch
        .calls()
        .iter()
        .all(|call| [INSPECT, SYSTEM_DF, LIST].contains(&call.as_str()));
    assert!(probes_only, "{:?}", scratch.calls());
}

// Removals, then the prunes, each once and in order, and the free space measured around them
fn assert_a_confirmed_run(scratch: &Scratch, output: &Output) {
    assert_eq!(output.status.code(), Some(0));
    scratch.assert_planned_items_gone();
    scratch.assert_the_rest_stays();
    assert_eq!(
        scratch.calls(),
        [
            INSPECT,
            SYSTEM_DF,
            LIST,
            DF,
            SYSTEM_PRUNE,
            BUILDER_PRUNE,
            DF
        ]
    );
    let stdout = text(&output.stdout);
    for line in [
        "Node.js modules removed: 1 (",
        "Rust target directories removed: 1 (",
        "Next.js .next directories removed: 1 (",
        "Old files in log folders removed: 5 (",
        "Total removed (files): ",
        "Docker reclaimed: 1.2GB",
        "Docker reclaimed: 0B",
        "Cleanup completed successfully!",
        "Free space: 1.00 MiB before, 2.00 MiB after (gained 1.00 MiB)",
        "Docker's share is not in that figure yet",
        "1 local snapshot(s) of / may still hold deleted files. To free that space run: tmutil thinlocalsnapshots / 99999999999 4",
    ] {
        assert_has(&stdout, line);
    }
}

#[test]
fn answering_yes_removes_the_plan_and_prunes_in_order() {
    let scratch = Scratch::new("answer-yes");
    scratch.project_tree();
    let mut command = scratch.mclean();
    command.env("FAKE_DF_MARK", scratch.tree("js/node_modules"));
    let output = answering(&mut command, "y\n");
    assert_a_confirmed_run(&scratch, &output);
}

#[test]
fn yes_needs_no_answer() {
    let scratch = Scratch::new("flag-yes");
    scratch.project_tree();
    let output = scratch
        .mclean()
        .arg("-y")
        .env("FAKE_DF_MARK", scratch.tree("js/node_modules"))
        .output()
        .unwrap();
    assert_a_confirmed_run(&scratch, &output);
    assert_lacks(&text(&output.stdout), "Proceed?");
}

// A run with clones of one seed in a target, each counting in full. They cost almost no disk, so a
// real 1 GiB is not needed. count gets the size of a clone. Returns the calls and the output.
fn run_with_clones(test: &str, count: impl Fn(u64) -> u64) -> (Vec<String>, String) {
    let scratch = Scratch::new(test);
    put(&scratch.tree("rs/Cargo.toml"));
    let seed = scratch.root.join("seed");
    let mut file = File::create(&seed).unwrap();
    file.write_all(&vec![7; 4 << 20]).unwrap();
    file.sync_all().unwrap();
    let target = scratch.tree("rs/target");
    fs::create_dir(&target).unwrap();
    fs::copy(&seed, target.join("c0")).unwrap();
    let each = fs::metadata(target.join("c0")).unwrap().blocks() * 512;
    for clone in 1..count(each) {
        fs::copy(&seed, target.join(format!("c{clone}"))).unwrap();
    }
    let output = scratch.mclean().arg("-y").output().unwrap();
    assert!(output.status.success());
    (scratch.calls(), text(&output.stdout))
}

#[test]
fn thinning_starts_at_one_gib_and_comes_before_the_prunes() {
    const GIB: u64 = 1 << 30;
    let (calls, stdout) = run_with_clones("below-gib", |each| (GIB - 1) / each);
    assert_eq!(
        calls,
        [
            INSPECT,
            SYSTEM_DF,
            LIST,
            DF,
            SYSTEM_PRUNE,
            BUILDER_PRUNE,
            DF
        ]
    );
    assert_has(&stdout, "No thinning: this plan removes less than 1.00 GiB");
    assert_has(&stdout, "may still hold deleted files");

    let (calls, stdout) = run_with_clones("at-gib", |each| (GIB - 1) / each + 1);
    assert_eq!(
        calls,
        [
            INSPECT,
            SYSTEM_DF,
            LIST,
            DF,
            THIN,
            LIST,
            SYSTEM_PRUNE,
            BUILDER_PRUNE,
            DF
        ]
    );
    assert_has(
        &stdout,
        "Then, if 1.00 GiB or more was removed, run: tmutil thinlocalsnapshots / 99999999999 4",
    );
    assert_has(&stdout, "may delete every local snapshot");
    assert_has(&stdout, "Local snapshots left: 1");
}

#[test]
fn a_failed_removal_exits_1_and_the_rest_goes_on() {
    let scratch = Scratch::new("failed-removal");
    if !modes_are_enforced(&scratch) {
        return;
    }
    put(&scratch.tree("app/Cargo.toml"));
    let locked = scratch.tree("app/target/locked");
    put(&locked.join("stuck.o"));
    // Sorts after app, so it runs after the failure
    put(&scratch.tree("web/package.json"));
    put(&scratch.tree("web/node_modules/a.js"));
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();

    let output = scratch.mclean().arg("-y").output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let (stdout, stderr) = (text(&output.stdout), text(&output.stderr));
    assert_has(&stdout, "Cleanup completed with failures");
    assert_has(&stdout, "Failed to remove: 1");
    assert_has(&stdout, "Rust target directories removed: 0");
    assert_has(&stderr, "Trying alternative method");
    assert_has(&stderr, "rm command failed");
    assert!(locked.join("stuck.o").exists());
    assert!(scratch.tree("app/Cargo.toml").exists());
    assert!(!scratch.tree("web/node_modules").exists());
    let calls = scratch.calls();
    assert!(calls.iter().any(|call| call == SYSTEM_PRUNE));
    assert!(calls.iter().any(|call| call == BUILDER_PRUNE));
}

#[test]
fn a_context_on_another_machine_is_never_pruned() {
    for (test, context) in [
        ("tcp", "remote tcp://10.0.0.1:2376"),
        ("ssh", "box ssh://me@box"),
        ("forwarded", "fwd unix:///tmp/prod.sock"),
        ("no-host", "onlyname"),
    ] {
        let scratch = Scratch::new(&format!("context-{test}"));
        scratch.project_tree();
        let output = scratch
            .mclean()
            .arg("-y")
            .env("FAKE_CONTEXT", context)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{context}");
        scratch.assert_planned_items_gone();
        let stdout = text(&output.stdout);
        assert_has(&stdout, "Warning: skipping Docker");
        assert_has(&stdout, "Docker skipped: ");
        assert_has(&stdout, "Cleanup completed with warnings");
        assert_eq!(scratch.calls(), [INSPECT, LIST, DF, DF], "{context}");
    }
}

#[test]
fn no_docker_is_a_warning() {
    let scratch = Scratch::new("no-docker");
    scratch.project_tree();
    fs::remove_file(scratch.root.join("bin/docker")).unwrap();
    let output = scratch.mclean().arg("-y").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    scratch.assert_planned_items_gone();
    let stdout = text(&output.stdout);
    assert_has(&stdout, "Warning: skipping Docker: docker not found");
    assert_has(&stdout, "Docker skipped: docker not found");
    assert_has(&stdout, "Cleanup completed with warnings");
}

#[test]
fn a_daemon_that_is_down_is_a_warning() {
    let scratch = Scratch::new("daemon-down");
    scratch.project_tree();
    let output = scratch
        .mclean()
        .arg("-y")
        .env("FAKE_SYSTEM_DF_FAILS", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    scratch.assert_planned_items_gone();
    assert_has(&text(&output.stdout), "Warning: skipping Docker");
    assert_eq!(scratch.calls(), [INSPECT, SYSTEM_DF, LIST, DF, DF]);
}

#[test]
fn a_failed_prune_is_a_warning_and_the_next_one_still_runs() {
    let scratch = Scratch::new("prune-fails");
    scratch.project_tree();
    let output = scratch
        .mclean()
        .arg("-y")
        .env("FAKE_PRUNE_FAILS", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = text(&output.stdout);
    assert_has(&stdout, "Cleanup completed with warnings");
    assert_has(&stdout, "Warnings: 1");
    assert_has(&text(&output.stderr), "prune failed");
    assert_eq!(
        scratch.calls(),
        [
            INSPECT,
            SYSTEM_DF,
            LIST,
            DF,
            SYSTEM_PRUNE,
            BUILDER_PRUNE,
            DF
        ]
    );
}

#[test]
fn df_and_tmutil_problems_are_warnings() {
    let scratch = Scratch::new("df-fails");
    scratch.project_tree();
    let output = scratch
        .mclean()
        .arg("-y")
        .env("FAKE_DF_FAILS", "1")
        .env("FAKE_TMUTIL_FAILS", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    scratch.assert_planned_items_gone();
    let stderr = text(&output.stderr);
    assert_has(&stderr, "Warning: cannot list local snapshots");
    assert_has(&stderr, "Warning: cannot measure free space");
    let stdout = text(&output.stdout);
    assert_has(&stdout, "Cleanup completed with warnings");
    assert_lacks(&stdout, "Docker's share");
}

#[test]
fn off_the_startup_volume_nothing_is_thinned() {
    let scratch = Scratch::new("other-volume");
    let output = scratch
        .mclean()
        .current_dir("/dev")
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = text(&output.stdout);
    assert_has(
        &stdout,
        "No thinning: this directory is not on the startup volume",
    );
    assert_lacks(&stdout, "tmutil");
    assert_eq!(scratch.calls(), [INSPECT, SYSTEM_DF]);
}

#[test]
fn an_empty_tree_without_docker_has_nothing_to_clean() {
    let scratch = Scratch::new("empty-no-docker");
    fs::remove_file(scratch.root.join("bin/docker")).unwrap();
    let output = scratch.mclean().output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = text(&output.stdout);
    assert_has(&stdout, "Nothing to clean.");
    assert_has(&stdout, "may still hold deleted files");
    assert_lacks(&stdout, "Proceed?");
}

#[test]
fn an_empty_tree_with_docker_still_asks_and_prunes() {
    let scratch = Scratch::new("empty-docker");
    let output = answering(&mut scratch.mclean(), "y\n");
    assert_eq!(output.status.code(), Some(0));
    assert_has(&text(&output.stdout), "Total removed (files): 0 B");
    assert_eq!(
        scratch.calls(),
        [
            INSPECT,
            SYSTEM_DF,
            LIST,
            DF,
            SYSTEM_PRUNE,
            BUILDER_PRUNE,
            DF
        ]
    );
}

#[test]
fn an_unreadable_start_directory_exits_1_before_anything_runs() {
    let scratch = Scratch::new("unreadable-start");
    if !modes_are_enforced(&scratch) {
        return;
    }
    let tree = scratch.tree("");
    fs::set_permissions(&tree, fs::Permissions::from_mode(0o311)).unwrap();
    let output = scratch.mclean().arg("-y").output().unwrap();
    fs::set_permissions(&tree, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_has(&text(&output.stderr), "cannot read the current directory");
    assert!(scratch.calls().is_empty());
}

#[test]
fn a_closed_stdout_and_stderr_do_not_stop_the_run() {
    // Only stdout closed
    let scratch = Scratch::new("closed-stdout");
    scratch.project_tree();
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let status = scratch
        .mclean()
        .arg("-y")
        .stdout(writer)
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    scratch.assert_planned_items_gone();
    assert_eq!(
        scratch.calls(),
        [
            INSPECT,
            SYSTEM_DF,
            LIST,
            DF,
            SYSTEM_PRUNE,
            BUILDER_PRUNE,
            DF
        ]
    );

    // Both, with a warning on the way: no docker
    let scratch = Scratch::new("closed-both");
    scratch.project_tree();
    fs::remove_file(scratch.root.join("bin/docker")).unwrap();
    let (reader, writer) = io::pipe().unwrap();
    let stderr = writer.try_clone().unwrap();
    drop(reader);
    let status = scratch
        .mclean()
        .arg("-y")
        .stdout(writer)
        .stderr(stderr)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    scratch.assert_planned_items_gone();

    // A bad argument still exits 2
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let status = scratch
        .mclean()
        .arg("--bogus")
        .stderr(writer)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(2));
}

#[test]
fn a_name_in_the_tree_cannot_draw_on_the_terminal() {
    let scratch = Scratch::new("hostile-name");
    let name = "x\u{1b}[2J\nRemove target: fake (1 B)";
    put(&scratch.tree(&format!("{name}/package.json")));
    put(&scratch.tree(&format!("{name}/node_modules/a.js")));
    let output = scratch.mclean().arg("--dry-run").output().unwrap();
    let stdout = text(&output.stdout);
    assert_lacks(&stdout, "\u{1b}");
    assert_lacks(&stdout, "\nRemove target: fake");
    assert_has(&stdout, "x\\u{1b}[2J\\nRemove target: fake (1 B)");
    assert!(!text(&output.stderr).contains('\u{1b}'));
}

// Reads stdout up to the prompt, swaps things while mclean waits there, answers yes
#[test]
fn a_swap_at_the_prompt_is_skipped() {
    let scratch = Scratch::new("swap");
    put(&scratch.tree("web/package.json"));
    put(&scratch.tree("web/node_modules/w.js"));
    put_old(&scratch.tree("svc/logs/devnet/agent/thesis.2026-01-01"));
    // Outside: the same shape, so only their identity tells them from the planned items
    put(&scratch.outside("proj/package.json"));
    put(&scratch.outside("proj/node_modules/v.js"));
    put_old(&scratch.outside("victim/thesis.2026-01-01"));

    let mut child = scratch
        .mclean()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut seen = Vec::new();
    while !seen.ends_with(PROMPT) {
        let mut buffer = [0; 4096];
        let read = stdout.read(&mut buffer).unwrap();
        assert!(read > 0, "no prompt in:\n{}", text(&seen));
        seen.extend_from_slice(&buffer[..read]);
    }
    assert!(scratch.tree("web/node_modules/w.js").exists());
    assert_eq!(scratch.calls(), [INSPECT, SYSTEM_DF, LIST]);

    let moved = scratch.root.join("moved");
    fs::create_dir(&moved).unwrap();
    fs::rename(scratch.tree("web"), moved.join("web")).unwrap();
    symlink(scratch.outside("proj"), scratch.tree("web")).unwrap();
    let agent = scratch.tree("svc/logs/devnet/agent");
    fs::rename(&agent, moved.join("agent")).unwrap();
    symlink(scratch.outside("victim"), &agent).unwrap();

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"y\n").unwrap();
    drop(stdin);
    let mut rest = String::new();
    stdout.read_to_string(&mut rest).unwrap();
    let status = child.wait().unwrap();

    assert_eq!(status.code(), Some(0));
    assert!(scratch.outside("proj/node_modules/v.js").exists());
    assert!(scratch.outside("victim/thesis.2026-01-01").exists());
    assert!(moved.join("web/node_modules/w.js").exists());
    assert!(moved.join("agent/thesis.2026-01-01").exists());
    assert_has(&rest, "Skipped (changed since the scan): 2\n");
    assert_has(&rest, "Cleanup completed with warnings");
}
