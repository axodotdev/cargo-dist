use std::fs::File;

use miette::{Context, IntoDiagnostic};

use crate::{ArtifactTarget, InstallerInfo};

////////////////////////////////////////////////////////////////
// Github Shell
////////////////////////////////////////////////////////////////

const GITHUB_INSTALL_SH_SCRIPT_PART1: &str = r###"#!/bin/bash
#
# Licensed under the MIT license
# <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is just a little script that can be downloaded from the internet to
# install $APP_NAME. It downloads the tarball from GitHub releases,
# extracts it and runs `???TODO???`. This means that you can pass
# arguments to this shell script and they will be passed along to the installer.

set -u
"###;

const GITHUB_INSTALL_SH_SCRIPT_PART2: &str = r###"
download_binary_and_run_installer() {
    downloader --check
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd rmdir
    need_cmd tar
    need_cmd which
    need_cmd dirname
    need_cmd awk
    need_cmd cut

    # if $VERSION isn't provided or has 0 length, use version from cargo.toml
    # ${VERSION:-} checks if version exists, and if doesn't uses the default
    # which is after the :-, which in this case is empty. -z checks for empty str
    if [ -z ${VERSION:-} ]; then
        # VERSION is either not set or empty
        DOWNLOAD_VERSION=$PACKAGE_VERSION
    else
        # VERSION set and not empty
        DOWNLOAD_VERSION=$VERSION
    fi


    get_architecture || return 1
    local _arch="$RETVAL"
    assert_nz "$_arch" "arch"

    local _ext=""
    local _zip_ext=".tar.xz"
    case "$_arch" in
        *windows*)
            _ext=".exe"
            _zip_ext=".zip"
            ;;
    esac

    local _tardir="$APP_NAME-$DOWNLOAD_VERSION-${_arch}"
    local _url="$BINARY_DOWNLOAD_PREFIX/$DOWNLOAD_VERSION/${_tardir}${_zip_ext}"
    local _dir="$(mktemp -d 2>/dev/null || ensure mktemp -d -t $APP_NAME)"
    local _file="$_dir/input.tar.gz"
    local _bin="$_dir/$APP_NAME$_ext"

    say "downloading $APP_NAME from $_url" 1>&2

    ensure mkdir -p "$_dir"
    downloader "$_url" "$_file"
    if [ $? != 0 ]; then
      say "failed to download $_url"
      say "this may be a standard network error, but it may also indicate"
      say "that $APP_NAME's release process is not working. When in doubt"
      say "please feel free to open an issue!"
      say "$REPO/issues/new/choose"
      exit 1
    fi

    ensure tar xf "$_file" --strip-components 1 -C "$_dir"

    install "$_bin" "$@"

    local _retval=$?

    ignore rm -rf "$_dir"

    return "$_retval"
}

install() {
    # TODO: REWORK THE INSTALL STEP, THIS IS A JANK HACK

    local _cargo_home_bin="$HOME/.cargo/bin/"
    local _bin="$1"
    mkdir -p "$_cargo_home_bin"
    cp "$_bin" "$_cargo_home_bin"
}

get_architecture() {
    local _ostype="$(uname -s)"
    local _cputype="$(uname -m)"

    if [ "$_ostype" = Darwin -a "$_cputype" = i386 ]; then
        # Darwin `uname -s` lies
        if sysctl hw.optional.x86_64 | grep -q ': 1'; then
            local _cputype=x86_64
        fi
    fi

    if [ "$_ostype" = Darwin -a "$_cputype" = arm64 ]; then
        # Darwin `uname -s` doesn't seem to lie on Big Sur
        # but the cputype we want is called aarch64, not arm64 (they are equivalent)
        local _cputype=aarch64
    fi

    case "$_ostype" in
        Linux)
            if has_required_glibc; then
                local _ostype=unknown-linux-gnu
            else
                local _ostype=unknown-linux-musl
                say "Downloading musl binary that does not include \`rover supergraph compose\`."
            fi
            ;;

        Darwin)
            local _ostype=apple-darwin
            ;;

        MINGW* | MSYS* | CYGWIN*)
            local _ostype=pc-windows-msvc
            ;;

        *)
            err "no precompiled binaries available for OS: $_ostype"
            ;;
    esac

    case "$_cputype" in
        # these are the only two acceptable values for cputype
        x86_64 | aarch64 )
            ;;
        *)
            err "no precompiled binaries available for CPU architecture: $_cputype"

    esac

    local _arch="$_cputype-$_ostype"

    RETVAL="$_arch"
}

say() {
    local green=`tput setaf 2 2>/dev/null || echo ''`
    local reset=`tput sgr0 2>/dev/null || echo ''`
    echo "$1"
}

err() {
    local red=`tput setaf 1 2>/dev/null || echo ''`
    local reset=`tput sgr0 2>/dev/null || echo ''`
    say "${red}ERROR${reset}: $1" >&2
    exit 1
}

has_required_glibc() {
    local _ldd_version="$(ldd --version 2>&1 | head -n1)"
    # glibc version string is inconsistent across distributions
    # instead check if the string does not contain musl (case insensitive)
    if echo "${_ldd_version}" | grep -iv musl >/dev/null; then
        local _glibc_version=$(echo "${_ldd_version}" | awk 'NR==1 { print $NF }')
        local _glibc_major_version=$(echo "${_glibc_version}" | cut -d. -f1)
        local _glibc_min_version=$(echo "${_glibc_version}" | cut -d. -f2)
        local _min_major_version=2
        local _min_minor_version=17
        if [ "${_glibc_major_version}" -gt "${_min_major_version}" ] \
            || { [ "${_glibc_major_version}" -eq "${_min_major_version}" ] \
            && [ "${_glibc_min_version}" -ge "${_min_minor_version}" ]; }; then
            return 0
        else
            say "This operating system needs glibc >= ${_min_major_version}.${_min_minor_version}, but only has ${_libc_version} installed."
        fi
    else
        say "This operating system does not support dynamic linking to glibc."
    fi

    return 1
}

need_cmd() {
    if ! check_cmd "$1"
    then err "need '$1' (command not found)"
    fi
}

check_cmd() {
    command -v "$1" > /dev/null 2>&1
    return $?
}

need_ok() {
    if [ $? != 0 ]; then err "$1"; fi
}

assert_nz() {
    if [ -z "$1" ]; then err "assert_nz $2"; fi
}

# Run a command that should never fail. If the command fails execution
# will immediately terminate with an error showing the failing
# command.
ensure() {
    "$@"
    need_ok "command failed: $*"
}

# This is just for indicating that commands' results are being
# intentionally ignored. Usually, because it's being executed
# as part of error handling.
ignore() {
    "$@"
}

# This wraps curl or wget. Try curl first, if not installed,
# use wget instead.
downloader() {
    if check_cmd curl
    then _dld=curl
    elif check_cmd wget
    then _dld=wget
    else _dld='curl or wget' # to be used in error message of need_cmd
    fi

    if [ "$1" = --check ]
    then need_cmd "$_dld"
    elif [ "$_dld" = curl ]
    then curl -sSfL "$1" -o "$2"
    elif [ "$_dld" = wget ]
    then wget "$1" -O "$2"
    else err "Unknown downloader"   # should not reach here
    fi
}

download_binary_and_run_installer "$@" || exit 1
"###;

pub(crate) fn generate_github_install_sh_script(
    artifact: &ArtifactTarget,
    info: &InstallerInfo,
) -> Result<(), miette::Report> {
    let installer_file = &artifact.file_path;
    let mut file = File::create(installer_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {installer_file}"))?;
    write_github_install_sh_script(&mut file, info)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write to installer file {installer_file}"))?;
    Ok(())
}

fn write_github_install_sh_script(
    f: &mut File,
    info: &InstallerInfo,
) -> Result<(), std::io::Error> {
    use std::io::Write;
    let InstallerInfo {
        app_name,
        app_version,
        repo_url,
    } = info;

    writeln!(f, "{GITHUB_INSTALL_SH_SCRIPT_PART1}")?;

    writeln!(f, "REPO=\"{repo_url}\"")?;
    writeln!(f, "APP_NAME=\"{app_name}\"")?;
    writeln!(f, "BINARY_DOWNLOAD_PREFIX=\"$REPO/releases/download\"")?;
    writeln!(f, "PACKAGE_VERSION=\"{app_version}\"")?;

    writeln!(f, "{GITHUB_INSTALL_SH_SCRIPT_PART2}")?;

    Ok(())
}

////////////////////////////////////////////////////////////////
// Github Powerhsell
////////////////////////////////////////////////////////////////

const GITHUB_INSTALL_PS_SCRIPT_PART1: &str = r###"# Licensed under the MIT license
# <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is just a little script that can be downloaded from the internet to
# install an app. It downloads the tarball from GitHub releases,
# extracts it and runs `??? TODO ???`. This means that you can pass
# arguments to this shell script and they will be passed along to the installer.
"###;

const GITHUB_INSTALL_PS_SCRIPT_PART2: &str = r###"
function Install-Binary($install_args) {
  $old_erroractionpreference = $ErrorActionPreference
  $ErrorActionPreference = 'stop'

  Initialize-Environment

  # If the VERSION env var is set, we use it instead
  # of the version defined in the cargo.toml
  $download_version = if (Test-Path env:VERSION) {
    $Env:VERSION
  } else {
    $package_version
  }

  $exe = Download($download_version)
  Invoke-Installer "$exe" "$install_args"

  $ErrorActionPreference = $old_erroractionpreference
}

function Download($version) {
  $url = "$repo/releases/download/$version/$app_name-$version-x86_64-pc-windows-msvc.zip"
  "Downloading $app_name from $url" | Out-Host
  $tmp = New-Temp-Dir
  $dir_path = "$tmp\$app_name.zip"
  $wc = New-Object Net.Webclient
  $wc.downloadFile($url, $dir_path)
  Expand-Archive -Path $dir_path -DestinationPath "$tmp"
  return "$tmp\$app_name.exe"
}

function Invoke-Installer($tmp, $install_args) {
  $bin_dir = New-Item -Force -ItemType Directory -Path (Join-Path $HOME ".cargo\bin")
  Copy-Item "$exe" -Destination "$bin_dir"
  Remove-Item "$tmp" -Recurse -Force
}

function Initialize-Environment() {
  If (($PSVersionTable.PSVersion.Major) -lt 5) {
    Write-Error "PowerShell 5 or later is required to install $app_name."
    Write-Error "Upgrade PowerShell: https://docs.microsoft.com/en-us/powershell/scripting/setup/installing-windows-powershell"
    break
  }

  # show notification to change execution policy:
  $allowedExecutionPolicy = @('Unrestricted', 'RemoteSigned', 'ByPass')
  If ((Get-ExecutionPolicy).ToString() -notin $allowedExecutionPolicy) {
    Write-Error "PowerShell requires an execution policy in [$($allowedExecutionPolicy -join ", ")] to run $app_name."
    Write-Error "For example, to set the execution policy to 'RemoteSigned' please run :"
    Write-Error "'Set-ExecutionPolicy RemoteSigned -scope CurrentUser'"
    break
  }

  # GitHub requires TLS 1.2
  If ([System.Enum]::GetNames([System.Net.SecurityProtocolType]) -notcontains 'Tls12') {
    Write-Error "Installing $app_name requires at least .NET Framework 4.5"
    Write-Error "Please download and install it first:"
    Write-Error "https://www.microsoft.com/net/download"
    break
  }
}

function New-Temp-Dir() {
  [CmdletBinding(SupportsShouldProcess)]
  param()
  $parent = [System.IO.Path]::GetTempPath()
  [string] $name = [System.Guid]::NewGuid()
  New-Item -ItemType Directory -Path (Join-Path $parent $name)
}

Install-Binary "$Args"
"###;

pub(crate) fn generate_github_install_ps_script(
    artifact: &ArtifactTarget,
    info: &InstallerInfo,
) -> Result<(), miette::Report> {
    let installer_file = &artifact.file_path;
    let mut file = File::create(installer_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {installer_file}"))?;
    write_github_install_ps_script(&mut file, info)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write to installer file {installer_file}"))?;
    Ok(())
}

fn write_github_install_ps_script(
    f: &mut File,
    info: &InstallerInfo,
) -> Result<(), std::io::Error> {
    use std::io::Write;
    let InstallerInfo {
        app_name,
        app_version,
        repo_url,
    } = info;

    writeln!(f, "{GITHUB_INSTALL_PS_SCRIPT_PART1}")?;

    writeln!(f, "$repo = '{repo_url}'")?;
    writeln!(f, "$app_name = '{app_name}'")?;
    writeln!(f, "$package_version = '{app_version}'")?;

    writeln!(f, "{GITHUB_INSTALL_PS_SCRIPT_PART2}")?;

    Ok(())
}
