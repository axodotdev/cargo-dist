# Licensed under the MIT license
# <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is just a little script that can be downloaded from the internet to
# install an app. It downloads the tarball from GitHub releases,
# extracts it and runs `??? TODO ???`. This means that you can pass
# arguments to this shell script and they will be passed along to the installer.

param (
    [Parameter(HelpMessage = 'The GitHub repo from which to download the program.')]
    [string]$repo = '{{REPO}}',
    [Parameter(HelpMessage = 'Which app to download from the GitHub release.')]
    [string]$app_name = '{{APP_NAME}}',
    [Parameter(HelpMessage = 'Which version of the app to download.')]
    [string]$package_version = '{{PACKAGE_VERSION}}'
)

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
