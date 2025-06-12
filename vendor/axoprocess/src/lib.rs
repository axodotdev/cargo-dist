#![deny(missing_docs)]

//! Nicer defaults for invoking CLI Commands.
//!
//! [`Cmd`][] is a wrapper around [`std::process::Command`] with largely the same
//! API except we want to be able to:
//!
//! * Produce nicer errors that explain what was being run (using `thiserror`/`miette`)
//! * Log every time the command is executed (defaults `tracing::info!`)
//! * Automatically check the return status's `success()` (can be opted-out per `Cmd`)
//!
//! If you like the defaults then mostly all you need to know is that `Cmd::new` takes
//! a second argument for "what should I tell the user this Command was trying to do at
//! a high level".
//!
//! This lets us turn the following logic:
//!
//! ```
//! # use std::process::Command;
//! # use tracing::info;
//! # use thiserror::Error;
//! # #[derive(Debug, Error)]
//! # #[error("{desc}")]
//! # struct MyCmdError { desc: &'static str, #[source] cause: std::io::Error }
//! # #[derive(Debug, Error)]
//! # #[error("{desc}: {status}")]
//! # struct MyStatusError { desc: &'static str, status: std::process::ExitStatus  }
//! #
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut cmd = Command::new("cargo");
//! cmd.arg("-V");
//!
//! info!("exec {:?}", cmd);
//!
//! let output = cmd.output()
//!   .map_err(|cause| MyCmdError {
//!       desc: "failed to get your cargo toolchain's version",
//!       cause
//!   })?;
//!
//! if !output.status.success() {
//!     Err(MyStatusError {
//!         desc: "failed to get your cargo toolchain's version",
//!         status: output.status
//!     })?;
//! }
//!
//! println!("version was {}", String::from_utf8_lossy(&output.stdout));
//! # Ok(())
//! # }
//! ```
//!
//!
//! Into this:
//!
//! ```
//! # use axoprocess::Cmd;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let output = Cmd::new("cargo", "get your cargo toolchain's version")
//!   .arg("-V")
//!   .output()?;
//!
//! println!("version was {}", String::from_utf8_lossy(&output.stdout));
//! # Ok(())
//! # }
//! ```
//!
//! Which is, a lot nicer!

pub use error::*;
use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, CommandArgs, CommandEnvs, ExitStatus, Stdio},
};

pub mod error;

/// A fancier Command, see the crate's top-level docs!
pub struct Cmd {
    /// The inner Command, in case you need to access it
    pub inner: Command,
    summary: String,
    log: Option<LogStrategy>,
    check_status: bool,
    #[cfg(not(feature = "stdout_to_stderr_modern"))]
    stdout_to_stderr_polyfill: bool,
}

/// Constructors
impl Cmd {
    /// Create a new Command with an additional "summary" of what this is trying to do
    pub fn new(command: impl AsRef<OsStr>, summary: impl Into<String>) -> Self {
        let inner = Command::new(command);
        Self {
            summary: summary.into(),
            inner,
            log: Some(LogStrategy::Tracing(tracing::Level::INFO)),
            check_status: true,
            #[cfg(not(feature = "stdout_to_stderr_modern"))]
            stdout_to_stderr_polyfill: false,
        }
    }
}

/// Builder APIs
impl Cmd {
    /// Pipe stdout into stderr
    ///
    /// This is useful for cases where you want your program to livestream
    /// the output of a command to give your user realtime feedback, but the command
    /// randomly writes some things to stdout, and you don't want your own stdout tainted.
    ///
    /// If the the "stdout_to_stderr_modern" feature is enabled, this will just do
    /// `command.stdout(std::io::stderr());`, which will actually let the two streams
    /// interleave and ideally do the best possible thing. In the future this will be
    /// enabled by default (once the MSRV is acceptable).
    ///
    /// Otherwise, it will use a polyfilled implementation for `output` and `status`
    /// that captures child stdout and prints it to the parent stderr at the end. If
    /// using `status`, `command.stderr(std::io::Inherit)` will be forced on, ignoring
    /// your own settings for stderr.
    pub fn stdout_to_stderr(&mut self) -> &mut Self {
        #[cfg(not(feature = "stdout_to_stderr_modern"))]
        {
            self.stdout_to_stderr_polyfill = true;
        }
        #[cfg(feature = "stdout_to_stderr_modern")]
        {
            self.inner.stdout(std::io::stderr());
        }
        self
    }

    /// Set how executions of this command should logged, accepting:
    ///
    /// * tracing: [`tracing::Level`][] (the default, set to `tracing::Level::INFO``)
    /// * stdout: [`std::io::Stdout`][]
    /// * stderr: [`std::io::Stderr`][]
    /// * not at all: `None`
    ///
    /// You can explicitly invoke the selected logging with [`Cmd::log_command`][]
    pub fn log(&mut self, strategy: impl Into<Option<LogStrategy>>) -> &mut Self {
        self.log = strategy.into();
        self
    }

    /// Set whether `Status::success` should be checked after executions
    /// (except `spawn`, which doesn't yet have a Status to check).
    ///
    /// Defaults to `true`.
    ///
    /// If true, an Err will be produced by those execution commands.
    ///
    /// Executions which produce status will pass them to [`Cmd::maybe_check_status`][],
    /// which uses this setting.
    pub fn check(&mut self, checked: bool) -> &mut Self {
        self.check_status = checked;
        self
    }
}

/// Execution APIs
impl Cmd {
    /// Equivalent to [`Cmd::status`][],
    /// but doesn't bother returning the actual status code (because it's captured in the Result)
    pub fn run(&mut self) -> Result<()> {
        self.status()?;
        Ok(())
    }
    /// Equivalent to [`std::process::Command::spawn`][],
    /// but logged and with the error wrapped.
    pub fn spawn(&mut self) -> Result<std::process::Child> {
        self.log_command();
        self.inner.spawn().map_err(|cause| AxoprocessError::Exec {
            summary: self.summary.clone(),
            cause,
        })
    }
    /// Equivalent to [`std::process::Command::output`][],
    /// but logged, with the error wrapped, and status checked (by default)
    pub fn output(&mut self) -> Result<std::process::Output> {
        #[cfg(not(feature = "stdout_to_stderr_modern"))]
        if self.stdout_to_stderr_polyfill {
            self.inner.stdout(Stdio::piped());
        }
        self.log_command();
        let res = self.inner.output().map_err(|cause| AxoprocessError::Exec {
            summary: self.summary.clone(),
            cause,
        })?;
        #[cfg(not(feature = "stdout_to_stderr_modern"))]
        if self.stdout_to_stderr_polyfill {
            use std::io::Write;
            let mut stderr = std::io::stderr().lock();
            let _ = stderr.write_all(&res.stdout);
            let _ = stderr.flush();
        }
        self.maybe_check_status(res.status)?;
        Ok(res)
    }
    /// Equivalent to [`std::process::Command::status`][]
    /// but logged, with the error wrapped, and status checked (by default)
    pub fn status(&mut self) -> Result<ExitStatus> {
        #[cfg(not(feature = "stdout_to_stderr_modern"))]
        if self.stdout_to_stderr_polyfill {
            // Emulate how status sets stderr to Inherit, simply refuse to acknowledge it being overriden
            // (if they wanted to blackhole all output, why are they using stdout_to_stderr?)
            self.inner.stderr(std::process::Stdio::inherit());
            let out = self.output()?;
            return Ok(out.status);
        }
        self.status_inner()
    }

    /// Actual impl of status, split out to support a polyfill
    fn status_inner(&mut self) -> Result<ExitStatus> {
        self.log_command();
        let res = self.inner.status().map_err(|cause| AxoprocessError::Exec {
            summary: self.summary.clone(),
            cause,
        })?;
        self.maybe_check_status(res)?;
        Ok(res)
    }
}

/// Transparently forwarded [`std::process::Command`][] APIs
impl Cmd {
    /// Forwards to [`std::process::Command::arg`][]
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    /// Forwards to [`std::process::Command::env`][]
    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.env(key, val);
        self
    }

    /// Forwards to [`std::process::Command::envs`][]
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.envs(vars);
        self
    }

    /// Forwards to [`std::process::Command::env_remove`][]
    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Self {
        self.inner.env_remove(key);
        self
    }

    /// Forwards to [`std::process::Command::env_clear`][]
    pub fn env_clear(&mut self) -> &mut Self {
        self.inner.env_clear();
        self
    }

    /// Forwards to [`std::process::Command::current_dir`][]
    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.inner.current_dir(dir);
        self
    }

    /// Forwards to [`std::process::Command::stdin`][]
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stdin(cfg);
        self
    }

    /// Forwards to [`std::process::Command::stdout`][]
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stdout(cfg);
        self
    }

    /// Forwards to [`std::process::Command::stderr`][]
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stderr(cfg);
        self
    }

    /// Forwards to [`std::process::Command::get_program`][]
    pub fn get_program(&self) -> &OsStr {
        self.inner.get_program()
    }

    /// Forwards to [`std::process::Command::get_args`][]
    pub fn get_args(&self) -> CommandArgs<'_> {
        self.inner.get_args()
    }

    /// Forwards to [`std::process::Command::get_envs`][]
    pub fn get_envs(&self) -> CommandEnvs<'_> {
        self.inner.get_envs()
    }

    /// Forwards to [`std::process::Command::get_current_dir`][]
    pub fn get_current_dir(&self) -> Option<&Path> {
        self.inner.get_current_dir()
    }
}

/// Bafflingly, tracing provides no builtin way to log with a dynamic log-level
/// (it's really really obsessed with being able to compile and cache log levels),
/// so here's a macro that does that.
macro_rules! log {
    ($lvl:expr, $fmt:expr, $($arg:tt)*) => {
        match $lvl {
            tracing::Level::TRACE => {
                tracing::trace!($fmt, $($arg)*);
            }
            tracing::Level::DEBUG => {
                tracing::debug!($fmt, $($arg)*);
            }
            tracing::Level::INFO => {
                tracing::info!($fmt, $($arg)*);
            }
            tracing::Level::WARN => {
                tracing::warn!($fmt, $($arg)*);
            }
            tracing::Level::ERROR => {
                tracing::error!($fmt, $($arg)*);
            }
        }
    }
}

/// Diagnostic APIs (used internally, but available for yourself)
impl Cmd {
    /// Check `Status::success`, producing a contextful Error if it's `false`.`
    pub fn check_status(&self, status: ExitStatus) -> Result<()> {
        if status.success() {
            Ok(())
        } else {
            Err(AxoprocessError::Status {
                summary: self.summary.clone(),
                status,
            })
        }
    }

    /// Invoke [`Cmd::check_status`][] if [`Cmd::check`][] is `true`
    /// (defaults to `true`).
    pub fn maybe_check_status(&self, status: ExitStatus) -> Result<()> {
        if self.check_status {
            self.check_status(status)?;
        }
        Ok(())
    }

    /// Log the current Command using the method specified by [`Cmd::log`][]
    /// (defaults to [`tracing::info!`][]).
    pub fn log_command(&self) {
        let Some(strategy) = self.log else {
            return;
        };
        match strategy {
            LogStrategy::Stdout => {
                println!("exec {:?}", self.inner);
            }
            LogStrategy::Stderr => {
                eprintln!("exec {:?}", self.inner);
            }
            LogStrategy::Tracing(level) => {
                log!(level, "exec {:?}", self.inner);
            }
        }
    }
}

/// How to log things
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogStrategy {
    /// To stdout
    Stdout,
    /// To stderr
    Stderr,
    /// With tracing
    Tracing(tracing::Level),
}

impl From<tracing::Level> for LogStrategy {
    fn from(level: tracing::Level) -> Self {
        Self::Tracing(level)
    }
}

impl From<std::io::Stdout> for LogStrategy {
    fn from(_stdout: std::io::Stdout) -> Self {
        Self::Stdout
    }
}

impl From<std::io::Stderr> for LogStrategy {
    fn from(_stderr: std::io::Stderr) -> Self {
        Self::Stderr
    }
}
