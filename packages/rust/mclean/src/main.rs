use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, ErrorKind, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// println! that ignores a closed stdout, so `mclean -y | head` cannot stop a cleanup halfway
macro_rules! out {
    ($($arg:tt)*) => {{
        let _ = writeln!(io::stdout(), $($arg)*);
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

const USAGE: &str = "\
Usage: mclean [-n | --dry-run] [-y | --yes]

Scans the current directory and everything below it, prints what it would remove and run,
and asks before doing it:
  node_modules and .next directories next to a package.json
  target directories next to a Cargo.toml
  files older than 30 days in logs/devnet, logs/testnet and logs/mainnet
  docker system prune -a -f, then docker builder prune -a -f on the builder of the
  current docker context (volumes are kept)
On macOS, once 1 GiB or more was removed, it thins local Time Machine snapshots (they keep
deleted files allocated) and prints the free space before and after.
Sizes are the disk space a removal frees: allocated blocks, and a hard-linked file once, only
when all its links are removed.
Symbolic links and .git directories are never entered.

Options:
  -n, --dry-run  Print the plan and exit; remove nothing (wins over --yes)
  -y, --yes      Remove without asking
  -h, --help     Print this help
  -V, --version  Print the version

Exit status: 0 done, nothing to do, or a dry run; 1 a removal failed or the answer was
not yes; 2 bad arguments.
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
}

// What the scan found to remove; scanning only reads
#[derive(Default)]
struct Plan {
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

// The plan item a shared file's bytes count for: a build dir, or a log file of a log folder
#[derive(Clone, Copy)]
enum Owner {
    Dir(usize),
    Log(usize, usize),
}

// The Docker step: the local context to prune (its builder has the same name), and
// `docker system df` for the preview
struct DockerPlan {
    context: String,
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
    // What each Docker prune reported
    docker: Vec<String>,
    // Docker and tmutil problems: reported, never failures
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
        eprintln!("Failed to remove {}: {error}", path.display());
        self.failures.push(format!("{}: {error}", path.display()));
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
            eprintln!("mclean: {e}");
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let start = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Error: cannot read the current directory: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mode = match confirm {
        Confirm::DryRun => "dry run, nothing will be removed",
        Confirm::Ask => "asks before removing",
        Confirm::Yes => "--yes, removes without asking",
    };
    out!("mclean v{VERSION} ({mode}): scanning {}", start.display());

    // Scan: read-only, builds the plan
    let macos = cfg!(target_os = "macos");
    let cutoff = log_cutoff(SystemTime::now());
    let plan = plan_cleanup(&start, cutoff);
    let docker = docker_preview();
    let snapshots = macos.then(list_snapshots);
    print_preview(&plan, &docker, snapshots.as_ref());

    if plan.dirs.is_empty() && plan.logs.is_empty() && docker.is_err() {
        out!("Nothing to clean.");
        return ExitCode::SUCCESS;
    }
    if confirm == Confirm::DryRun {
        out!("Dry run: nothing was removed.");
        return ExitCode::SUCCESS;
    }
    // Not answering yes exits 1, as apt and terraform do: the cleanup asked for did not happen
    if confirm == Confirm::Ask && !confirm_proceed(&mut io::stdin().lock()) {
        out!("Aborted: nothing was removed.");
        return ExitCode::FAILURE;
    }

    // Execute: removals, Docker, then on macOS snapshot thinning and the free space gained
    let free_before = macos.then(free_space);
    let mut outcome = execute(&plan, cutoff);
    if let Ok(docker) = &docker {
        prune_docker(&docker.context, &mut outcome);
    }
    if macos && outcome.removed_bytes() >= THIN_MIN_BYTES {
        thin_snapshots(&mut outcome);
    }
    print_outcome(&outcome);
    if let Some(before) = free_before {
        print_free_space(before, free_space());
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
    for entry in read_entries(dir, &mut plan.warnings) {
        // file_type() does not follow symlinks: a linked directory is never entered or removed
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.path();
        let file_name = entry.file_name();
        // node_modules and .next next to a package.json, target next to a Cargo.toml, and the
        // devnet, testnet and mainnet folders directly inside a logs directory
        let kind = match &*file_name.to_string_lossy() {
            "node_modules" if dir.join("package.json").exists() => Kind::NodeModules,
            ".next" if dir.join("package.json").exists() => Kind::Next,
            "target" if dir.join("Cargo.toml").exists() => Kind::Target,
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
        let owner = Owner::Dir(plan.dirs.len());
        let size = disk_bytes(&meta) + dir_size(&path, owner, &mut plan.shared);
        plan.dirs.push(BuildDir {
            kind,
            path,
            size,
            id: file_id(&meta),
        });
    }
}

// Plans the files of a logs/devnet, logs/testnet or logs/mainnet folder older than the cut-off
fn scan_log_folder(path: PathBuf, cutoff: SystemTime, plan: &mut Plan) {
    let mut files = Vec::new();
    let folder = plan.logs.len();
    let (shared, warnings) = (&mut plan.shared, &mut plan.warnings);
    find_old_logs(&path, cutoff, folder, &mut files, shared, warnings);
    if !files.is_empty() {
        plan.logs.push(LogFolder { path, files });
    }
}

// Regular files below dir, at any depth, last modified before the cut-off. Directories stay, and
// so do symlinks and hidden entries (.DS_Store, .gitkeep).
fn find_old_logs(
    dir: &Path,
    cutoff: SystemTime,
    folder: usize,
    files: &mut Vec<LogFile>,
    shared: &mut SharedInodes,
    warnings: &mut u32,
) {
    for entry in read_entries(dir, warnings) {
        if entry.file_name().as_encoded_bytes().starts_with(b".") {
            continue;
        }
        let path = entry.path();
        // DirEntry::metadata() does not follow symlinks: a link is neither a file nor a directory
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(e) => {
                warn(warnings, &format!("cannot read {}: {e}", path.display()));
                continue;
            }
        };
        if meta.is_dir() {
            find_old_logs(&path, cutoff, folder, files, shared, warnings);
        } else if meta.is_file() && is_old(&meta, cutoff) {
            let owner = Owner::Log(folder, files.len());
            files.push(LogFile {
                path,
                size: file_bytes(&meta, owner, shared),
                id: file_id(&meta),
            });
        }
    }
}

// Device and inode: the removal checks them again, because a parent directory swapped for a
// symlink after the scan makes the same path name a file outside the tree
fn file_id(meta: &fs::Metadata) -> (u64, u64) {
    (meta.dev(), meta.ino())
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

// The disk space removing dir's contents frees: allocated blocks, so a sparse file counts what
// it occupies. Symlinks are not followed, and what cannot be read counts as 0.
fn dir_size(dir: &Path, owner: Owner, shared: &mut SharedInodes) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut size = 0;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        size += if meta.is_dir() {
            disk_bytes(&meta) + dir_size(&entry.path(), owner, shared)
        } else {
            file_bytes(&meta, owner, shared)
        };
    }
    size
}

// Allocated bytes: st_blocks counts 512-byte units
fn disk_bytes(meta: &fs::Metadata) -> u64 {
    meta.blocks().saturating_mul(512)
}

// What removing one link to a file frees. With more links, nothing yet: the file is recorded in
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

// Adds each hard-linked file whose links are all in the plan, once, to the item that met it
// first. One with a link elsewhere (a pnpm node_modules linked to the store) frees nothing.
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
    let mut plan = Plan::default();
    scan_directory(start, cutoff, &mut plan);
    settle_shared(&mut plan);
    plan
}

// Read-only: the current docker context, if it is local and its daemon answers, with the
// daemon's disk usage. The error says why the Docker step is skipped.
fn docker_preview() -> Result<DockerPlan, String> {
    let context = run("docker", &CONTEXT_INSPECT)?;
    // A context name has no spaces, and the trim leaves no empty name (an empty --builder would
    // mean whichever builder `docker buildx use` picked)
    let Some((name, host)) = context.trim().split_once(' ') else {
        return Err(format!("unexpected docker context: {}", context.trim()));
    };
    if !host.starts_with("unix://") {
        return Err(format!("docker context {name} is not local ({host})"));
    }
    let usage = run("docker", &["--context", name, "system", "df"])?;
    Ok(DockerPlan {
        context: name.to_string(),
        usage,
    })
}

// macOS: the local Time Machine snapshots of the startup volume
fn list_snapshots() -> Result<Vec<String>, String> {
    let output = run("tmutil", &["listlocalsnapshots", "/"])?;
    Ok(snapshot_names(&output))
}

// The plan, printed before anything is removed
fn print_preview(
    plan: &Plan,
    docker: &Result<DockerPlan, String>,
    snapshots: Option<&Result<Vec<String>, String>>,
) {
    out!("\nPlan (nothing has been removed yet):");
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
            "Remove {} log file(s) older than {LOG_MAX_AGE_DAYS} days ({}) in {}",
            folder.files.len(),
            format_size(folder.size()),
            folder.path.display()
        );
    }
    match docker {
        Ok(docker) => {
            out!("Docker (machine-wide, not only this directory):");
            for args in prune_args(&docker.context) {
                out!("Run: docker {}", args.join(" "));
            }
            out!("Docker disk usage now (docker system df):");
            print_indented(&docker.usage);
        }
        Err(e) => eprintln!("Warning: skipping Docker: {e}"),
    }
    if let Some(snapshots) = snapshots {
        match snapshots {
            Ok(names) => out!("Time Machine: {} local snapshot(s) of /", names.len()),
            Err(e) => eprintln!("Warning: cannot list local snapshots: {e}"),
        }
        out!(
            "Then, if {} or more was removed, run: tmutil {} (deletes local snapshots)",
            format_size(THIN_MIN_BYTES),
            THIN_ARGS.join(" ")
        );
    }
    let dir_bytes: u64 = plan.dirs.iter().map(|dir| dir.size).sum();
    let log_bytes: u64 = plan.logs.iter().map(LogFolder::size).sum();
    let log_files: usize = plan.logs.iter().map(|folder| folder.files.len()).sum();
    out!(
        "To remove: {} directories ({}) and {log_files} log files ({}), {} in total",
        plan.dirs.len(),
        format_size(dir_bytes),
        format_size(log_bytes),
        format_size(dir_bytes + log_bytes)
    );
    if plan.warnings > 0 {
        out!("Scan warnings: {} (see above)", plan.warnings);
    }
}

// Asks Proceed? [y/N] and reads one line: only y or yes proceeds; an empty line, EOF or a read
// error does not
fn confirm_proceed(input: &mut impl BufRead) -> bool {
    let _ = write!(io::stdout(), "Proceed? [y/N] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(0) => {
            out!();
            false
        }
        Ok(_) => is_yes(&answer),
        Err(e) => {
            eprintln!("\nWarning: cannot read the answer: {e}");
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
fn execute(plan: &Plan, cutoff: SystemTime) -> Outcome {
    let mut outcome = Outcome::default();
    for dir in &plan.dirs {
        remove_build_dir(dir, &mut outcome);
    }
    for folder in &plan.logs {
        out!(
            "Removing {} log file(s) ({}) in {}",
            folder.files.len(),
            format_size(folder.size()),
            folder.path.display()
        );
        for file in &folder.files {
            remove_log(file, cutoff, &mut outcome);
        }
    }
    outcome
}

fn remove_build_dir(dir: &BuildDir, outcome: &mut Outcome) {
    // Still the same real directory: a symlink put in its place, or in place of a parent, is
    // left alone
    let same_dir = |meta: &fs::Metadata| meta.is_dir() && file_id(meta) == dir.id;
    if !recheck(&dir.path, same_dir, outcome) {
        return;
    }
    out!(
        "Removing {}: {} ({})",
        dir.kind.name(),
        dir.path.display(),
        format_size(dir.size)
    );
    if let Err(e) = fs::remove_dir_all(&dir.path) {
        // Gone since the check: nothing was removed, so nothing is counted
        if gone(&e) {
            outcome.skipped += 1;
            return;
        }
        eprintln!(
            "Warning: Failed to remove {}: {e}. Trying alternative method...",
            dir.path.display()
        );
        if let Err(e2) = force_remove_dir(&dir.path) {
            outcome.fail(&dir.path, e2);
            return;
        }
    }
    let tally = outcome.tally(dir.kind);
    tally.count += 1;
    tally.bytes += dir.size;
}

fn remove_log(file: &LogFile, cutoff: SystemTime, outcome: &mut Outcome) {
    // Still the same regular file, and not written to since the scan
    let still_old =
        |meta: &fs::Metadata| meta.is_file() && file_id(meta) == file.id && is_old(meta, cutoff);
    if !recheck(&file.path, still_old, outcome) {
        return;
    }
    match fs::remove_file(&file.path) {
        Ok(()) => {
            outcome.old_logs.count += 1;
            outcome.old_logs.bytes += file.size;
        }
        Err(e) if gone(&e) => outcome.skipped += 1,
        Err(e) => outcome.fail(&file.path, e),
    }
}

// The path no longer exists, or a parent directory on it was replaced by a file
fn gone(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
}

// Right before a removal: whether path still is what the scan saw (want). Something that changed
// or is gone is skipped; any other error is a failure.
fn recheck(path: &Path, want: impl Fn(&fs::Metadata) -> bool, outcome: &mut Outcome) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) if want(&meta) => true,
        Err(e) if !gone(&e) => {
            outcome.fail(path, e);
            false
        }
        _ => {
            outcome.skipped += 1;
            false
        }
    }
}

fn force_remove_dir(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("rm").arg("-rf").arg(dir).output()?;

    if !output.status.success() {
        return Err(format!(
            "rm command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    Ok(())
}

// docker system prune -a -f, then docker builder prune -a -f; never --volumes. Both name the
// context checked as local before the prompt, so a `docker context use` meanwhile changes
// nothing. The builder is named because the default is whatever `docker buildx use` picked
// last, which may be the team's shared cloud builder.
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

// Runs both prunes and keeps the size each one reports; a failure is a warning
fn prune_docker(context: &str, outcome: &mut Outcome) {
    for args in prune_args(context) {
        let command = format!("docker {}", args.join(" "));
        out!("Running: {command}");
        match run("docker", &args) {
            Ok(output) => {
                let total = docker_total(&output).unwrap_or("unknown");
                out!("Docker reclaimed: {total}");
                outcome.docker.push(format!("{command}: reclaimed {total}"));
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
// space back. Then lists the snapshots left.
fn thin_snapshots(outcome: &mut Outcome) {
    out!("Running: tmutil {}", THIN_ARGS.join(" "));
    match run("tmutil", &THIN_ARGS) {
        Ok(output) => print_indented(&output),
        Err(e) => warn(&mut outcome.warnings, &e),
    }
    match list_snapshots() {
        Ok(left) if left.is_empty() => out!("Local snapshots left: none"),
        Ok(left) => {
            out!("Local snapshots left: {}", left.len());
            for name in left {
                out!("  {name}");
            }
        }
        Err(e) => warn(&mut outcome.warnings, &e),
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

// Bytes available on the start directory's volume: df's Available column (KiB). df runs in the
// start directory, which is this process's current directory.
fn free_space() -> Result<u64, String> {
    let output = run("df", &["-kP", "."])?;
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

fn print_outcome(outcome: &Outcome) {
    if outcome.failures.is_empty() {
        out!("Cleanup completed successfully!");
    } else {
        out!("Cleanup completed with failures (listed below).");
    }
    out!("\n=== Cleanup Statistics ===");
    out!("Node.js modules removed: {}", outcome.node_modules);
    out!("Rust target directories removed: {}", outcome.target_dirs);
    out!("Next.js .next directories removed: {}", outcome.next_dirs);
    out!("Old log files removed: {}", outcome.old_logs);
    out!("Total removed: {}", format_size(outcome.removed_bytes()));
    for line in &outcome.docker {
        out!("{line}");
    }
    if outcome.skipped > 0 {
        out!("Skipped (changed since the scan): {}", outcome.skipped);
    }
    if outcome.warnings > 0 {
        out!("Warnings: {} (see above)", outcome.warnings);
    }
    if !outcome.failures.is_empty() {
        out!("Failed to remove: {}", outcome.failures.len());
        for failure in &outcome.failures {
            out!("  {failure}");
        }
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
        (Err(e), _) | (_, Err(e)) => eprintln!("Warning: cannot measure free space: {e}"),
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

// Prints a warning and counts it
fn warn(count: &mut u32, message: &str) {
    eprintln!("Warning: {message}");
    *count += 1;
}

// Runs a command with stdin closed and returns what it printed; the error names the command and
// says why it failed
fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let command = format!("{program} {}", args.join(" "));
    let result = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output();
    let output = match result {
        Ok(output) => output,
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(format!("{program} not found")),
        Err(e) => return Err(format!("{command}: {e}")),
    };
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{command} failed ({}): {}",
            output.status,
            stderr.trim()
        ))
    }
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
            ("app/Cargo.toml", 2, now),
            ("app/target/debug/app", 1000, now),
            ("bare/node_modules/x.js", 1, now),
            ("bare/target/x", 1, now),
            ("repo/.git/sub/package.json", 1, now),
            ("repo/.git/sub/node_modules/x.js", 1, now),
            ("linked/package.json", 1, now),
            ("svc/logs/devnet/agent/agent.2023-01-01", 300, old),
            ("svc/logs/devnet/agent/agent.2026-10-31", 300, now),
            ("svc/logs/mainnet/notes.txt", 7, old),
        ];
        for (path, len, mtime) in files {
            put(&root.join(path), len, mtime);
        }
        put(&outside.join("package.json"), 1, now);
        put(&outside.join("node_modules/x.js"), 1, now);
        // A linked node_modules next to a package.json, and a linked project
        let modules = outside.join("node_modules");
        link(&modules, &root.join("linked/node_modules"));
        link(&outside, &root.join("project"));
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
        let expected = [
            (root.join("svc/logs/devnet"), 1, devnet_old),
            (
                root.join("svc/logs/mainnet"),
                1,
                du(&root.join("svc/logs/mainnet/notes.txt")),
            ),
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
            ("a/logs/devnet/agent/v2/deep.log", old, true),
            ("a/logs/testnet/node.log", old, true),
            ("a/logs/mainnet/notes.txt", old, true),
            ("a/logs/mainnet/just-old", cutoff() - SECOND, true),
            ("a/logs/mainnet/at-cutoff", cutoff(), false),
            ("a/logs/mainnet/just-new", cutoff() + SECOND, false),
            ("a/logs/devnet/agent/today", now, false),
            ("a/logs/devnet/.DS_Store", old, false),
            ("a/logs/devnet/agent/.gitkeep", old, false),
            ("a/logs/devnet/.archive/old.log", old, false),
            ("a/logs/staging/old.log", old, false),
            ("a/logs/loose.log", old, false),
            ("a/logs/rpc/testnet/old.log", old, false),
            ("a/devnet/old.log", old, false),
            ("b/Logs/mainnet/old.log", old, false),
            ("c/logs/devnet2/old.log", old, false),
        ];
        for (path, mtime, _) in files {
            put(&root.join(path), 10, mtime);
        }
        put(&outside.join("old.log"), 10, old);
        put(&outside.join("devnet/old.log"), 10, old);
        fs::create_dir_all(root.join("d/logs")).unwrap();
        // A linked log file, a linked folder in a network folder, a linked network folder
        let devnet = root.join("a/logs/devnet");
        link(&outside.join("old.log"), &devnet.join("linked.log"));
        link(&outside.join("devnet"), &devnet.join("agent/linked"));
        link(&outside.join("devnet"), &root.join("d/logs/devnet"));

        let plan = scan(&root);

        let folders: Vec<_> = plan.logs.iter().map(|l| l.path.clone()).collect();
        let networks = ["a/logs/devnet", "a/logs/mainnet", "a/logs/testnet"];
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
        for (name, len) in [("a", 10), ("b", 20), ("c", 30), ("d", 40), ("e", 50)] {
            put(&agent.join(name), len, old);
        }
        put(&agent.join("new"), 5, now);
        let plan = scan(&root);
        assert_eq!(plan.dirs.len(), 3);
        assert_eq!(plan.logs[0].files.len(), 5);
        let web = du(&root.join("web/node_modules"));
        let app = du(&root.join("app/target"));
        let logs = du(&agent.join("a")) + du(&agent.join("e"));

        // While the prompt waits: b is written to, c is deleted, d is replaced by a symlink,
        // gone/target goes away, and an old file and a node_modules appear that are not planned
        touch(&agent.join("b"), now);
        fs::remove_file(agent.join("c")).unwrap();
        fs::remove_file(agent.join("d")).unwrap();
        link(&root.join("web/package.json"), &agent.join("d"));
        fs::remove_dir_all(root.join("gone/target")).unwrap();
        put(&agent.join("f"), 60, old);
        put(&root.join("late/package.json"), 1, now);
        put(&root.join("late/node_modules/x.js"), 1, now);

        let outcome = execute(&plan, cutoff());

        let removed = [
            "web/node_modules",
            "app/target",
            "svc/logs/mainnet/agent/a",
            "svc/logs/mainnet/agent/e",
        ];
        for path in removed {
            assert!(!root.join(path).exists(), "{path} is still there");
        }
        let kept = [
            "web/package.json",
            "app/Cargo.toml",
            "gone/Cargo.toml",
            "late/node_modules/x.js",
            "svc/logs/mainnet/agent/b",
            "svc/logs/mainnet/agent/f",
            "svc/logs/mainnet/agent/new",
        ];
        for path in kept {
            assert!(root.join(path).exists(), "{path} was removed");
        }
        assert!(agent.join("d").is_symlink());
        assert!(agent.is_dir(), "log folders stay");
        assert_eq!(counts(&outcome.node_modules), (1, web));
        assert_eq!(counts(&outcome.target_dirs), (1, app));
        assert_eq!(counts(&outcome.next_dirs), (0, 0));
        assert_eq!(counts(&outcome.old_logs), (2, logs));
        // b, c, d and gone/target were skipped; none of them is a failure
        assert_eq!(outcome.skipped, 4);
        assert!(outcome.failures.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_removal_is_listed_and_the_rest_goes_on() {
        let root = scratch("failure");
        let locked = root.join("logs/devnet/locked");
        put(&locked.join("stuck.log"), 10, cutoff() - DAY);
        put(&root.join("logs/devnet/free.log"), 20, cutoff() - DAY);
        let plan = scan(&root);
        chmod(&locked, 0o555);

        let outcome = execute(&plan, cutoff());
        let stuck = locked.join("stuck.log").exists(); // false when the tests run as root

        chmod(&locked, 0o755);
        assert!(!root.join("logs/devnet/free.log").exists());
        assert_eq!(outcome.failures.len(), usize::from(stuck));
        assert!(outcome.failures.iter().all(|f| f.contains("stuck.log")));
        assert_eq!(outcome.old_logs.count, 2 - u32::from(stuck));
        fs::remove_dir_all(root).unwrap();
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
        let log = root.join("svc/logs/devnet/kept.log");
        put(&log, 50_000, old);
        fs::hard_link(&log, root.join("svc/kept-copy")).unwrap();

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
        assert_eq!(plan.logs[0].size(), 0);
        fs::remove_dir_all(root).unwrap();
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
    fn the_help_states_the_log_age() {
        let age = format!("older than {LOG_MAX_AGE_DAYS} days");
        assert!(USAGE.contains(&age));
    }
}
