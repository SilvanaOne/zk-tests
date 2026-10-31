use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, ErrorKind, IsTerminal, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// println! and eprintln! that ignore a closed stream, so `mclean -y 2>&1 | head` cannot stop a
// cleanup halfway, and that escape what a name could use to draw on the terminal
macro_rules! out {
    ($($arg:tt)*) => {{
        let _ = writeln!(io::stdout(), "{}", printable(format_args!($($arg)*)));
    }};
}

macro_rules! err {
    ($($arg:tt)*) => {{
        let _ = writeln!(io::stderr(), "{}", printable(format_args!($($arg)*)));
    }};
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
// Files in logs/devnet, logs/testnet and logs/mainnet not modified for this long are removed
const LOG_MAX_AGE_DAYS: u64 = 30;
const LOG_MAX_AGE: Duration = Duration::from_hours(LOG_MAX_AGE_DAYS * 24);
// macOS: a local Time Machine snapshot keeps deleted files allocated until it is thinned, which
// mclean does once it has removed at least this much
const THIN_MIN_BYTES: u64 = 1 << 30;
const THIN_ARGS: [&str; 4] = ["thinlocalsnapshots", "/", "99999999999", "4"];
// The name of the current docker context and the address of its daemon
const CONTEXT_INSPECT: [&str; 4] = [
    "context",
    "inspect",
    "--format",
    "{{.Name}} {{.Endpoints.docker.Host}}",
];
// How long a command may run before it is stopped and its step skipped with a warning
const PROBE_LIMIT: Duration = Duration::from_secs(60);
const PRUNE_LIMIT: Duration = Duration::from_mins(10);
const THIN_LIMIT: Duration = Duration::from_mins(5);
// How long to wait for a stopped command to go, or for output its own child keeps open
const GRACE: Duration = Duration::from_secs(2);

const USAGE: &str = "\
Usage: mclean [-n | --dry-run] [-y | --yes]

Scans the current directory and everything below it, prints what it would do, and asks first.
In this order, it:
  removes build directories: node_modules and .next next to a package.json, target next to a
  Cargo.toml
  removes every regular file older than 30 days (last modified), whatever its name, at any depth,
  in logs/devnet, logs/testnet and logs/mainnet; the folders, symlinks, sockets and FIFOs stay
  on macOS, once 1 GiB or more was removed from the startup volume, thins local Time Machine
  snapshots (they keep deleted files allocated), which can delete all of them
  runs docker system prune -a -f, then docker builder prune -a -f on the builder of the current
  docker context, if its socket is this machine's (machine-wide; volumes are kept)
A confirmed run on macOS prints the free space before and after. The preview asks Docker for its
disk usage, which can wake Docker Desktop's VM.
Sizes are allocated blocks at scan time, a hard-linked file once, only when all its links are
removed. An APFS clone counts in full, as in du, though it shares its blocks, so the space freed
can be a few percent less; the free space line shows the real gain.
Symbolic links are never followed. The scan skips .git directories and other volumes mounted
below it; a build directory is removed whole, with any .git inside it.
Run it at or above a logs directory: from inside logs/devnet it finds none of those files.

Options:
  -n, --dry-run  Print the plan and exit; remove nothing (wins over --yes)
  -y, --yes      Do all of it without asking
  -h, --help     Print this help
  -V, --version  Print the version

Exit status: 0 done, nothing to do, or a dry run; 1 a removal failed, the answer was not
yes, or the current directory cannot be read; 2 bad arguments.
";

#[derive(Debug, PartialEq)]
enum Mode {
    Clean(Confirm),
    Help,
    Version,
}

// What happens once the plan is printed
#[derive(Clone, Copy, Debug, PartialEq)]
enum Confirm {
    DryRun,
    Ask,
    Yes,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    NodeModules,
    Next,
    Target,
}

// A build directory, removed whole
struct BuildDir {
    kind: Kind,
    path: PathBuf,
    size: u64,
    id: (u64, u64),
}

// A logs/devnet, logs/testnet or logs/mainnet folder and its files older than the cut-off
struct LogFolder {
    path: PathBuf,
    files: Vec<LogFile>,
}

struct LogFile {
    path: PathBuf,
    size: u64,
    id: (u64, u64),
    mtime: (i64, i64),
}

// What the scan found to remove; scanning only reads
#[derive(Default)]
struct Plan {
    // Where the scan started, and that directory's device
    start: PathBuf,
    dev: u64,
    dirs: Vec<BuildDir>,
    logs: Vec<LogFolder>,
    shared: SharedInodes,
    warnings: u32,
}

// Files with more than one hard link met while sizing the plan, by (device, inode)
type SharedInodes = HashMap<(u64, u64), Shared>;

// A file with more than one link frees its blocks only once every link is removed
struct Shared {
    links_seen: u64,
    links: u64,
    bytes: u64,
    owner: Owner,
}

// The plan item a shared file's bytes count for: a build dir, or a file of a log folder
#[derive(Clone, Copy)]
enum Owner {
    Dir(usize),
    Log(usize, usize),
}

// The Docker step: the local context to prune (its builder has the same name), its socket, and
// `docker system df` for the preview
struct DockerPlan {
    context: String,
    host: String,
    usage: String,
}

#[derive(Default)]
struct Tally {
    count: u32,
    bytes: u64,
}

// What a run removed, and what went wrong
#[derive(Default)]
struct Outcome {
    node_modules: Tally,
    next_dirs: Tally,
    target_dirs: Tally,
    old_logs: Tally,
    // Planned items that changed or went away between the scan and their removal
    skipped: u32,
    // "path: error" for each removal that failed; any of them makes the exit code 1
    failures: Vec<String>,
    // A prune reported reclaiming something
    docker_freed: bool,
    // Scan, Docker and tmutil problems: reported, never failures
    warnings: u32,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::NodeModules => "node_modules",
            Kind::Next => ".next",
            Kind::Target => "target",
        }
    }
}

impl LogFolder {
    fn size(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }
}

impl Plan {
    fn dir_bytes(&self) -> u64 {
        self.dirs.iter().map(|dir| dir.size).sum()
    }

    fn log_bytes(&self) -> u64 {
        self.logs.iter().map(LogFolder::size).sum()
    }
}

impl fmt::Display for Tally {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.count, format_size(self.bytes))
    }
}

impl Outcome {
    fn tally(&mut self, kind: Kind) -> &mut Tally {
        match kind {
            Kind::NodeModules => &mut self.node_modules,
            Kind::Next => &mut self.next_dirs,
            Kind::Target => &mut self.target_dirs,
        }
    }

    fn removed_bytes(&self) -> u64 {
        self.node_modules.bytes
            + self.next_dirs.bytes
            + self.target_dirs.bytes
            + self.old_logs.bytes
    }

    fn fail(&mut self, path: &Path, error: impl fmt::Display) {
        err!("Failed to remove {}: {error}", path.display());
        self.failures.push(format!("{}: {error}", path.display()));
    }

    fn skip(&mut self, path: &Path) {
        out!("Skipped (changed since the scan): {}", path.display());
        self.skipped += 1;
    }
}

fn main() -> ExitCode {
    // Arguments first: a typo or a path exits 2 before anything is read
    let confirm = match parse_args(std::env::args_os().skip(1)) {
        Ok(Mode::Clean(confirm)) => confirm,
        Ok(Mode::Help) => {
            out!("mclean {VERSION}");
            let _ = write!(io::stdout(), "{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Mode::Version) => {
            out!("mclean {VERSION}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            err!("mclean: {e}");
            let _ = write!(io::stderr(), "{USAGE}");
            return ExitCode::from(2);
        }
    };
    // Read it now: a denied directory (macOS privacy settings) must not look like nothing to do
    let start = match std::env::current_dir().and_then(|dir| fs::read_dir(&dir).map(|_| dir)) {
        Ok(dir) => dir,
        Err(e) => {
            err!("Error: cannot read the current directory: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mode = match confirm {
        Confirm::DryRun => "dry run, nothing will be removed",
        Confirm::Ask => "asks before removing",
        Confirm::Yes => "--yes, removes without asking",
    };
    out!("mclean v{VERSION} ({mode}): scanning {}", start.display());

    // Scan: read-only, builds the plan. The files print first: a hung docker cannot hide them.
    let macos = cfg!(target_os = "macos");
    let cutoff = log_cutoff(SystemTime::now());
    let plan = plan_cleanup(&start, cutoff);
    print_file_plan(&plan);
    out!("Checking Docker and local snapshots...");
    let docker = docker_preview();
    let thin_here = macos && on_startup_volume(&start);
    let snapshots = thin_here.then(list_snapshots);
    if macos {
        print_thin_plan(&plan, snapshots.as_ref());
    }
    print_docker_plan(&docker);

    if plan.dirs.is_empty() && plan.logs.is_empty() && docker.is_err() {
        if plan.warnings > 0 {
            out!("Nothing to clean in what could be read.");
        } else {
            out!("Nothing to clean.");
        }
        if let Some(Ok(names)) = &snapshots {
            print_thin_hint(names);
        }
        return ExitCode::SUCCESS;
    }
    if confirm == Confirm::DryRun {
        out!("Dry run: nothing was removed.");
        return ExitCode::SUCCESS;
    }
    // Not answering yes exits 1, as apt and terraform do: the cleanup asked for did not happen
    if confirm == Confirm::Ask && !(show_prompt() && confirm_proceed(&mut io::stdin().lock())) {
        out!("Aborted: nothing was removed.");
        return ExitCode::FAILURE;
    }

    // Execute: removals, then on macOS snapshot thinning, which returns their space, then Docker,
    // and the free space gained. Each step reports as it ends: a later one can take minutes.
    let free_before = macos.then(free_space);
    let mut outcome = execute(&plan);
    outcome.warnings += plan.warnings + u32::from(matches!(snapshots, Some(Err(_))));
    print_removals(&outcome);
    let left = if thin_here && outcome.removed_bytes() >= THIN_MIN_BYTES {
        thin_snapshots(&mut outcome)
    } else {
        snapshots.and_then(Result::ok)
    };
    match &docker {
        Ok(docker) => prune_docker(&docker.context, &mut outcome),
        Err(e) => {
            out!("Docker skipped: {e}");
            outcome.warnings += 1;
        }
    }
    print_status(&outcome);
    if let Some(before) = free_before {
        let after = free_space();
        let measured = before.is_ok() && after.is_ok();
        print_free_space(before, after);
        if measured && outcome.docker_freed {
            out!(
                "Docker's share is not in that figure yet: Docker Desktop returns it about 20 s after a prune."
            );
        }
    }
    if let Some(names) = &left {
        print_thin_hint(names);
    }
    if outcome.failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// Flags only. Anything else, a path included, is an error before anything is scanned, so a typo
// can never start a cleanup.
fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Mode, String> {
    let mut dry_run = false;
    let mut yes = false;
    let mut help = false;
    let mut version = false;
    for arg in args {
        match arg.to_str() {
            Some("-n" | "--dry-run") => dry_run = true,
            Some("-y" | "--yes") => yes = true,
            Some("-h" | "--help") => help = true,
            Some("-V" | "--version") => version = true,
            _ => return Err(format!("unknown argument '{}'", arg.display())),
        }
    }
    // --help, then --version, win over a run; --dry-run wins over --yes
    let mode = if help {
        Mode::Help
    } else if version {
        Mode::Version
    } else if dry_run {
        Mode::Clean(Confirm::DryRun)
    } else if yes {
        Mode::Clean(Confirm::Yes)
    } else {
        Mode::Clean(Confirm::Ask)
    };
    Ok(mode)
}

// Walks dir and plans what to remove. It only reads: an unreadable directory is a warning and the
// walk goes on.
fn scan_directory(dir: &Path, cutoff: SystemTime, plan: &mut Plan) {
    let in_logs = dir.file_name().is_some_and(|name| name == "logs");
    let entries = read_entries(dir, &mut plan.warnings);
    // A manifest is a regular file of exactly this name, from the listing: a stat would follow a
    // symlink, and on a case-insensitive volume also accept cargo.toml
    let has_file = |name: &str| {
        entries
            .iter()
            .any(|entry| entry.file_name() == name && entry.file_type().is_ok_and(|t| t.is_file()))
    };
    for entry in &entries {
        // file_type() does not follow symlinks: a linked directory is never entered or removed
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.path();
        // That type dates from the listing, up to a minute ago, and read_dir follows symlinks:
        // ask again, so a directory swapped for a link since then is not opened
        let meta = match entry.metadata() {
            Ok(meta) if meta.is_dir() => meta,
            Ok(_) => continue,
            Err(e) => {
                warn(
                    &mut plan.warnings,
                    &format!("cannot read {}: {e}", path.display()),
                );
                continue;
            }
        };
        if meta.dev() != plan.dev {
            warn_other_volume(&path, &mut plan.warnings);
            continue;
        }
        let file_name = entry.file_name();
        // node_modules and .next next to a package.json, target next to a Cargo.toml, and the
        // devnet, testnet and mainnet folders directly inside a logs directory
        let kind = match &*file_name.to_string_lossy() {
            "node_modules" if has_file("package.json") => Kind::NodeModules,
            ".next" if has_file("package.json") => Kind::Next,
            "target" if has_file("Cargo.toml") => Kind::Target,
            ".git" => continue,
            "devnet" | "testnet" | "mainnet" if in_logs => {
                scan_log_folder(path, cutoff, plan);
                continue;
            }
            _ => {
                // Recursively process subdirectories
                scan_directory(&path, cutoff, plan);
                continue;
            }
        };
        // The one sign of life in a scan that takes a minute or more
        err!("Sizing {}: {}", kind.name(), path.display());
        let dev = plan.dev;
        plan_build_dir(kind, path, &meta, dev, plan);
    }
}

// Sizes a build dir and plans it, unless something below it is not on device dev: that is another
// volume mounted inside, which removing the directory would empty
fn plan_build_dir(kind: Kind, path: PathBuf, meta: &fs::Metadata, dev: u64, plan: &mut Plan) {
    let owner = Owner::Dir(plan.dirs.len());
    // A map of its own: a directory dropped here must leave no links counted
    let mut shared = SharedInodes::new();
    let Some(contents) = dir_size(&path, dev, owner, &mut shared, &mut plan.warnings) else {
        let message = format!(
            "not removing {}: another volume is mounted inside it",
            path.display()
        );
        warn(&mut plan.warnings, &message);
        return;
    };
    merge_shared(&mut plan.shared, shared);
    plan.dirs.push(BuildDir {
        kind,
        path,
        size: disk_bytes(meta) + contents,
        id: file_id(meta),
    });
}

// Plans the files of a logs/devnet, logs/testnet or logs/mainnet folder older than the cut-off
fn scan_log_folder(path: PathBuf, cutoff: SystemTime, plan: &mut Plan) {
    let mut files = Vec::new();
    let folder = plan.logs.len();
    find_old_logs(&path, cutoff, folder, &mut files, plan);
    if !files.is_empty() {
        plan.logs.push(LogFolder { path, files });
    }
}

// Regular files below dir, at any depth, last modified before the cut-off, whatever their names.
// Directories stay, and so do symlinks and the .git directories the whole scan skips.
fn find_old_logs(
    dir: &Path,
    cutoff: SystemTime,
    folder: usize,
    files: &mut Vec<LogFile>,
    plan: &mut Plan,
) {
    for entry in read_entries(dir, &mut plan.warnings) {
        let path = entry.path();
        // DirEntry::metadata() does not follow symlinks: a link is neither a file nor a directory
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(e) => {
                warn(
                    &mut plan.warnings,
                    &format!("cannot read {}: {e}", path.display()),
                );
                continue;
            }
        };
        if meta.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            if meta.dev() != plan.dev {
                warn_other_volume(&path, &mut plan.warnings);
                continue;
            }
            find_old_logs(&path, cutoff, folder, files, plan);
        } else if meta.is_file() && is_old(&meta, cutoff) {
            let owner = Owner::Log(folder, files.len());
            files.push(LogFile {
                path,
                size: file_bytes(&meta, owner, &mut plan.shared),
                id: file_id(&meta),
                mtime: mtime(&meta),
            });
        }
    }
}

// Device and inode: the removal checks them again, because a directory replaced after the scan
// makes the same path name something else
fn file_id(meta: &fs::Metadata) -> (u64, u64) {
    (meta.dev(), meta.ino())
}

// The modification time as the scan saw it, to tell a file rewritten since then
fn mtime(meta: &fs::Metadata) -> (i64, i64) {
    (meta.mtime(), meta.mtime_nsec())
}

// True when a file was last modified before the cut-off; an unreadable time keeps the file
fn is_old(meta: &fs::Metadata, cutoff: SystemTime) -> bool {
    meta.modified().is_ok_and(|modified| modified < cutoff)
}

// Files last modified before this instant, 30 days before the scan, are old
fn log_cutoff(now: SystemTime) -> SystemTime {
    now.checked_sub(LOG_MAX_AGE).unwrap_or(UNIX_EPOCH)
}

// The entries of dir sorted by name; an unreadable directory or entry is a warning
fn read_entries(dir: &Path, warnings: &mut u32) -> Vec<fs::DirEntry> {
    let mut entries = Vec::new();
    match fs::read_dir(dir) {
        Ok(read) => {
            for entry in read {
                match entry {
                    Ok(entry) => entries.push(entry),
                    Err(e) => warn(warnings, &format!("cannot read in {}: {e}", dir.display())),
                }
            }
        }
        Err(e) => warn(warnings, &format!("cannot read {}: {e}", dir.display())),
    }
    entries.sort_by_key(|entry| entry.file_name());
    entries
}

// The allocated blocks of dir's contents, as du counts them: a sparse file counts what it occupies,
// an APFS clone counts in full. Symlinks are not followed, and what cannot be read counts as 0. None
// if another volume is mounted below dir: removing dir would empty it.
fn dir_size(
    dir: &Path,
    dev: u64,
    owner: Owner,
    shared: &mut SharedInodes,
    warnings: &mut u32,
) -> Option<u64> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        // Files come and go in a target a build is writing to
        Err(e) if gone(&e) => return Some(0),
        Err(e) => {
            warn(warnings, &format!("cannot size {}: {e}", dir.display()));
            return Some(0);
        }
    };
    let mut size = 0;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        size += if meta.is_dir() {
            if meta.dev() != dev {
                return None;
            }
            disk_bytes(&meta) + dir_size(&entry.path(), dev, owner, shared, warnings)?
        } else {
            file_bytes(&meta, owner, shared)
        };
    }
    Some(size)
}

// Allocated bytes: st_blocks counts 512-byte units
fn disk_bytes(meta: &fs::Metadata) -> u64 {
    meta.blocks().saturating_mul(512)
}

// What one link to a file counts for. With more links, nothing yet: the file is recorded in
// shared, and settle_shared counts it once if every link to it is in the plan.
fn file_bytes(meta: &fs::Metadata, owner: Owner, shared: &mut SharedInodes) -> u64 {
    if meta.nlink() <= 1 {
        return disk_bytes(meta);
    }
    let file = shared.entry(file_id(meta)).or_insert(Shared {
        links_seen: 0,
        links: meta.nlink(),
        bytes: disk_bytes(meta),
        owner,
    });
    file.links_seen += 1;
    0
}

// Adds what one build dir met to the plan's record; the bytes stay with the item that met it first
fn merge_shared(into: &mut SharedInodes, from: SharedInodes) {
    for (id, file) in from {
        let seen = file.links_seen;
        into.entry(id)
            .and_modify(|known| known.links_seen += seen)
            .or_insert(file);
    }
}

// Adds each hard-linked file whose links are all in the plan, once, to the item that met it
// first. One with a link elsewhere (a node_modules hard-linked to a package store) frees nothing.
fn settle_shared(plan: &mut Plan) {
    for file in plan.shared.values() {
        if file.links_seen < file.links {
            continue;
        }
        match file.owner {
            Owner::Dir(dir) => plan.dirs[dir].size += file.bytes,
            Owner::Log(folder, log) => plan.logs[folder].files[log].size += file.bytes,
        }
    }
}

// Scans start and sizes what it plans to remove
fn plan_cleanup(start: &Path, cutoff: SystemTime) -> Plan {
    let mut plan = Plan {
        start: start.to_path_buf(),
        ..Plan::default()
    };
    match fs::metadata(start) {
        Ok(meta) => plan.dev = meta.dev(),
        Err(e) => {
            warn(
                &mut plan.warnings,
                &format!("cannot read {}: {e}", start.display()),
            );
            return plan;
        }
    }
    scan_directory(start, cutoff, &mut plan);
    settle_shared(&mut plan);
    plan
}

// Read-only: the current docker context, if its socket is this machine's and its daemon answers,
// with the daemon's disk usage. The error says why the Docker step is skipped.
fn docker_preview() -> Result<DockerPlan, String> {
    let context = run("docker", &CONTEXT_INSPECT, PROBE_LIMIT)?;
    let home = std::env::home_dir().unwrap_or_default();
    let (name, host) = local_context(&context, &home)?;
    let usage = run("docker", &["--context", name, "system", "df"], PROBE_LIMIT)?;
    Ok(DockerPlan {
        context: name.to_string(),
        host: host.to_string(),
        usage,
    })
}

// The name and socket in `docker context inspect` output, if the socket is this machine's own: the
// system one or Docker Desktop's. A unix:// address alone proves nothing, because a socket can
// forward to another machine (ssh -L, Docker Offload).
fn local_context<'a>(inspect: &'a str, home: &Path) -> Result<(&'a str, &'a str), String> {
    // A context name has no spaces, and the trim leaves no empty name (an empty --builder would
    // mean whichever builder $BUILDX_BUILDER names)
    let Some((name, host)) = inspect.trim().split_once(' ') else {
        return Err(format!("unexpected docker context: {}", inspect.trim()));
    };
    let desktop = format!("unix://{}/.docker/run/docker.sock", home.display());
    if host != "unix:///var/run/docker.sock" && host != desktop {
        return Err(format!(
            "docker context {name} is not this machine's Docker socket ({host})"
        ));
    }
    Ok((name, host))
}

// macOS: the local Time Machine snapshots of the startup volume
fn list_snapshots() -> Result<Vec<String>, String> {
    let output = run("tmutil", &["listlocalsnapshots", "/"], PROBE_LIMIT)?;
    Ok(snapshot_names(&output))
}

// macOS: whether dir is on the volume thinning acts on. Both paths are compared: they share a
// device number on this Mac, but nothing promises that.
fn on_startup_volume(dir: &Path) -> bool {
    let dev = |path: &Path| fs::metadata(path).map(|meta| meta.dev()).ok();
    let here = dev(dir);
    here.is_some()
        && (here == dev(Path::new("/")) || here == dev(Path::new("/System/Volumes/Data")))
}

// What the scan plans to remove, printed before anything else can block
fn print_file_plan(plan: &Plan) {
    out!("");
    out!("Plan (nothing has been removed yet):");
    for dir in &plan.dirs {
        out!(
            "Remove {}: {} ({})",
            dir.kind.name(),
            dir.path.display(),
            format_size(dir.size)
        );
    }
    for folder in &plan.logs {
        out!(
            "Remove {} file(s) older than {LOG_MAX_AGE_DAYS} days ({}) in {}",
            folder.files.len(),
            format_size(folder.size()),
            folder.path.display()
        );
    }
    let (dir_bytes, log_bytes) = (plan.dir_bytes(), plan.log_bytes());
    let log_files: usize = plan.logs.iter().map(|folder| folder.files.len()).sum();
    out!(
        "To remove: {} directories ({}) and {log_files} files in log folders ({}), {} in total",
        plan.dirs.len(),
        format_size(dir_bytes),
        format_size(log_bytes),
        format_size(dir_bytes + log_bytes)
    );
    if plan.warnings > 0 {
        out!("Scan warnings: {} (see above)", plan.warnings);
    }
}

// macOS: the thinning step of the plan. No snapshots listed means this is not the startup volume.
fn print_thin_plan(plan: &Plan, snapshots: Option<&Result<Vec<String>, String>>) {
    let Some(snapshots) = snapshots else {
        out!("No thinning: this directory is not on the startup volume");
        return;
    };
    match snapshots {
        Ok(names) => out!("Time Machine: {} local snapshot(s) of /", names.len()),
        Err(e) => err!("Warning: cannot list local snapshots: {e}"),
    }
    if plan.dir_bytes() + plan.log_bytes() >= THIN_MIN_BYTES {
        out!(
            "Then, if {} or more was removed, run: tmutil {} (may delete every local snapshot of /, oldest first, until 100 GB more is free)",
            format_size(THIN_MIN_BYTES),
            THIN_ARGS.join(" ")
        );
    } else {
        out!(
            "No thinning: this plan removes less than {}",
            format_size(THIN_MIN_BYTES)
        );
    }
}

// The Docker step of the plan, or why there is none
fn print_docker_plan(docker: &Result<DockerPlan, String>) {
    match docker {
        Ok(docker) => {
            out!(
                "Docker (machine-wide, not only this directory), context {} at {}:",
                docker.context,
                docker.host
            );
            for args in prune_args(&docker.context) {
                out!("Run: docker {}", args.join(" "));
            }
            out!("Docker disk usage now (docker system df):");
            print_indented(&docker.usage);
        }
        Err(e) => out!("Warning: skipping Docker: {e}"),
    }
}

// Asks Proceed? [y/N]. Someone at a terminal must see the plan they answer: a stdout that cannot
// be written ends the run here, and one that goes elsewhere gets the question repeated on stderr.
// An answer piped in was given without looking anyway, like --yes.
fn show_prompt() -> bool {
    let mut stdout = io::stdout();
    let shown = write!(stdout, "Proceed? [y/N] ").and_then(|()| stdout.flush());
    if !io::stdin().is_terminal() {
        return true;
    }
    if let Err(e) = shown {
        err!("Error: cannot show the plan on stdout: {e}. Nothing was removed.");
        return false;
    }
    if !stdout.is_terminal() {
        let _ = write!(
            io::stderr(),
            "Proceed with the plan printed on stdout? [y/N] "
        );
    }
    true
}

// Reads one line: only y or yes proceeds; an empty line, EOF or a read error does not
fn confirm_proceed(input: &mut impl BufRead) -> bool {
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(0) => {
            out!("");
            false
        }
        Ok(_) => is_yes(&answer),
        Err(e) => {
            err!("");
            err!("Warning: cannot read the answer: {e}");
            false
        }
    }
}

// y or yes in any case, surrounding whitespace ignored
fn is_yes(answer: &str) -> bool {
    let answer = answer.trim();
    answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")
}

// Removes what the plan lists. Each item is checked again first: the answer may come minutes
// after the scan.
fn execute(plan: &Plan) -> Outcome {
    let mut outcome = Outcome::default();
    for dir in &plan.dirs {
        remove_build_dir(dir, &plan.start, &mut outcome);
    }
    for folder in &plan.logs {
        out!(
            "Removing {} file(s) ({}) in {}",
            folder.files.len(),
            format_size(folder.size()),
            folder.path.display()
        );
        for file in &folder.files {
            remove_log(file, &plan.start, &mut outcome);
        }
    }
    outcome
}

fn remove_build_dir(dir: &BuildDir, start: &Path, outcome: &mut Outcome) {
    // Still the same real directory: a symlink put in its place is left alone
    let same_dir = |meta: &fs::Metadata| meta.is_dir() && file_id(meta) == dir.id;
    // Before the check: a blocked terminal must not widen the gap between the check and the removal
    out!(
        "Removing {}: {} ({})",
        dir.kind.name(),
        dir.path.display(),
        format_size(dir.size)
    );
    if !recheck(&dir.path, start, same_dir, outcome) {
        return;
    }
    if let Err(e) = fs::remove_dir_all(&dir.path) {
        // Gone since the check: nothing was removed, so nothing is counted
        if gone(&e) {
            outcome.skip(&dir.path);
            return;
        }
        err!(
            "Warning: Failed to remove {}: {e}. Trying alternative method...",
            dir.path.display()
        );
        // That removal can take minutes, and the path may name something else by now
        if !recheck(&dir.path, start, same_dir, outcome) {
            outcome.tally(dir.kind).bytes += dir.size.saturating_sub(left_behind(dir));
            return;
        }
        if let Err(e2) = force_remove_dir(&dir.path) {
            let freed = dir.size.saturating_sub(left_behind(dir));
            outcome.tally(dir.kind).bytes += freed;
            let note = if freed > 0 {
                format!(" (partly removed: {} went)", format_size(freed))
            } else {
                String::new()
            };
            outcome.fail(&dir.path, format!("{e2}{note}"));
            return;
        }
    }
    let tally = outcome.tally(dir.kind);
    tally.count += 1;
    tally.bytes += dir.size;
}

// After a failed removal: what is left of dir, counted as the scan counted it. A path that is gone
// left nothing; anything that cannot be checked claims nothing went.
fn left_behind(dir: &BuildDir) -> u64 {
    let meta = match fs::symlink_metadata(&dir.path) {
        Ok(meta) if meta.is_dir() && file_id(&meta) == dir.id => meta,
        Err(e) if gone(&e) => return 0,
        _ => return dir.size,
    };
    let mut shared = SharedInodes::new();
    let mut warnings = 0;
    let owner = Owner::Dir(0);
    let Some(contents) = dir_size(&dir.path, meta.dev(), owner, &mut shared, &mut warnings) else {
        return dir.size;
    };
    let linked: u64 = shared
        .values()
        .filter(|file| file.links_seen >= file.links)
        .map(|file| file.bytes)
        .sum();
    disk_bytes(&meta) + contents + linked
}

fn remove_log(file: &LogFile, start: &Path, outcome: &mut Outcome) {
    // Still the same regular file, and not written to since the scan
    let unchanged = |meta: &fs::Metadata| {
        meta.is_file() && file_id(meta) == file.id && mtime(meta) == file.mtime
    };
    if !recheck(&file.path, start, unchanged, outcome) {
        return;
    }
    match fs::remove_file(&file.path) {
        Ok(()) => {
            outcome.old_logs.count += 1;
            outcome.old_logs.bytes += file.size;
        }
        Err(e) if gone(&e) => outcome.skip(&file.path),
        Err(e) => outcome.fail(&file.path, e),
    }
}

// The path no longer exists, or a parent directory on it was replaced by a file
fn gone(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
}

// Right before a removal: whether path still is what the scan saw (want), reached through real
// directories. Something that changed or is gone is skipped; any other error is a failure.
fn recheck(
    path: &Path,
    start: &Path,
    want: impl Fn(&fs::Metadata) -> bool,
    outcome: &mut Outcome,
) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) if want(&meta) && real_parents(path, start) => true,
        Err(e) if !gone(&e) => {
            outcome.fail(path, e);
            false
        }
        _ => {
            outcome.skip(path);
            false
        }
    }
}

// Every directory between start and path is still a real one. lstat resolves the parents, so a
// parent moved away and replaced by a symlink to it would still show the planned inode.
fn real_parents(path: &Path, start: &Path) -> bool {
    path.ancestors()
        .skip(1)
        .take_while(|dir| *dir != start)
        .all(|dir| fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()))
}

// Goes on past what it cannot remove, where remove_dir_all stops at the first error. The absolute
// path keeps it from finding a stray rm earlier on PATH.
fn force_remove_dir(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("/bin/rm").arg("-rf").arg(dir).output()?;

    if !output.status.success() {
        return Err(format!(
            "rm command failed: {}",
            first_line(&String::from_utf8_lossy(&output.stderr))
        )
        .into());
    }

    Ok(())
}

// The first line of text and how many more there were: rm can fail on every file of a large tree
fn first_line(text: &str) -> String {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("no message").to_string();
    match lines.count() {
        0 => first,
        rest => format!("{first} (and {rest} more lines)"),
    }
}

// docker system prune -a -f, then docker builder prune -a -f; never --volumes. Both name the
// context checked as local before the prompt, so a `docker context use` meanwhile changes
// nothing. The builder is named too: without --builder, `docker builder` takes it from
// $BUILDX_BUILDER, which may name the team's shared cloud builder, and only then from the
// current context. `docker buildx use` does not change this default.
fn prune_args(context: &str) -> [Vec<&str>; 2] {
    [
        vec!["--context", context, "system", "prune", "-a", "-f"],
        vec![
            "--context",
            context,
            "builder",
            "prune",
            "-a",
            "-f",
            "--builder",
            context,
        ],
    ]
}

// Runs both prunes and reports the size each one reclaimed; a failure is a warning
fn prune_docker(context: &str, outcome: &mut Outcome) {
    for args in prune_args(context) {
        out!("Running: docker {}", args.join(" "));
        match run("docker", &args, PRUNE_LIMIT) {
            Ok(output) => {
                let total = docker_total(&output).unwrap_or("unknown");
                out!("Docker reclaimed: {total}");
                outcome.docker_freed |= !matches!(total, "0B" | "unknown");
            }
            Err(e) => warn(&mut outcome.warnings, &e),
        }
    }
}

// The size on the Total line a prune ends with: "Total reclaimed space: 1.2GB" (system prune)
// or "Total: 1.2GB" (builder prune)
fn docker_total(output: &str) -> Option<&str> {
    let line = output.lines().rfind(|line| line.starts_with("Total"))?;
    line.split_once(':').map(|(_, size)| size.trim())
}

// macOS: deleted files stay allocated while a local snapshot holds them, and thinning gives that
// space back. Then lists the snapshots left, and returns them if that worked.
fn thin_snapshots(outcome: &mut Outcome) -> Option<Vec<String>> {
    out!(
        "Running: tmutil {} (can take a minute or two)",
        THIN_ARGS.join(" ")
    );
    match run("tmutil", &THIN_ARGS, THIN_LIMIT) {
        Ok(output) => print_indented(&output),
        Err(e) => warn(&mut outcome.warnings, &e),
    }
    match list_snapshots() {
        Ok(left) if left.is_empty() => {
            out!("Local snapshots left: none");
            Some(left)
        }
        Ok(left) => {
            out!("Local snapshots left: {}", left.len());
            for name in &left {
                out!("  {name}");
            }
            Some(left)
        }
        Err(e) => {
            warn(&mut outcome.warnings, &e);
            None
        }
    }
}

// The names tmutil listlocalsnapshots prints below its "Snapshots for disk /:" header
fn snapshot_names(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.ends_with(':'))
        .map(String::from)
        .collect()
}

// A dataless reference snapshot holds no file data, and thinning skips it
fn snapshots_holding_files(names: &[String]) -> usize {
    names
        .iter()
        .filter(|name| !name.ends_with("(dataless)"))
        .count()
}

// A later mclean run cannot do this: with nothing left to remove, it does not thin
fn print_thin_hint(names: &[String]) {
    let holding = snapshots_holding_files(names);
    if holding > 0 {
        out!(
            "{holding} local snapshot(s) of / may still hold deleted files. To free that space run: tmutil {} (may delete all of them)",
            THIN_ARGS.join(" ")
        );
    }
}

// Bytes available on the start directory's volume: df's Available column (KiB). df runs in the
// start directory, which is this process's current directory.
fn free_space() -> Result<u64, String> {
    let output = run("df", &["-kP", "."], PROBE_LIMIT)?;
    parse_df_available(&output).ok_or_else(|| format!("unexpected df output: {}", output.trim()))
}

// The Available column of df -kP, in bytes. It is the field before Capacity ("80%"): the device
// name and the mount point may contain spaces, so neither end of the line can be counted from.
fn parse_df_available(output: &str) -> Option<u64> {
    let line = output.lines().rfind(|line| !line.trim().is_empty())?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    for (i, field) in fields.iter().enumerate().skip(1) {
        if let Some(percent) = field.strip_suffix('%')
            && percent.parse::<u64>().is_ok()
        {
            return fields[i - 1].parse::<u64>().ok()?.checked_mul(1024);
        }
    }
    None
}

// What the removals did, printed as soon as they end
fn print_removals(outcome: &Outcome) {
    out!("");
    out!("=== Cleanup Statistics ===");
    out!("Node.js modules removed: {}", outcome.node_modules);
    out!("Rust target directories removed: {}", outcome.target_dirs);
    out!("Next.js .next directories removed: {}", outcome.next_dirs);
    out!("Old files in log folders removed: {}", outcome.old_logs);
    out!(
        "Total removed (files): {}",
        format_size(outcome.removed_bytes())
    );
    if outcome.skipped > 0 {
        out!("Skipped (changed since the scan): {}", outcome.skipped);
    }
    if !outcome.failures.is_empty() {
        out!("Failed to remove: {}", outcome.failures.len());
        for failure in &outcome.failures {
            out!("  {failure}");
        }
    }
}

// The last word on the run, once every step has ended
fn print_status(outcome: &Outcome) {
    if !outcome.failures.is_empty() {
        out!("Cleanup completed with failures (listed above).");
    } else if outcome.warnings > 0 || outcome.skipped > 0 {
        out!("Cleanup completed with warnings (see above).");
    } else {
        out!("Cleanup completed successfully!");
    }
    if outcome.warnings > 0 {
        out!("Warnings: {} (see above)", outcome.warnings);
    }
}

fn print_free_space(before: Result<u64, String>, after: Result<u64, String>) {
    match (before, after) {
        (Ok(before), Ok(after)) => {
            let change = if after >= before { "gained" } else { "lost" };
            out!(
                "Free space: {} before, {} after ({change} {})",
                format_size(before),
                format_size(after),
                format_size(after.abs_diff(before))
            );
        }
        (Err(e), _) | (_, Err(e)) => err!("Warning: cannot measure free space: {e}"),
    }
}

// Binary units: 1536 bytes is "1.50 KiB"
fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1 << 10;
    const MIB: u64 = 1 << 20;
    const GIB: u64 = 1 << 30;
    let size = bytes as f64;
    if bytes < KIB {
        format!("{bytes} B")
    } else if bytes < MIB {
        format!("{:.2} KiB", size / KIB as f64)
    } else if bytes < GIB {
        format!("{:.2} MiB", size / MIB as f64)
    } else {
        format!("{:.2} GiB", size / GIB as f64)
    }
}

fn print_indented(text: &str) {
    for line in text.lines() {
        out!("  {line}");
    }
}

// Text as the terminal should get it: control characters and the direction marks that can reorder
// a line are escaped, so a directory name cannot clear the screen or draw a fake plan. A tab cannot
// draw anything, and docker and tmutil output may hold one.
fn printable(text: fmt::Arguments<'_>) -> String {
    let mut shown = String::new();
    for c in text.to_string().chars() {
        let bidi = matches!(
            c,
            '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        );
        if bidi || (c.is_control() && c != '\t') {
            shown.extend(c.escape_default());
        } else {
            shown.push(c);
        }
    }
    shown
}

// Prints a warning and counts it
fn warn(count: &mut u32, message: &str) {
    err!("Warning: {message}");
    *count += 1;
}

fn warn_other_volume(path: &Path, warnings: &mut u32) {
    let message = format!(
        "skipping {}: another volume is mounted there",
        path.display()
    );
    warn(warnings, &message);
}

// Runs a command with stdin closed and returns what it printed; the error names the command and
// says why it failed. A command still running at the limit is stopped: a hung docker must not
// block the cleanup.
fn run(program: &str, args: &[&str], limit: Duration) -> Result<String, String> {
    let command = format!("{program} {}", args.join(" "));
    let spawned = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(format!("{program} not found")),
        Err(e) => return Err(format!("{command}: {e}")),
    };
    // On threads: a full pipe would block the command, and a child of its own that keeps a pipe
    // open must not block mclean
    let stdout = read_in_background(child.stdout.take());
    let stderr = read_in_background(child.stderr.take());
    let status = match wait_within(&mut child, limit) {
        Ok(Some(status)) => status,
        Ok(None) => {
            let seconds = limit.as_secs();
            return Err(format!("{command}: no answer after {seconds} s, stopped"));
        }
        Err(e) => return Err(format!("{command}: {e}")),
    };
    let deadline = Instant::now() + GRACE;
    let collect = |pipe: mpsc::Receiver<Vec<u8>>| {
        let mut bytes = Vec::new();
        while let Ok(chunk) = pipe.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            bytes.extend(chunk);
        }
        bytes
    };
    let (stdout, stderr) = (collect(stdout), collect(stderr));
    if status.success() {
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&stderr);
        Err(format!("{command} failed ({status}): {}", stderr.trim()))
    }
}

// What a pipe holds, chunk by chunk as it comes, until it closes
fn read_in_background(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let Some(mut pipe) = pipe else {
            return;
        };
        let mut chunk = [0; 8192];
        while let Ok(read @ 1..) = pipe.read(&mut chunk) {
            if sender.send(chunk[..read].to_vec()).is_err() {
                return;
            }
        }
    });
    receiver
}

// The exit status, or None if child was still running at the limit and was killed. It never waits
// without a bound: a process stuck in a kernel call outlives SIGKILL.
fn wait_within(child: &mut Child, limit: Duration) -> io::Result<Option<ExitStatus>> {
    const POLL: Duration = Duration::from_millis(50);
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        thread::sleep(POLL);
    }
    let _ = child.kill();
    let deadline = Instant::now() + GRACE;
    while Instant::now() < deadline && matches!(child.try_wait(), Ok(None)) {
        thread::sleep(POLL);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{PermissionsExt, symlink};

    const DAY: Duration = Duration::from_hours(24);
    const SECOND: Duration = Duration::from_secs(1);

    // A fixed cut-off in whole seconds (14 Nov 2023), so mtimes compare exactly on any filesystem
    fn cutoff() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000)
    }

    // A fresh directory under the system temp dir, unique to this process and test
    fn scratch(test: &str) -> PathBuf {
        let name = format!("mclean-test-{}-{test}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // Sets the last-modified time of path
    fn touch(path: &Path, mtime: SystemTime) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(mtime).unwrap();
    }

    // Writes len bytes to path, creating its parents, last modified at mtime
    fn put(path: &Path, len: usize, mtime: SystemTime) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x".repeat(len)).unwrap();
        touch(path, mtime);
    }

    // A symlink at `at` that points to `target`
    fn link(target: &Path, at: &Path) {
        symlink(target, at).unwrap();
    }

    fn chmod(path: &Path, mode: u32) {
        let permissions = fs::Permissions::from_mode(mode);
        fs::set_permissions(path, permissions).unwrap();
    }

    // Every path below root, sorted, to compare a tree before and after
    fn listing(root: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let mut dirs = vec![root.to_path_buf()];
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

    fn scan(root: &Path) -> Plan {
        plan_cleanup(root, cutoff())
    }

    // Allocated bytes of path and everything below it, counted the way du does (these trees have
    // no hard links unless a test makes them)
    fn du(path: &Path) -> u64 {
        let meta = fs::symlink_metadata(path).unwrap();
        let mut bytes = meta.blocks() * 512;
        if meta.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                bytes += du(&entry.unwrap().path());
            }
        }
        bytes
    }

    fn counts(tally: &Tally) -> (u32, u64) {
        (tally.count, tally.bytes)
    }

    fn parse(args: &[&str]) -> Result<Mode, String> {
        parse_args(args.iter().map(OsString::from))
    }

    struct Broken;

    impl io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("unplugged"))
        }
    }

    #[test]
    fn flags_parse_and_anything_else_is_refused() {
        assert_eq!(parse(&[]), Ok(Mode::Clean(Confirm::Ask)));
        assert_eq!(parse(&["-n"]), Ok(Mode::Clean(Confirm::DryRun)));
        assert_eq!(parse(&["--dry-run"]), Ok(Mode::Clean(Confirm::DryRun)));
        assert_eq!(parse(&["-y"]), Ok(Mode::Clean(Confirm::Yes)));
        assert_eq!(parse(&["--yes"]), Ok(Mode::Clean(Confirm::Yes)));
        // --dry-run wins over --yes; --help and --version win over both
        assert_eq!(parse(&["--yes", "-n"]), Ok(Mode::Clean(Confirm::DryRun)));
        assert_eq!(parse(&["-y", "-n", "-y"]), Ok(Mode::Clean(Confirm::DryRun)));
        assert_eq!(parse(&["-h"]), Ok(Mode::Help));
        assert_eq!(parse(&["--yes", "--help"]), Ok(Mode::Help));
        assert_eq!(parse(&["-V"]), Ok(Mode::Version));
        assert_eq!(parse(&["--version", "-n"]), Ok(Mode::Version));
        let refused = [
            "--dryrun",
            "--dry-run=1",
            "-ny",
            "-Y",
            "--Yes",
            "yes",
            ".",
            "/tmp",
            "--",
            "-",
            "",
        ];
        for bad in refused {
            let calls: [&[&str]; 3] = [&[bad], &["--dry-run", bad], &["--help", bad]];
            for args in calls {
                assert!(parse(args).is_err(), "{args:?} was accepted");
            }
        }
    }

    #[test]
    fn a_non_utf8_argument_is_refused() {
        let arg = OsString::from_vec(b"--dry-run\xff".to_vec());
        assert!(parse_args([arg]).is_err());
    }

    #[test]
    fn only_y_or_yes_proceeds() {
        let yes = ["y", "Y", "yes", "YES", "Yes", " y ", "yes\n", "\tY\r\n"];
        for answer in yes {
            assert!(is_yes(answer), "{answer:?}");
        }
        let no = ["", "\n", "n", "no", "N", "ye", "yess", "y y", "ok", "1"];
        for answer in no {
            assert!(!is_yes(answer), "{answer:?}");
        }
    }

    #[test]
    fn the_prompt_reads_one_line_and_eof_or_an_error_aborts() {
        assert!(confirm_proceed(&mut "yes\n".as_bytes()));
        assert!(confirm_proceed(&mut "Y".as_bytes()));
        assert!(!confirm_proceed(&mut "\n".as_bytes()));
        assert!(!confirm_proceed(&mut "".as_bytes()));
        assert!(!confirm_proceed(&mut "no\nyes\n".as_bytes()));
        assert!(!confirm_proceed(&mut b"\xff\n".as_slice()));
        assert!(!confirm_proceed(&mut io::BufReader::new(Broken)));
    }

    #[test]
    fn the_scan_plans_build_dirs_and_old_logs_and_removes_nothing() {
        let root = scratch("scan");
        let outside = scratch("scan-outside");
        let (old, now) = (cutoff() - DAY, SystemTime::now());
        let files = [
            ("web/package.json", 2, now),
            ("web/node_modules/a/index.js", 100, now),
            ("web/node_modules/a/package.json", 2, now),
            ("web/node_modules/a/node_modules/b.js", 50, now),
            ("web/.next/cache/page", 10, now),
            ("web/target/x", 1, now),
            ("app/Cargo.toml", 2, now),
            ("app/target/debug/app", 1000, now),
            ("app/node_modules/x.js", 1, now),
            ("app/.next/x", 1, now),
            ("bare/node_modules/x.js", 1, now),
            ("bare/.next/x", 1, now),
            ("bare/target/x", 1, now),
            ("repo/.git/sub/package.json", 1, now),
            ("repo/.git/sub/node_modules/x.js", 1, now),
            ("linked/package.json", 1, now),
            ("svc/logs/devnet/agent/agent.2023-01-01", 300, old),
            ("svc/logs/devnet/agent/agent.2026-10-31", 300, now),
            ("svc/logs/mainnet/node.2023-01-01", 7, old),
            ("svc/logs/mainnet/notes.txt", 7, old),
        ];
        for (path, len, mtime) in files {
            put(&root.join(path), len, mtime);
        }
        put(&outside.join("package.json"), 1, now);
        put(&outside.join("node_modules/x.js"), 1, now);
        // A linked node_modules next to a package.json, a linked project, and a link out of a
        // planned directory, which sizing must not follow
        let modules = outside.join("node_modules");
        link(&modules, &root.join("linked/node_modules"));
        link(&outside, &root.join("project"));
        link(&modules, &root.join("web/node_modules/a/escape"));
        let before = listing(&root);

        let plan = scan(&root);

        assert_eq!(listing(&root), before, "the scan changed the tree");
        let summary = |d: &BuildDir| (d.kind, d.path.clone(), d.size);
        let dirs: Vec<_> = plan.dirs.iter().map(summary).collect();
        let expected = [
            (
                Kind::Target,
                root.join("app/target"),
                du(&root.join("app/target")),
            ),
            (
                Kind::Next,
                root.join("web/.next"),
                du(&root.join("web/.next")),
            ),
            (
                Kind::NodeModules,
                root.join("web/node_modules"),
                du(&root.join("web/node_modules")),
            ),
        ];
        assert_eq!(dirs, expected);
        let summary = |l: &LogFolder| (l.path.clone(), l.files.len(), l.size());
        let logs: Vec<_> = plan.logs.iter().map(summary).collect();
        let devnet_old = du(&root.join("svc/logs/devnet/agent/agent.2023-01-01"));
        let mainnet_old = du(&root.join("svc/logs/mainnet/node.2023-01-01"))
            + du(&root.join("svc/logs/mainnet/notes.txt"));
        let expected = [
            (root.join("svc/logs/devnet"), 1, devnet_old),
            (root.join("svc/logs/mainnet"), 2, mainnet_old),
        ];
        assert_eq!(logs, expected);
        assert_eq!(plan.warnings, 0);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn old_logs_are_regular_files_inside_logs_network_folders() {
        let root = scratch("logs");
        let outside = scratch("logs-outside");
        let (old, now) = (cutoff() - DAY, SystemTime::now());
        // (path, last modified, planned)
        let files = [
            ("a/logs/devnet/agent/agent.2023-01-01", old, true),
            ("a/logs/devnet/agent/v2/deep.2023-01-01", old, true),
            ("a/logs/testnet/node.2023-01-01", old, true),
            ("a/logs/mainnet/notes.2023-01-01.gz", old, true),
            (
                "a/logs/mainnet/just-old.2023-01-01",
                cutoff() - SECOND,
                true,
            ),
            ("a/logs/mainnet/at-cutoff.2023-01-01", cutoff(), false),
            (
                "a/logs/mainnet/just-new.2023-01-01",
                cutoff() + SECOND,
                false,
            ),
            ("a/logs/devnet/agent/today.2026-10-31", now, false),
            ("a/logs/devnet/.DS_Store", old, true),
            ("a/logs/devnet/agent/.gitkeep", old, true),
            ("a/logs/devnet/.archive/old.2023-01-01", old, true),
            ("a/logs/devnet/.git/old.2023-01-01", old, false),
            ("a/logs/devnet/wt/.git", old, true),
            ("a/logs/devnet/agent/.github/old.2023-01-01", old, true),
            ("a/logs/devnet/agent/repo/.git/old.2023-01-01", old, false),
            ("a/logs/devnet/.fresh", now, false),
            ("a/logs/devnet/.archive/fresh", now, false),
            ("f/.hidden/logs/mainnet/old.2023-01-01", old, true),
            ("g/logs/testnet/today", now, false),
            ("a/logs/staging/old.2023-01-01", old, false),
            ("a/logs/loose.2023-01-01", old, false),
            ("a/logs/rpc/testnet/old.2023-01-01", old, false),
            ("a/devnet/old.2023-01-01", old, false),
            ("b/Logs/mainnet/old.2023-01-01", old, false),
            ("c/logs/devnet2/old.2023-01-01", old, false),
            ("e/xlogs/devnet/old.2023-01-01", old, false),
            ("e/logs-old/mainnet/old.2023-01-01", old, false),
            ("e/logs/xdevnet/old.2023-01-01", old, false),
        ];
        for (path, mtime, _) in files {
            let len = if path.ends_with(".gitkeep") { 0 } else { 10 };
            put(&root.join(path), len, mtime);
        }
        // Written today, though setting the old time moved its birth time back as well
        let active = root.join("a/logs/mainnet/active");
        put(&active, 10, old);
        touch(&active, now);
        put(&outside.join("old.2023-01-01"), 10, old);
        put(&outside.join("devnet/old.2023-01-01"), 10, old);
        fs::create_dir_all(root.join("d/logs")).unwrap();
        // A linked log file, old itself, a linked folder in a network folder, a linked network
        // folder
        let devnet = root.join("a/logs/devnet");
        let linked_log = devnet.join("linked.2023-01-01");
        link(&outside.join("old.2023-01-01"), &linked_log);
        let aged = fs::FileTimes::new().set_modified(old);
        fs::set_times_nofollow(&linked_log, aged).unwrap();
        // An old FIFO is not a regular file
        let fifo = devnet.join("old.fifo");
        let made = Command::new("mkfifo").arg(&fifo).status().unwrap();
        assert!(made.success());
        fs::set_times_nofollow(&fifo, aged).unwrap();
        link(&outside.join("devnet"), &devnet.join("agent/linked"));
        link(&outside.join("devnet"), &root.join("d/logs/devnet"));

        let plan = scan(&root);

        let folders: Vec<_> = plan.logs.iter().map(|l| l.path.clone()).collect();
        let networks = [
            "a/logs/devnet",
            "a/logs/mainnet",
            "a/logs/testnet",
            "f/.hidden/logs/mainnet",
        ];
        assert_eq!(folders, networks.map(|p| root.join(p)));
        let mut planned = Vec::new();
        for folder in &plan.logs {
            planned.extend(folder.files.iter().map(|f| f.path.clone()));
        }
        planned.sort();
        let mut expected: Vec<_> = files
            .iter()
            .filter(|f| f.2)
            .map(|f| root.join(f.0))
            .collect();
        expected.sort();
        assert_eq!(planned, expected);
        assert!(plan.dirs.is_empty());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn a_scan_finds_a_log_folder_from_at_or_above_its_logs_directory() {
        let root = scratch("start");
        put(
            &root.join("svc/logs/devnet/old.2023-01-01"),
            1,
            cutoff() - DAY,
        );
        for (start, folders) in [("svc", 1), ("svc/logs", 1), ("svc/logs/devnet", 0)] {
            assert_eq!(scan(&root.join(start)).logs.len(), folders, "{start}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_old_file_in_a_log_folder_goes_whatever_its_name() {
        let root = scratch("any-name");
        let old = cutoff() - DAY;
        let folder = root.join("svc/logs/mainnet");
        let names = [
            "agent.2023-01-01",
            "holdings-28-mar.txt",
            "agent2/agent-state-2026-02-25T22-20-17Z.json",
            "rpc/payments-rpc.tar.gz",
            "no-extension",
            ".DS_Store",
            ".archive/.hidden",
        ];
        for name in names {
            put(&folder.join(name), 5000, old);
        }
        let new = folder.join("holdings-new.txt");
        put(&new, 5000, SystemTime::now());

        let plan = scan(&root);

        assert_eq!(plan.logs[0].files.len(), names.len());
        let bytes: u64 = names.iter().map(|name| du(&folder.join(name))).sum();
        assert_eq!(plan.log_bytes(), bytes);

        let outcome = execute(&plan);

        for name in names {
            assert!(!folder.join(name).exists(), "{name} is still there");
        }
        assert!(new.exists(), "a new file was removed");
        assert!(folder.join(".archive").is_dir(), "a folder was removed");
        assert_eq!(counts(&outcome.old_logs), (names.len() as u32, bytes));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_manifest_is_a_regular_file_of_exactly_that_name() {
        let root = scratch("manifest");
        let now = SystemTime::now();
        // Each next to a build dir of its kind; none of them is a manifest
        fs::create_dir_all(root.join("dir/Cargo.toml")).unwrap();
        put(&root.join("dir/target/x"), 1, now);
        put(&root.join("real-file"), 1, now);
        fs::create_dir_all(root.join("sym")).unwrap();
        link(&root.join("real-file"), &root.join("sym/package.json"));
        put(&root.join("sym/node_modules/x.js"), 1, now);
        fs::create_dir_all(root.join("dangling")).unwrap();
        link(&root.join("no-such"), &root.join("dangling/package.json"));
        put(&root.join("dangling/.next/x"), 1, now);
        put(&root.join("lower/cargo.toml"), 1, now);
        put(&root.join("lower/target/x"), 1, now);
        put(&root.join("upper/PACKAGE.JSON"), 1, now);
        put(&root.join("upper/node_modules/x.js"), 1, now);

        let plan = scan(&root);

        let dirs: Vec<_> = plan.dirs.iter().map(|dir| dir.path.clone()).collect();
        assert!(dirs.is_empty(), "{dirs:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn another_volume_below_the_scan_is_left_alone() {
        let root = scratch("volume");
        let (old, now) = (cutoff() - DAY, SystemTime::now());
        put(&root.join("app/Cargo.toml"), 1, now);
        put(&root.join("app/target/debug/x"), 1, now);
        put(&root.join("svc/logs/devnet/sub/old.2023-01-01"), 1, old);
        // No subdirectory: only the check in the walk itself can tell these are on another device
        put(&root.join("flat/Cargo.toml"), 1, now);
        put(&root.join("flat/target/x"), 1, now);
        put(&root.join("svc/logs/testnet/old.2023-01-01"), 1, old);
        let dev = fs::metadata(&root).unwrap().dev();
        let (mut shared, mut warnings) = (SharedInodes::new(), 0);
        let owner = Owner::Dir(0);
        // A subdirectory on a device other than the one given
        let target = root.join("app/target");
        let size = dir_size(&target, dev + 1, owner, &mut shared, &mut warnings);
        assert_eq!(size, None);
        let same = dir_size(&target, dev, owner, &mut shared, &mut warnings);
        assert!(same.is_some());
        // Every directory of the scan on another device
        let mut plan = Plan {
            start: root.clone(),
            dev: dev + 1,
            ..Plan::default()
        };
        scan_directory(&root, cutoff(), &mut plan);
        assert!(plan.dirs.is_empty() && plan.logs.is_empty());
        assert!(plan.warnings > 0);
        // A subdirectory of a log folder on another device
        let (mut files, mut plan) = (
            Vec::new(),
            Plan {
                dev: dev + 1,
                ..Plan::default()
            },
        );
        find_old_logs(
            &root.join("svc/logs/devnet"),
            cutoff(),
            0,
            &mut files,
            &mut plan,
        );
        assert!(files.is_empty());
        assert_eq!(plan.warnings, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_build_dir_with_another_volume_inside_leaves_no_trace() {
        let root = scratch("volume-inside");
        let now = SystemTime::now();
        let target = root.join("app/target");
        let linked = target.join("debug/x.o");
        put(&linked, 50_000, now);
        fs::hard_link(&linked, target.join("debug/y.o")).unwrap();
        // Enough files beside the subdirectory that the walk meets some of them before it
        let beside: Vec<PathBuf> = (0..8).map(|i| target.join(format!("f{i}.o"))).collect();
        for (i, file) in beside.iter().enumerate() {
            put(file, 5000, now);
            fs::hard_link(file, target.join(format!("g{i}.o"))).unwrap();
        }
        let meta = fs::symlink_metadata(&target).unwrap();
        let dev = meta.dev();
        let mut plan = Plan {
            start: root.clone(),
            dev,
            ..Plan::default()
        };

        plan_build_dir(Kind::Target, target.clone(), &meta, dev + 1, &mut plan);

        assert!(plan.dirs.is_empty(), "it was planned");
        assert!(plan.shared.is_empty(), "its links were counted");
        assert_eq!(plan.warnings, 1);

        // The next one takes the place in the list, and its links count once
        plan_build_dir(Kind::Target, target.clone(), &meta, dev, &mut plan);
        settle_shared(&mut plan);

        assert_eq!(plan.dirs.len(), 1);
        let beside_bytes: u64 = beside.iter().map(|file| du(file)).sum();
        assert_eq!(plan.dirs[0].size, du(&target) - du(&linked) - beside_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_cutoff_is_30_days_before_the_scan() {
        let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        assert_eq!(log_cutoff(now), now - Duration::from_secs(30 * 24 * 3600));
    }

    #[test]
    fn an_unreadable_directory_is_a_warning_and_the_scan_goes_on() {
        let root = scratch("unreadable");
        let now = SystemTime::now();
        put(&root.join("locked/web/package.json"), 1, now);
        put(&root.join("locked/web/node_modules/x.js"), 1, now);
        put(&root.join("web/package.json"), 1, now);
        put(&root.join("web/node_modules/x.js"), 10, now);
        let locked = root.join("locked");
        chmod(&locked, 0o000);

        let plan = scan(&root);
        let readable = fs::read_dir(&locked).is_ok(); // true when the tests run as root

        chmod(&locked, 0o755);
        assert_eq!(plan.dirs.len(), 1 + usize::from(readable));
        assert_eq!(plan.warnings, u32::from(!readable));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_unreadable_directory_in_a_build_dir_is_a_warning() {
        let root = scratch("unreadable-inside");
        let now = SystemTime::now();
        put(&root.join("web/package.json"), 1, now);
        let locked = root.join("web/node_modules/locked");
        put(&locked.join("x.js"), 1, now);
        chmod(&locked, 0o000);

        let plan = scan(&root);
        let readable = fs::read_dir(&locked).is_ok(); // true when the tests run as root

        chmod(&locked, 0o755);
        assert_eq!(plan.dirs.len(), 1);
        assert_eq!(plan.warnings, u32::from(!readable));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn execution_removes_the_plan_and_checks_each_item_again() {
        let root = scratch("execute");
        let (old, now) = (cutoff() - DAY, SystemTime::now());
        put(&root.join("web/package.json"), 1, now);
        put(&root.join("web/node_modules/a.js"), 100, now);
        put(&root.join("app/Cargo.toml"), 1, now);
        put(&root.join("app/target/debug/app"), 1000, now);
        put(&root.join("gone/Cargo.toml"), 1, now);
        put(&root.join("gone/target/x"), 1, now);
        let agent = root.join("svc/logs/mainnet/agent");
        let log = |name: &str| agent.join(format!("{name}.2023-01-01"));
        let planned = [
            ("a", 10),
            ("b", 20),
            ("c", 30),
            ("d", 40),
            ("e", 50),
            ("g", 70),
        ];
        for (name, len) in planned {
            put(&log(name), len, old);
        }
        put(&log("new"), 5, now);
        let parent = root.join("svc/logs/mainnet/parent");
        put(&parent.join("h.2023-01-01"), 80, old);
        let plan = scan(&root);
        assert_eq!(plan.dirs.len(), 3);
        assert_eq!(plan.logs[0].files.len(), 7);
        let web = du(&root.join("web/node_modules"));
        let app = du(&root.join("app/target"));
        let logs = du(&log("a")) + du(&log("e"));

        // While the prompt waits: b is written to, c is deleted, d is replaced by a symlink, g is
        // rewritten with another old time, h's parent becomes a file, gone/target goes away, and
        // an old file and a node_modules appear that are not planned
        touch(&log("b"), now);
        fs::remove_file(log("c")).unwrap();
        fs::remove_file(log("d")).unwrap();
        link(&root.join("web/package.json"), &log("d"));
        touch(&log("g"), old - DAY);
        fs::remove_dir_all(&parent).unwrap();
        fs::write(&parent, "x").unwrap();
        fs::remove_dir_all(root.join("gone/target")).unwrap();
        put(&log("f"), 60, old);
        put(&root.join("late/package.json"), 1, now);
        put(&root.join("late/node_modules/x.js"), 1, now);

        let outcome = execute(&plan);

        let removed = [
            root.join("web/node_modules"),
            root.join("app/target"),
            log("a"),
            log("e"),
        ];
        for path in &removed {
            assert!(!path.exists(), "{} is still there", path.display());
        }
        let kept = [
            root.join("web/package.json"),
            root.join("app/Cargo.toml"),
            root.join("gone/Cargo.toml"),
            root.join("late/node_modules/x.js"),
            log("b"),
            log("f"),
            log("g"),
            log("new"),
        ];
        for path in &kept {
            assert!(path.exists(), "{} was removed", path.display());
        }
        assert!(log("d").is_symlink());
        assert!(agent.is_dir(), "log folders stay");
        assert_eq!(counts(&outcome.node_modules), (1, web));
        assert_eq!(counts(&outcome.target_dirs), (1, app));
        assert_eq!(counts(&outcome.next_dirs), (0, 0));
        assert_eq!(counts(&outcome.old_logs), (2, logs));
        // b, c, d, g, h and gone/target were skipped; none of them is a failure
        assert_eq!(outcome.skipped, 6);
        assert!(outcome.failures.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_replaced_item_or_parent_is_skipped() {
        let root = scratch("swap");
        let moved = scratch("swap-moved");
        let (old, now) = (cutoff() - DAY, SystemTime::now());
        put(&root.join("app/Cargo.toml"), 1, now);
        put(&root.join("app/target/x"), 1, now);
        put(&root.join("svc/logs/devnet/agent/old.2023-01-01"), 10, old);
        put(&root.join("re/Cargo.toml"), 1, now);
        put(&root.join("re/target/x"), 1, now);
        let log = root.join("svc/logs/testnet/new.2023-01-01");
        put(&log, 10, old);
        let plan = scan(&root);
        assert_eq!(plan.dirs.len(), 2);
        assert_eq!(plan.logs.len(), 2);

        // Parents moved away and linked back: the planned inodes, reached through a symlink
        fs::rename(root.join("app"), moved.join("app")).unwrap();
        link(&moved.join("app"), &root.join("app"));
        let agent = root.join("svc/logs/devnet/agent");
        fs::rename(&agent, moved.join("agent")).unwrap();
        link(&moved.join("agent"), &agent);
        // Items replaced by new ones: the same path, type and time, other inodes
        fs::rename(root.join("re/target"), moved.join("re-target")).unwrap();
        put(&root.join("re/target/keep"), 1, now);
        fs::rename(&log, moved.join("new-log")).unwrap();
        put(&log, 10, old);

        let outcome = execute(&plan);

        assert!(moved.join("app/target/x").exists());
        assert!(moved.join("agent/old.2023-01-01").exists());
        assert!(root.join("re/target/keep").exists());
        assert!(log.exists());
        assert_eq!(outcome.skipped, 4);
        assert_eq!(outcome.removed_bytes(), 0);
        assert!(outcome.failures.is_empty());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(moved).unwrap();
    }

    #[test]
    fn a_failed_removal_is_listed_and_the_rest_goes_on() {
        let root = scratch("failure");
        let locked = root.join("logs/devnet/locked");
        put(&locked.join("stuck.2023-01-01"), 10, cutoff() - DAY);
        // Sorts after locked, so it runs after the failure
        let free = root.join("logs/devnet/z-free.2023-01-01");
        put(&free, 20, cutoff() - DAY);
        let plan = scan(&root);
        chmod(&locked, 0o555);

        let outcome = execute(&plan);
        let stuck = locked.join("stuck.2023-01-01").exists(); // false when the tests run as root

        chmod(&locked, 0o755);
        assert!(!free.exists());
        assert_eq!(outcome.failures.len(), usize::from(stuck));
        assert!(outcome.failures.iter().all(|f| f.contains("stuck")));
        assert_eq!(outcome.old_logs.count, 2 - u32::from(stuck));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_partly_removed_build_dir_counts_what_went() {
        let root = scratch("partial");
        let now = SystemTime::now();
        put(&root.join("app/Cargo.toml"), 1, now);
        let target = root.join("app/target");
        put(&target.join("free/big"), 200_000, now);
        let locked = target.join("locked");
        put(&locked.join("stuck"), 1, now);
        let plan = scan(&root);
        let size = plan.dirs[0].size;
        chmod(&locked, 0o555);

        let outcome = execute(&plan);
        let stuck = locked.join("stuck").exists(); // false when the tests run as root

        if stuck {
            chmod(&locked, 0o755);
            assert_eq!(outcome.target_dirs.count, 0);
            assert_eq!(outcome.failures.len(), 1);
            assert!(outcome.failures[0].contains("partly removed"));
            assert!(outcome.target_dirs.bytes > 0 && outcome.target_dirs.bytes < size);
        } else {
            assert_eq!(counts(&outcome.target_dirs), (1, size));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_total_removed_adds_all_four_kinds() {
        let tally = |bytes| Tally { count: 1, bytes };
        let outcome = Outcome {
            node_modules: tally(1),
            next_dirs: tally(2),
            target_dirs: tally(4),
            old_logs: tally(8),
            ..Outcome::default()
        };
        assert_eq!(outcome.removed_bytes(), 15);
    }

    #[test]
    fn free_space_is_the_available_column_of_df() {
        let mac = "\
Filesystem   1024-blocks      Used Available Capacity  Mounted on
/dev/disk3s5   961584552 709872928 182841732    80%    /System/Volumes/Data
";
        assert_eq!(parse_df_available(mac), Some(182_841_732 * 1024));
        let spaces = "\
Filesystem    1024-blocks Used Available Capacity  Mounted on
map auto_home           0    0         0   100%    /System/Volumes/Data/home
";
        assert_eq!(parse_df_available(spaces), Some(0));
        let share = "\
Filesystem         1024-blocks Used Available Capacity Mounted on
//mike@nas/share           100   40        60      40% /Volumes/My Share
";
        assert_eq!(parse_df_available(share), Some(60 * 1024));
        assert_eq!(parse_df_available(""), None);
        assert_eq!(parse_df_available(mac.lines().next().unwrap()), None);
        assert_eq!(parse_df_available("df: .: No such file\n"), None);
    }

    #[test]
    fn the_reclaimed_size_is_read_from_both_prunes() {
        let system = "\
Deleted Images:
untagged: node:22
deleted: sha256:4f1c2d

Total reclaimed space: 1.2GB
";
        let builder = "\
ID\t\t\t\t\t\tRECLAIMABLE\tSIZE\tLAST ACCESSED
k2x9q\ttrue\t2.1kB\t2 days ago
Total:\t2.1kB
";
        assert_eq!(docker_total(system), Some("1.2GB"));
        assert_eq!(docker_total(builder), Some("2.1kB"));
        assert_eq!(docker_total("Total:\t0B\n"), Some("0B"));
        assert_eq!(docker_total("nothing to prune\n"), None);
    }

    #[test]
    fn snapshot_names_follow_the_header() {
        let listed = "\
Snapshots for disk /:
com.apple.TimeMachine.2026-10-31-140138.local
com.apple.TimeMachine.2026-10-31-150200.local
";
        let names = [
            "com.apple.TimeMachine.2026-10-31-140138.local",
            "com.apple.TimeMachine.2026-10-31-150200.local",
        ];
        assert_eq!(snapshot_names(listed), names);
        assert!(snapshot_names("Snapshots for disk /:\n").is_empty());
        assert!(snapshot_names("").is_empty());
    }

    #[test]
    fn a_dataless_snapshot_holds_no_files() {
        let names = [
            "com.apple.TimeMachine.2026-10-31-150200.local".to_string(),
            "com.apple.TimeMachine.2026-10-17-090000.local (dataless)".to_string(),
        ];
        assert_eq!(snapshots_holding_files(&names), 1);
        assert_eq!(snapshots_holding_files(&[]), 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn thinning_needs_the_startup_volume() {
        assert!(on_startup_volume(Path::new("/")));
        assert!(!on_startup_volume(Path::new("/dev")));
        assert!(!on_startup_volume(Path::new("/no/such/directory")));
    }

    #[test]
    fn sizes_print_in_binary_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1536), "1.50 KiB");
        assert_eq!(format_size(5 << 20), "5.00 MiB");
        assert_eq!(format_size(3 << 29), "1.50 GiB");
        assert_eq!(format_size(3 << 40), "3072.00 GiB");
    }

    #[test]
    fn sizes_are_allocated_blocks_and_each_hard_link_counts_once() {
        let root = scratch("sizes");
        let (old, now) = (cutoff() - DAY, SystemTime::now());
        // A log folder with no old file takes no place in the plan's list of folders
        put(&root.join("a/logs/devnet/today.2026-10-31"), 1, now);
        // A sparse 1 GiB file occupies next to nothing
        put(&root.join("sparse/Cargo.toml"), 1, now);
        let sparse = root.join("sparse/target/debug/big");
        put(&sparse, 0, now);
        fs::File::options()
            .write(true)
            .open(&sparse)
            .unwrap()
            .set_len(1 << 30)
            .unwrap();
        // Two links inside one target (deps and incremental) free their blocks once
        put(&root.join("app/Cargo.toml"), 1, now);
        let deps = root.join("app/target/debug/deps/x.o");
        put(&deps, 50_000, now);
        let incremental = root.join("app/target/debug/incremental/x.o");
        fs::create_dir_all(incremental.parent().unwrap()).unwrap();
        fs::hard_link(&deps, &incremental).unwrap();
        // A node_modules file linked to a store outside the plan frees nothing, nor does an old
        // log linked to a file outside its folder
        put(&root.join("web/package.json"), 1, now);
        let store = root.join("store/pkg.js");
        put(&store, 50_000, now);
        let module = root.join("web/node_modules/pkg.js");
        fs::create_dir_all(module.parent().unwrap()).unwrap();
        fs::hard_link(&store, &module).unwrap();
        let log = root.join("svc/logs/devnet/linked.2023-01-01");
        put(&log, 50_000, old);
        fs::hard_link(&log, root.join("svc/kept-copy")).unwrap();
        // Three links, two in one target and one outside it, free nothing
        put(&root.join("three/Cargo.toml"), 1, now);
        let three_a = root.join("three/target/a.o");
        let three_b = root.join("three/target/b.o");
        put(&three_a, 50_000, now);
        fs::hard_link(&three_a, &three_b).unwrap();
        fs::hard_link(&three_a, root.join("three-copy")).unwrap();
        // One file linked across two targets counts once, on the first
        put(&root.join("x1/Cargo.toml"), 1, now);
        put(&root.join("x2/Cargo.toml"), 1, now);
        let across = root.join("x1/target/s.o");
        put(&across, 50_000, now);
        fs::create_dir_all(root.join("x2/target")).unwrap();
        fs::hard_link(&across, root.join("x2/target/s.o")).unwrap();
        // Two old logs linked to each other count once, on the first
        let log_a = root.join("z/logs/mainnet/a.2023-01-01");
        put(&log_a, 50_000, old);
        fs::hard_link(&log_a, root.join("z/logs/mainnet/b.2023-01-01")).unwrap();

        let plan = scan(&root);

        let size = |path: &str| {
            let dir = plan.dirs.iter().find(|dir| dir.path == root.join(path));
            dir.unwrap().size
        };
        assert!(
            size("sparse/target") < 1 << 20,
            "a sparse file counts its length"
        );
        let file = du(&deps);
        let app = du(&root.join("app/target"));
        assert_eq!(
            size("app/target"),
            app - file,
            "the second link counted again"
        );
        let modules = du(&root.join("web/node_modules"));
        assert_eq!(size("web/node_modules"), modules - du(&module));
        let three = du(&root.join("three/target"));
        assert_eq!(size("three/target"), three - 2 * du(&three_a));
        assert_eq!(size("x1/target"), du(&root.join("x1/target")));
        let second = du(&root.join("x2/target"));
        assert_eq!(size("x2/target"), second - du(&across));
        let folder = |path: &str| {
            let found = plan.logs.iter().find(|l| l.path == root.join(path));
            found.unwrap()
        };
        assert_eq!(folder("svc/logs/devnet").size(), 0);
        let linked = folder("z/logs/mainnet");
        assert_eq!(linked.files[0].size, du(&log_a));
        assert_eq!(linked.files[1].size, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_this_machines_docker_sockets_count_as_local() {
        let home = Path::new("/Users/me");
        let desktop = "unix:///Users/me/.docker/run/docker.sock";
        assert_eq!(
            local_context(&format!("desktop-linux {desktop}\n"), home),
            Ok(("desktop-linux", desktop))
        );
        let system = "unix:///var/run/docker.sock";
        assert_eq!(
            local_context(&format!("default {system}"), home),
            Ok(("default", system))
        );
        let refused = [
            "prod tcp://10.0.0.5:2376",
            "c ssh://me@host",
            "c unix:///tmp/prod.sock",
            "c unix:///Users/me/.docker/cloud/engine.sock",
            "c unix:///Users/other/.docker/run/docker.sock",
            "c http://unix:///var/run/docker.sock",
            "name",
            "name ",
            "",
        ];
        for bad in refused {
            assert!(local_context(bad, home).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn docker_prunes_keep_volumes_and_name_the_builder() {
        let [system, builder] = prune_args("desktop-linux");
        let context = ["--context", "desktop-linux"];
        assert_eq!(
            system,
            [&context[..], &["system", "prune", "-a", "-f"]].concat()
        );
        let prune = ["builder", "prune", "-a", "-f", "--builder", "desktop-linux"];
        assert_eq!(builder, [&context[..], &prune].concat());
        assert!(!system.contains(&"--volumes") && !builder.contains(&"--volumes"));
    }

    #[test]
    fn a_name_cannot_draw_on_the_terminal() {
        let name = "x\u{1b}[2J\nfake\u{9b}\u{202e}\u{61c}\r\tПривет";
        assert_eq!(
            printable(format_args!("Remove: {name}")),
            "Remove: x\\u{1b}[2J\\nfake\\u{9b}\\u{202e}\\u{61c}\\r\tПривет"
        );
        assert_eq!(printable(format_args!("плain 1.2GB")), "плain 1.2GB");
    }

    #[test]
    fn an_error_keeps_its_first_line_and_a_count() {
        assert_eq!(first_line("a\nb\nc\n"), "a (and 2 more lines)");
        assert_eq!(first_line("a\n"), "a");
        assert_eq!(first_line(""), "no message");
    }

    #[test]
    fn a_command_returns_its_output_or_says_why_not() {
        let limit = Duration::from_secs(20);
        let printed = run("sh", &["-c", "echo hello"], limit);
        assert_eq!(printed, Ok("hello\n".to_string()));
        let in_parts = run("sh", &["-c", "echo first; sleep 0.2; echo second"], limit);
        assert_eq!(in_parts, Ok("first\nsecond\n".to_string()));
        let failed = run("sh", &["-c", "echo nope >&2; exit 3"], limit).unwrap_err();
        assert!(failed.contains("nope"), "{failed}");
        let missing = run("mclean-no-such-program", &[], limit);
        assert_eq!(missing, Err("mclean-no-such-program not found".to_string()));
    }

    #[test]
    fn a_stuck_command_is_stopped_at_its_limit() {
        let started = Instant::now();
        let stuck = run("sh", &["-c", "exec sleep 30"], Duration::from_millis(200));
        assert!(stuck.unwrap_err().contains("no answer"));
        // A child of its own that keeps the pipes open costs only the grace period
        let left = run(
            "sh",
            &["-c", "echo kept; sleep 30 & exit 0"],
            Duration::from_secs(20),
        );
        assert_eq!(left, Ok("kept\n".to_string()));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn the_help_states_the_log_age() {
        let age = format!("older than {LOG_MAX_AGE_DAYS} days");
        assert!(USAGE.contains(&age));
    }
}
