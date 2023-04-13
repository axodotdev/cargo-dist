# Licensed under the MIT license
# <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is just a little script that can be downloaded from the internet to
# install an app. It downloads the tarball from artifact_download_url,
# and extracts it to ~/.cargo/bin/
#
# In the future this script will gain extra features, but for now it's
# intentionally very simplistic to avoid shipping broken things.

param (
    [Parameter(HelpMessage = 'The name of the App')]
    [string]$app_name = '{{APP_NAME}}',
    [Parameter(HelpMessage = 'The version of the App')]
    [string]$app_version = '{{APP_VERSION}}',
    [Parameter(HelpMessage = 'The URL of the directory where artifacts can be fetched from')]
    [string]$artifact_download_url = '{{ARTIFACT_DOWNLOAD_URL}}'
)

function Install-Binary($install_args) {
  $old_erroractionpreference = $ErrorActionPreference
  $ErrorActionPreference = 'stop'

  Initialize-Environment

  # Platform info injected by cargo-dist
  $platforms = {{PLATFORM_INFO}}

  $fetched = Download "$artifact_download_url" $platforms
  # FIXME: add a flag that lets the user not do this step
  Invoke-Installer $fetched "$install_args"

  $ErrorActionPreference = $old_erroractionpreference
}

function Get-TargetTriple() {
  try {
    # NOTE: this might return X64 on ARM64 Windows, which is OK since emulation is available.
    # It works correctly starting in PowerShell Core 7.3 and Windows PowerShell in Win 11 22H2.
    # Ideally this would just be
    #   [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    # but that gets a type from the wrong assembly on Windows PowerShell (i.e. not Core)
    $a = [System.Reflection.Assembly]::LoadWithPartialName("System.Runtime.InteropServices.RuntimeInformation")
    $t = $a.GetType("System.Runtime.InteropServices.RuntimeInformation")
    $p = $t.GetProperty("OSArchitecture")
    # Possible OSArchitecture Values: https://learn.microsoft.com/dotnet/api/system.runtime.interopservices.architecture
    # Rust supported platforms: https://doc.rust-lang.org/stable/rustc/platform-support.html
    switch ($p.GetValue($null).ToString())
    {
      "X86" { return "i686-pc-windows-msvc" }
      "X64" { return "x86_64-pc-windows-msvc" }
      "Arm" { return "thumbv7a-pc-windows-msvc" }
      "Arm64" { return "aarch64-pc-windows-msvc" }
    }
  } catch {
    # The above was added in .NET 4.7.1, so Windows PowerShell in versions of Windows
    # prior to Windows 10 v1709 may not have this API.
    Write-Verbose "Get-TargetTriple: Exception when trying to determine OS architecture."
    Write-Verbose $_
  }

  # This is available in .NET 4.0. We already checked for PS 5, which requires .NET 4.5.
  Write-Verbose("Get-TargetTriple: falling back to Is64BitOperatingSystem.")
  if ([System.Environment]::Is64BitOperatingSystem) {
    return "x86_64-pc-windows-msvc"
  } else {
    return "i686-pc-windows-msvc"
  }
}

function Download($download_url, $platforms) {
  # FIXME: make this something we lookup based on the current machine
  $arch = Get-TargetTriple

  if (-not $platforms.ContainsKey($arch)) {
    # X64 is well-supported, including in emulation on ARM64
    Write-Verbose "$arch is not availablem falling back to X64"
    $arch = "x86_64-pc-windows-msvc"
  }

  if (-not $platforms.ContainsKey($arch)) {
    # should not be possible, as currently we always produce X64 binaries.
    $platforms_json = ConvertTo-Json $platforms
    throw "ERROR: could not find binaries for this platform. Last platform tried: $arch platform info: $platforms_json"
  }

  # Lookup what we expect this platform to look like
  $info = $platforms[$arch]
  $zip_ext = $info["zip_ext"]
  $bin_names = $info["bins"]
  $artifact_name = $info["artifact_name"]

  # Make a new temp dir to unpack things to
  $tmp = New-Temp-Dir
  $dir_path = "$tmp\$app_name$zip_ext"

  # Download and unpack!
  $url = "$download_url/$artifact_name"
  "Downloading $app_name $app_version $arch" | Out-Host
  "  from $url" | Out-Host
  "  to $dir_path" | Out-Host
  $wc = New-Object Net.Webclient
  $wc.downloadFile($url, $dir_path)

  "Unpacking to $tmp" | Out-Host

  # Select the tool to unpack the files with.
  #
  # Expand-Archive defaults to removing the root dir (convenient!), so we pass
  # --strip-components 1 to tar to make them behave the same.
  #
  # As of windows 10(?), powershell comes with tar preinstalled, but in practice
  # it only seems to support .tar.gz, and not xz/zstd. Still, we should try to
  # forward all tars to it in case the user has a machine that can handle it!
  switch -Wildcard ($zip_ext) {
    ".zip" {
      Expand-Archive -Path $dir_path -DestinationPath "$tmp";
      Break
    }
    ".tar.*" {
      tar xf $dir_path --strip-components 1 -C "$tmp";
      Break
    }
    Default {
      throw "ERROR: unknown archive format $zip_ext"
    }
  }

  # Let the next step know what to copy
  $bin_paths = @()
  foreach ($bin_name in $bin_names) {
    "  Unpacked $bin_name" | Out-Host
    $bin_paths += "$tmp\$bin_name"
  }
  return $bin_paths
}

function Invoke-Installer($bin_paths) {
  # FIXME: respect $CARGO_HOME if set
  # FIXME: add a flag that lets the user pick this dir
  # FIXME: try to detect other "nice" dirs on the user's PATH?
  # FIXME: detect if the selected install dir exists or is on PATH?
  $dest_dir = New-Item -Force -ItemType Directory -Path (Join-Path $HOME ".cargo\bin")

  "Installing to $dest_dir" | Out-Host
  # Just copy the binaries from the temp location to the install dir
  foreach ($bin_path in $bin_paths) {
    Copy-Item "$bin_path" -Destination "$dest_dir"
    Remove-Item "$bin_path" -Recurse -Force
  }

  "Everything's installed!" | Out-Host
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
