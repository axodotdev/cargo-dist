# Licensed under the MIT license
# <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is just a little script that can be downloaded from the internet to
# install an app. It downloads the tarball from GitHub releases,
# and extracts it to ~/.cargo/bin/
#
# In the future this script will gain extra features, but for now it's
# intentionally very simplistic to avoid shipping broken things.

param (
    [Parameter(HelpMessage = 'The name of the App')]
    [string]$app_name = '{{APP_NAME}}',
    [Parameter(HelpMessage = 'The version of the App')]
    [string]$app_version = '{{APP_VERSION}}',
    [Parameter(HelpMessage = 'The base name to use for artifacts to fetch')]
    [string]$artifact_base_name = '{{ARTIFACT_BASE_NAME}}',
    [Parameter(HelpMessage = 'The URL of the directory where artifacts can be fetched from')]
    [string]$artifact_download_url = '{{ARTIFACT_DOWNLOAD_URL}}'
)

function Install-Binary($install_args) {
  $old_erroractionpreference = $ErrorActionPreference
  $ErrorActionPreference = 'stop'

  Initialize-Environment

  $exe = Download "$artifact_download_url" "$artifact_base_name"
  Invoke-Installer "$exe" "$install_args"

  $ErrorActionPreference = $old_erroractionpreference
}

function Download($download_url, $base_name) {
  $arch = "x86_64-pc-windows-msvc"
  $zip_ext = ".zip"
  $exe_ext = ".exe"
  $url = "$download_url/$base_name-$arch$zip_ext"
  "Downloading $app_name $app_version from $url" | Out-Host
  $tmp = New-Temp-Dir
  $dir_path = "$tmp\$app_name$zip_ext"
  $wc = New-Object Net.Webclient
  $wc.downloadFile($url, $dir_path)
  Expand-Archive -Path $dir_path -DestinationPath "$tmp"

  # TODO: take a list of binaries each zip will contain so we know what to extract
  return "$tmp\$app_name$exe_ext"
}

function Invoke-Installer($tmp, $install_args) {
  # FIXME: respect $CARGO_HOME if set
  $bin_dir = New-Item -Force -ItemType Directory -Path (Join-Path $HOME ".cargo\bin")
  Copy-Item "$exe" -Destination "$bin_dir"
  Remove-Item "$tmp" -Recurse -Force

  "Installed $app_name $app_version to $bin_dir" | Out-Host
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
