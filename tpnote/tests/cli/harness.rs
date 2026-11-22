//! Test harness for the `tests/cli` suite: spawns the real `tpnote` binary
//! against a throw-away directory, the way `tests/functional/run-tests` and
//! `run-tests2` do with `cargo run`.
//!
//! Why a subprocess and not an in-process call? The `tpnote` crate is a binary
//! crate whose whole runtime hangs off process-global `LazyLock` singletons --
//! `ARGS` (`Args::parse()` over the real `argv`), `CFG`, `STDIN`, `PROJECT_PATHS`
//! -- each initialized exactly once per process. There is no library target and
//! no way to feed them synthetic values, so one process per scenario is the only
//! way to vary the command line, the environment and the standard input.
//!
//! Every default here exists to stop a specific way these tests can lie:
//!
//! 1. **Standard input is `/dev/null` unless a test asks for it.** The binary
//!    slurps standard input whenever it is not a terminal (cf. `STDIN` in
//!    `src/settings.rs`). Under `cargo test` the inherited standard input is a
//!    pipe, not a terminal, so an un-redirected child would silently read test
//!    runner noise as note content. `stdin()` is the only way to opt in.
//! 2. **The environment is cleared, then rebuilt explicitly.** This replaces the
//!    `unset TPNOTE_*` preamble of the shell scripts, per test rather than once
//!    per script, and keeps a developer's own `TPNOTE_*` settings out.
//! 3. **`HOME` points inside the sandbox**, so no user-level configuration file
//!    is ever discovered, whatever `--config` does.
//! 4. **`TZ=UTC`.** Dates come from `current_local_offset()` with a UTC fallback
//!    (`tpnote_lib::filter`). That call fails in a multi-threaded process (the
//!    test runner) and succeeds in the child, so without pinning the zone the
//!    two disagree around midnight. Pinning it makes `today()` below exact.
//! 5. **Paths are canonicalized.** The binary canonicalizes `<path>`, and on
//!    macOS a temporary directory lives under `/var/folders/...`, a symlink to
//!    `/private/var/folders/...`. Comparing un-canonicalized paths would fail
//!    there and nowhere else.
//!
//! Tests are free to run in parallel: each owns a `TempDir`, and the generated
//! configuration file is built once per process and then only read.

#![allow(dead_code)] // Each test module uses a different part of this harness.

use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use tempfile::TempDir;

/// The binary under test, built by Cargo before this test runs.
const BIN: &str = env!("CARGO_BIN_EXE_tpnote");

/// Filename scheme to exercise. `Zettel` corresponds to the `TPNOTE_SCHEME`
/// export at the top of `tests/functional/run-tests2`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Scheme {
    Default,
    Zettel,
}

/// Reads one of the reference files of `tests/functional/`, which stay the
/// single source of truth while the shell scripts are still in service: the
/// port is verified by comparing against them byte for byte, unmodified.
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../../../tests/functional/",
            $name,
            "-expected-output"
        ))
    };
}

/// Today's date as `YYYYMMDD`, the sort tag the `default` scheme puts in front
/// of a new note's filename. Exact because the harness pins `TZ=UTC`; cf. the
/// module documentation.
pub fn today() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!("{:04}{:02}{:02}", now.year(), now.month() as u8, now.day())
}

/// Process-wide configuration file, derived from the binary's own
/// `--config-defaults` output exactly as the shell scripts derive theirs.
/// The `TempDir` is parked in the `OnceLock` so it outlives every test.
fn test_config() -> &'static Path {
    static CONFIG: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    let (_dir, path) = CONFIG.get_or_init(|| {
        let dir = tempfile::Builder::new()
            .prefix("tpnote-cli-config-")
            .tempdir()
            .expect("create configuration sandbox");
        let defaults = dir.path().join("tpnote.toml");

        let out = base_command()
            .arg("--config-defaults")
            .arg(&defaults)
            .output()
            .expect("run --config-defaults");
        assert!(
            out.status.success(),
            "`--config-defaults` failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let generated = fs::read_to_string(&defaults).expect("read generated configuration");
        let path = dir.path().join("tpnote-test.toml");
        fs::write(&path, transform_config(&generated)).expect("write test configuration");
        (dir, path)
    });
    path
}

/// The three edits that turn the shipped defaults into a test configuration.
/// The first two mirror `tests/functional/run-tests` line for line; the third
/// closes a hole those scripts leave open.
fn transform_config(generated: &str) -> String {
    let mut out = String::with_capacity(generated.len());
    for line in generated.lines() {
        // 1. `grep -v -e "key='fm_date'"`: drop the `date` line from every
        //    template, so a note's content does not change from one day to the
        //    next.
        if line.contains("key='fm_date'") {
            continue;
        }
        // 2. `sed -e 's/^# //g'`: the generator comments out the variables it
        //    treats as defaults; uncommenting them pins each one explicitly.
        let line = line.strip_prefix("# ").unwrap_or(line);
        // 3. Not in the shell scripts: four of their tests (022, 025, 122, 125)
        //    deliberately omit `--batch`, and the binary reads the system
        //    clipboard whenever `clipboard.read_enabled` is set and `--batch` is
        //    absent. Those tests therefore read whatever the developer happens
        //    to have copied. Switching the default off makes them hermetic.
        let line = if line.trim() == "read_enabled = true" {
            "read_enabled = false"
        } else {
            line
        };
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A `Command` with the environment cleared and only the variables every
/// invocation needs. Deliberately absent: `DISPLAY` (so the binary never
/// believes it is on a graphical desktop), and every `TPNOTE_*` variable.
fn base_command() -> Command {
    let mut cmd = Command::new(BIN);
    cmd.env_clear();
    // `PATH` has to survive: the binary may resolve helper programs through it,
    // and clearing it changes behaviour rather than isolating it.
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.env("TZ", "UTC");
    cmd.stdin(Stdio::null());
    cmd
}

/// One scenario: a sandbox directory, the files seeded into it, and the command
/// line and environment to run against it.
/// One sandbox: a throw-away note directory, the files seeded into it, the
/// configuration to use and the environment to run under. A `Tp` can be run
/// more than once -- several of the shell tests invoke the binary twice in the
/// same directory, e.g. synchronizing a note and then exporting it, or creating
/// two notes to watch the second one take the next sort tag.
pub struct Tp {
    dir: TempDir,
    root: PathBuf,
    home: PathBuf,
    scheme: Scheme,
    config: Option<PathBuf>,
    env: Vec<(OsString, OsString)>,
}

impl Tp {
    pub fn new(scheme: Scheme) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("tpnote-cli-")
            .tempdir()
            .expect("create sandbox");
        // The note directory is named `test_output`, exactly as the shell
        // scripts name theirs. This is load bearing: a new note created from a
        // directory takes its title from that directory, so the reference files
        // of tests 002, 010, 102 and 110 contain `title: test_output`. A random
        // temporary name could never match them.
        //
        // Hazard 5: resolve `/var` -> `/private/var` and friends once, here, so
        // every expected path the tests build agrees with what the binary
        // reports back.
        let root = dir.path().join("test_output");
        fs::create_dir_all(&root).expect("create note directory");
        let root = root.canonicalize().expect("canonicalize sandbox path");
        // Mark the note directory as the document root. `--export-link-rewriting`
        // resolves a rewritten link against the root, and the root is the nearest
        // ancestor holding a `tpnote.toml` (`FILENAME_ROOT_PATH_MARKER`), falling
        // back to the filesystem root when there is none -- which would turn every
        // "short" link into an absolute path containing the sandbox's random name.
        //
        // The shell scripts get this by accident: `cargo run -- -C "$PWD/tpnote.toml"`
        // writes the generated configuration into the note directory and nothing
        // ever removes it, so that leftover marks the root for their whole run.
        // (Their test 016 looks like it does this on purpose with `:> .tpnote.toml`,
        // but the marker has no leading dot, so that line has no effect.) Here the
        // marker is explicit and empty: it contributes no configuration of its own,
        // only the root.
        fs::write(root.join("tpnote.toml"), "").expect("write document root marker");

        // Hazard 3: an empty `HOME`, a sibling of the note directory rather than
        // a child of it. The `zettel` scheme derives a new note's sort tag from
        // what the note directory already holds, so nothing that is not part of
        // the scenario may sit in there.
        let home = dir.path().join("home");
        fs::create_dir_all(&home).expect("create sandbox home");

        let mut tp = Self {
            dir,
            root,
            home,
            scheme,
            config: None,
            env: Vec::new(),
        };
        tp = tp.env("LANG", "en_US.UTF-8").env("LOGNAME", "myuser");
        if scheme == Scheme::Zettel {
            // `run-tests2` exports this for its whole run.
            tp = tp.env("TPNOTE_SCHEME", "zettel");
        }
        tp
    }

    /// The sandbox's note directory, canonicalized. This is what a test passes
    /// to the binary as `<path>` when creating a new note.
    pub fn dir(&self) -> &Path {
        &self.root
    }

    /// Absolute path of a file inside the note directory.
    pub fn at(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Seeds a file into the note directory.
    pub fn file(self, name: &str, content: &str) -> Self {
        let path = self.at(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(&path, content).unwrap_or_else(|e| panic!("seed {}: {e}", path.display()));
        self
    }

    /// Seeds an empty file, the shell scripts' `:>"$INPUT_FILENAME"`. Used
    /// where a test only needs something for the binary to annotate, or a
    /// predecessor for the `zettel` scheme to count on from.
    pub fn empty_file(self, name: &str) -> Self {
        self.file(name, "")
    }

    /// Replaces the generated test configuration with another file, for the
    /// tests that need a modified policy.
    pub fn config(mut self, path: impl Into<PathBuf>) -> Self {
        self.config = Some(path.into());
        self
    }

    /// Writes a variant of the test configuration into the sandbox, applying
    /// `edit` to each line, and selects it. This is the typed form of the shell
    /// scripts' `sed 's/^exporter_... = .*/.../' "$TOML" > "$INLINE_TOML"`.
    ///
    /// The file is written beside the note directory, not inside it: a stray
    /// `.toml` in there would show up in failure listings and, under the
    /// `zettel` scheme, in the directory the next sort tag is derived from.
    pub fn config_edited(self, name: &str, edit: impl Fn(&str) -> Option<String>) -> Self {
        let source = fs::read_to_string(test_config()).expect("read test configuration");
        let mut out = String::with_capacity(source.len());
        for line in source.lines() {
            match edit(line) {
                Some(replacement) => out.push_str(&replacement),
                None => out.push_str(line),
            }
            out.push('\n');
        }
        let path = self.dir.path().join(name);
        fs::write(&path, out).expect("write edited configuration");
        self.config(path)
    }

    /// Sets an environment variable for every run, overriding any default.
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        let key = key.as_ref().to_os_string();
        self.env.retain(|(k, _)| *k != key);
        self.env.push((key, value.as_ref().to_os_string()));
        self
    }

    /// Runs the binary with `args`, standard input at `/dev/null`.
    ///
    /// `--config` is supplied automatically; nothing else is. A test spells out
    /// its own command line, because which flags are present is often exactly
    /// what is under test -- `--batch` above all, which decides whether the
    /// exporter's error policy is forced to `HardError`.
    pub fn run<I, S>(&self, args: I) -> Outcome<'_>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec(args, None)
    }

    /// Runs the binary with `args`, feeding `input` to its standard input.
    /// Hazard 1: without this, standard input is `/dev/null`, never the test
    /// runner's.
    pub fn run_stdin<I, S>(&self, args: I, input: impl AsRef<[u8]>) -> Outcome<'_>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec(args, Some(input.as_ref()))
    }

    fn exec<I, S>(&self, args: I, stdin: Option<&[u8]>) -> Outcome<'_>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = base_command();
        cmd.current_dir(&self.root);
        cmd.env("HOME", &self.home);
        for (key, value) in &self.env {
            cmd.env(key, value);
        }
        cmd.arg("--config")
            .arg(self.config.as_deref().unwrap_or(test_config()));
        cmd.args(args);

        let output = if let Some(input) = stdin {
            // `output()` pipes all three streams for us; `spawn()` does not, and
            // an inherited stdout would both escape into the test runner's own
            // output and leave `Outcome::stdout` empty.
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            let mut child = cmd.spawn().expect("spawn tpnote");
            // Dropping the handle closes the pipe, which is what tells the
            // binary the input has ended.
            let mut sink = child.stdin.take().expect("child standard input");
            sink.write_all(input).expect("write child standard input");
            drop(sink);
            child.wait_with_output().expect("wait for tpnote")
        } else {
            cmd.output().expect("run tpnote")
        };

        Outcome {
            tp: self,
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// What one run produced. Borrows its sandbox, so a test may keep running
/// against the same directory; the sandbox is removed when the `Tp` goes out of
/// scope at the end of the test.
pub struct Outcome<'a> {
    tp: &'a Tp,
    status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl<'a> Outcome<'a> {
    /// Absolute path of a file inside the note directory.
    pub fn at(&self, name: &str) -> PathBuf {
        self.tp.root.join(name)
    }

    /// Everything the note directory holds, sorted, for failure reports. The
    /// document root marker is left out: it is scaffolding, present in every
    /// sandbox, and listing it only adds noise.
    fn listing(&self) -> String {
        let mut names: Vec<String> = fs::read_dir(&self.tp.root)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n != "tpnote.toml")
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        if names.is_empty() {
            "    (empty)".to_string()
        } else {
            names
                .iter()
                .map(|n| format!("    {n}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    /// Context appended to every failure, so a red test is diagnosable without
    /// a rerun.
    fn report(&self) -> String {
        let mut s = String::new();
        let _ = write!(s, "\n  scheme: {:?}", self.tp.scheme);
        let _ = write!(s, "\n  exit status: {:?}", self.status);
        if !self.stdout.trim().is_empty() {
            let _ = write!(s, "\n  standard output:\n{}", indent(&self.stdout));
        }
        if !self.stderr.trim().is_empty() {
            let _ = write!(s, "\n  standard error:\n{}", indent(&self.stderr));
        }
        let _ = write!(s, "\n  note directory holds:\n{}", self.listing());
        s
    }

    pub fn success(self) -> Self {
        assert_eq!(
            self.status,
            Some(0),
            "expected tpnote to succeed{}",
            self.report()
        );
        self
    }

    /// Asserts the run reported failure.
    pub fn failure(self) -> Self {
        assert_ne!(
            self.status,
            Some(0),
            "expected tpnote to fail{}",
            self.report()
        );
        self
    }

    /// Asserts a sandbox file matches `expected` exactly. This is the
    /// replacement for the shell scripts' `diff`.
    pub fn file_eq(self, name: &str, expected: &str) -> Self {
        let path = self.at(name);
        let actual = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "expected the note {}, but could not read it: {e}{}",
                name,
                self.report()
            )
        });
        if actual != expected {
            panic!(
                "{} does not match the reference{}\n{}",
                name,
                self.report(),
                line_diff(expected, &actual)
            );
        }
        self
    }

    /// Asserts a sandbox file matches `expected` once every line containing
    /// `drop` is removed, mirroring the shell scripts' `grep -v -e "..."`. Used
    /// where one field of the header is not reproducible -- a sort tag derived
    /// from today's date, say.
    pub fn file_eq_without(self, name: &str, drop: &str, expected: &str) -> Self {
        let path = self.at(name);
        let actual = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "expected the note {}, but could not read it: {e}{}",
                name,
                self.report()
            )
        });
        let filtered: String = actual
            .lines()
            .filter(|l| !l.contains(drop))
            .map(|l| format!("{l}\n"))
            .collect();
        if filtered != expected {
            panic!(
                "{} does not match the reference once {:?} lines are dropped{}\n{}",
                name,
                drop,
                self.report(),
                line_diff(expected, &filtered)
            );
        }
        self
    }

    /// Asserts a sandbox file contains `needle`, the shell scripts' `grep -q`.
    pub fn file_contains(self, name: &str, needle: &str) -> Self {
        let path = self.at(name);
        let actual = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "expected the file {}, but could not read it: {e}{}",
                name,
                self.report()
            )
        });
        assert!(
            actual.contains(needle),
            "{} does not contain {:?}{}",
            name,
            needle,
            self.report()
        );
        self
    }

    /// Asserts some single line of a sandbox file contains all of `needles`.
    /// Models the shell scripts' patterned greps, e.g. `grep -q '<math
    /// [^>]*MathML'`, without pulling in a regex engine: what those patterns
    /// check is that the pieces occur together on one line, not merely
    /// somewhere in the document.
    pub fn file_line_contains_all(self, name: &str, needles: &[&str]) -> Self {
        let path = self.at(name);
        let actual = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "expected the file {}, but could not read it: {e}{}",
                name,
                self.report()
            )
        });
        let found = actual
            .lines()
            .any(|l| needles.iter().all(|n| l.contains(n)));
        assert!(
            found,
            "no single line of {} contains all of {:?}{}",
            name,
            needles,
            self.report()
        );
        self
    }

    /// Asserts a sandbox file exists.
    pub fn file_exists(self, name: &str) -> Self {
        assert!(
            self.at(name).is_file(),
            "expected {} to exist{}",
            name,
            self.report()
        );
        self
    }

    /// Asserts a sandbox file does not exist. The error-policy tests turn on
    /// this: a failed export must leave nothing behind.
    pub fn file_absent(self, name: &str) -> Self {
        assert!(
            !self.at(name).exists(),
            "expected {} not to exist{}",
            name,
            self.report()
        );
        self
    }

    /// Asserts standard error matches `expected` once the machine-specific tail
    /// is cut, mirroring the shell scripts'
    /// `sed '/^Additional technical details:/,$d'`.
    pub fn stderr_eq_trimmed(self, expected: &str) -> Self {
        let trimmed: String = self
            .stderr
            .lines()
            .take_while(|l| !l.starts_with("Additional technical details:"))
            .map(|l| format!("{l}\n"))
            .collect();
        if trimmed != expected {
            panic!(
                "standard error does not match the reference{}\n{}",
                self.report(),
                line_diff(expected, &trimmed)
            );
        }
        self
    }

    /// Asserts standard error contains `needle`.
    pub fn stderr_contains(self, needle: &str) -> Self {
        assert!(
            self.stderr.contains(needle),
            "standard error does not contain {:?}{}",
            needle,
            self.report()
        );
        self
    }

    /// The last line the binary printed, which is the path of the note it
    /// settled on.
    pub fn last_stdout_line(&self) -> &str {
        self.stdout.trim_end().lines().next_back().unwrap_or("")
    }
}

fn indent(s: &str) -> String {
    s.lines()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// First differing line, with a little context. Enough to see what moved
/// without printing two ten-kilobyte documents side by side.
fn line_diff(expected: &str, actual: &str) -> String {
    let expected: Vec<&str> = expected.lines().collect();
    let actual: Vec<&str> = actual.lines().collect();
    let mut out = String::new();
    let first = (0..expected.len().max(actual.len())).find(|&i| expected.get(i) != actual.get(i));
    match first {
        None => out.push_str("  (identical line by line; the difference is in the trailing bytes)"),
        Some(i) => {
            let _ = write!(
                out,
                "  first difference at line {} of {} expected / {} actual",
                i + 1,
                expected.len(),
                actual.len()
            );
            let from = i.saturating_sub(2);
            for (offset, line) in expected[from..i].iter().enumerate() {
                let _ = write!(out, "\n     context {:>4}: {}", from + offset + 1, line);
            }
            let _ = write!(
                out,
                "\n    expected {:>4}: {}",
                i + 1,
                expected.get(i).unwrap_or(&"(end of file)")
            );
            let _ = write!(
                out,
                "\n      actual {:>4}: {}",
                i + 1,
                actual.get(i).unwrap_or(&"(end of file)")
            );
        }
    }
    out
}
