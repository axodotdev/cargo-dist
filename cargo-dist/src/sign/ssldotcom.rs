//! Codesigning using ssl.com's CodeSignTool
use axoasset::AxoClient;
use axoasset::LocalAsset;
use axoprocess::Cmd;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use tracing::info;
use tracing::warn;

use crate::config::ProductionMode;
use crate::errors::*;
use crate::TargetTriple;

/// An instance of ssl.com's CodeSignTool
#[derive(Debug)]
pub struct CodeSignTool {
    tool: Utf8PathBuf,
    tool_dir: Utf8PathBuf,
    env: CodeSignToolEnv,
}

/// Required env var secrets for ssl.com's CodeSignTool
struct CodeSignToolEnv {
    username: String,
    password: String,
    credential_id: String,
    totp_secret: String,
}

// manual debug impl to prevent anyone adding derive(Debug) and leaking SECRETS
impl std::fmt::Debug for CodeSignToolEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeSignToolEnv")
            .field("username", &"<hidden>")
            .field("password", &"<hidden>")
            .field("credential_id", &"<hidden>")
            .field("totp_secret", &"<hidden>")
            .finish()
    }
}

impl CodeSignToolEnv {
    fn new() -> DistResult<Option<Self>> {
        if let (Some(username), Some(password), Some(credential_id), Some(totp_secret)) = (
            Self::var("SSLDOTCOM_USERNAME"),
            Self::var("SSLDOTCOM_PASSWORD"),
            Self::var("SSLDOTCOM_CREDENTIAL_ID"),
            Self::var("SSLDOTCOM_TOTP_SECRET"),
        ) {
            Ok(Some(Self {
                username,
                password,
                credential_id,
                totp_secret,
            }))
        } else {
            Ok(None)
        }
    }

    fn var(var: &str) -> Option<String> {
        let val = std::env::var(var).ok();
        if val.is_none() {
            warn!("{var} is missing");
        }
        val
    }
}

impl CodeSignTool {
    pub fn new(
        client: &AxoClient,
        host_target: &TargetTriple,
        dist_dir: &Utf8Path,
        ssldotcom_windows_sign: Option<ProductionMode>,
    ) -> DistResult<Option<Self>> {
        // Feature must be enabled
        let Some(mode) = ssldotcom_windows_sign else {
            return Ok(None);
        };
        // Must be running on x64 windows
        if host_target != axoproject::platforms::TARGET_X64_WINDOWS {
            return Ok(None);
        }

        if let Some(env) = CodeSignToolEnv::new()? {
            let tool = fetch_code_sign_tool(client, dist_dir)?;
            let tool_dir = tool
                .parent()
                .expect("CodeSignTool wasn't in a directory!?")
                .to_owned();
            configure_code_sign_tool(&tool_dir, mode)?;

            Ok(Some(CodeSignTool {
                tool,
                tool_dir,
                env,
            }))
        } else {
            warn!("skipping codesigning, required SSLDOTCOM env-vars aren't set");
            Ok(None)
        }
    }

    pub fn sign(&self, file: &Utf8Path) -> DistResult<()> {
        info!("ssl.com signing {file}");

        let CodeSignTool {
            tool,
            tool_dir,
            env,
        } = self;

        Cmd::new(tool, "sign windows artifacts")
            // CodeSignTool seems to expect that it will be invoked from its own directory,
            // so we need to set current_dir here.
            .current_dir(tool_dir)
            .arg("sign")
            .arg(format!("-input_file_path={file}"))
            .arg(format!("-username={}", &env.username))
            .arg(format!("-password={}", &env.password))
            .arg(format!("-credential_id={}", &env.credential_id))
            .arg(format!("-totp_secret={}", &env.totp_secret))
            .arg("-override=true")
            // Disable logging, we're passing several SECRETS
            .log(None)
            .stdout_to_stderr()
            .status()?;
        Ok(())
    }
}

/// Download code sign tool and prepare it for usage
fn fetch_code_sign_tool(client: &AxoClient, dist_dir: &Utf8Path) -> DistResult<Utf8PathBuf> {
    // Download links from <https://www.ssl.com/guide/esigner-codesigntool-command-guide/>
    // On windows they provide a .bat script that we're supposed to use as the primary interface.
    const WINDOWS_CMD_NAME: &str = "CodeSignTool.bat";
    const WINDOWS_URL: &str = "https://www.ssl.com/download/codesigntool-for-windows/";
    const EXTRA_FETCH_DIR: &str = "_extra_tools";
    const CODESIGNTOOL_SUBDIR: &str = "CodeSignTool";
    const ZIP_NAME: &str = "CodeSignTool.zip";

    let fetch_dir = dist_dir.join(EXTRA_FETCH_DIR);
    let zip_path = fetch_dir.join(ZIP_NAME);
    let unzipped_dir = fetch_dir.join(CODESIGNTOOL_SUBDIR);
    let cmd = unzipped_dir.join(WINDOWS_CMD_NAME);

    // Cache the download
    if cmd.exists() {
        info!("CodeSignTool already fetched");
        return Ok(cmd);
    }

    info!("fetching CodeSignTool");
    // Download and unpack the zip
    LocalAsset::create_dir_all(fetch_dir)?;
    tokio::runtime::Handle::current()
        .block_on(client.load_and_write_to_file(WINDOWS_URL, &zip_path))?;
    LocalAsset::unzip_all(&zip_path, unzipped_dir)?;

    Cmd::new(&cmd, "check tool is runnable")
        .current_dir(cmd.parent().unwrap())
        .arg("--version")
        .stdout_to_stderr()
        .run()?;
    info!("fetched CodeSignTool");

    Ok(cmd)
}

/// Configure the tool to point at the right API
///
/// This matches logic in <https://github.com/SSLcom/esigner-codesign>
/// to provide access to both the "sandbox" and "production" environments.
/// The tool seems to come out-of-the-box with the production config set,
/// but the tool nevertheless configures these settings unconditionally.
fn configure_code_sign_tool(tool_dir: &Utf8Path, mode: ProductionMode) -> DistResult<()> {
    let config = match mode {
        ProductionMode::Prod => {
            r#"
CLIENT_ID=kaXTRACNijSWsFdRKg_KAfD3fqrBlzMbWs6TwWHwAn8
OAUTH2_ENDPOINT=https://login.ssl.com/oauth2/token
CSC_API_ENDPOINT=https://cs.ssl.com
TSA_URL=http://ts.ssl.com
TSA_LEGACY_URL=http://ts.ssl.com/legacy
        "#
        }
        ProductionMode::Test => {
            r#"
CLIENT_ID=qOUeZCCzSqgA93acB3LYq6lBNjgZdiOxQc-KayC3UMw
OAUTH2_ENDPOINT=https://oauth-sandbox.ssl.com/oauth2/token
CSC_API_ENDPOINT=https://cs-try.ssl.com
TSA_URL=http://ts.ssl.com
TSA_LEGACY_URL=http://ts.ssl.com/legacy
        "#
        }
    };
    LocalAsset::write_new_all(
        config.trim(),
        tool_dir.join("conf/code_sign_tool.properties"),
    )?;
    Ok(())
}
