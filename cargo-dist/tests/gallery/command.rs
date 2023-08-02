use miette::{miette, Context, IntoDiagnostic};
use std::process::Command;

pub struct CommandInfo {
    name: String,
    cmd: String,
    args: Vec<String>,
    version: Option<String>,
}

impl CommandInfo {
    /// Create a new command, checking that it works by running it with `--version`
    pub fn new(name: &str, path: Option<&str>) -> Option<Self> {
        let cmd = path.unwrap_or(name).to_owned();
        let output = Command::new(&cmd).arg("--version").output().ok()?;

        Some(CommandInfo {
            name: name.to_owned(),
            cmd,
            args: vec![],
            version: parse_version(output),
        })
    }

    /// Create a new command, don't check that it works
    #[allow(dead_code)]
    pub fn new_unchecked(name: &str, path: Option<&str>) -> Self {
        let cmd = path.unwrap_or(name).to_owned();

        CommandInfo {
            name: name.to_owned(),
            cmd,
            args: vec![],
            version: None,
        }
    }

    /// Create a new powershell command (for running things like powershell modules)
    pub fn new_powershell_command(name: &str) -> Option<Self> {
        let output = Command::new("powershell")
            .arg("-Command")
            .arg("Get-Command")
            .arg(name)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(CommandInfo {
            name: name.to_owned(),
            cmd: "powershell".to_owned(),
            args: vec!["-Command".to_owned(), name.to_owned()],
            version: parse_version(output),
        })
    }

    /// Run with `.output` and check for errors/status
    pub fn output_checked(
        &self,
        builder: impl FnOnce(&mut Command) -> &mut Command,
    ) -> Result<std::process::Output, miette::Report> {
        let mut command = Command::new(&self.cmd);
        command.args(&self.args);
        builder(&mut command);
        let output = command
            .output()
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to run \"{}\"", pretty_cmd(&self.name, &command)))?;
        if output.status.success() {
            Ok(output)
        } else {
            let mut out = String::new();
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            out.push_str("\nstdout:\n");
            out.push_str(&stdout);
            out.push_str("\nstderr:\n");
            out.push_str(&stderr);
            Err(miette!("{out}")).wrap_err_with(|| {
                format!(
                    "\"{}\" failed ({})",
                    pretty_cmd(&self.name, &command),
                    output.status
                )
            })
        }
    }

    /// Run with `.output` and only check for errors, DON'T check status
    pub fn output(
        &self,
        builder: impl FnOnce(&mut Command) -> &mut Command,
    ) -> Result<std::process::Output, miette::Report> {
        let mut command = Command::new(&self.cmd);
        command.args(&self.args);
        builder(&mut command);
        let output = command
            .output()
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to run \"{}\"", pretty_cmd(&self.name, &command)))?;
        Ok(output)
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

/// Parse out the version from `--version` assuming the standard `app-name 0.1.0` format
fn parse_version(output: std::process::Output) -> Option<String> {
    let version_bytes = output.stdout;
    let version_full = String::from_utf8(version_bytes).ok()?;
    let version_line = version_full.lines().next()?;
    let version_suffix = version_line.split_once(' ')?.1.trim().to_owned();
    Some(version_suffix)
}

/// Pretty print a command invocation
fn pretty_cmd(name: &str, cmd: &Command) -> String {
    let mut out = String::new();
    out.push_str(name);
    for arg in cmd.get_args() {
        out.push(' ');
        out.push_str(&arg.to_string_lossy())
    }
    out
}
