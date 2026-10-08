param([Parameter(Mandatory=$true)][string]$Directory, [ValidateSet('python','cpp','c')][string]$Language='python')
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
New-Item -ItemType Directory -Force -Path $Directory | Out-Null
$lock = Join-Path $Directory '.install-lock'
if (Test-Path $lock) { throw 'Tool installation is already running' }
New-Item -ItemType Directory -Path $lock | Out-Null
Set-Content -Path (Join-Path $lock 'owner') -Value $env:LEET_TOOL_INSTALL_ID
function Run-Quiet([string]$Program, [string[]]$Arguments) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Program
    $info.Arguments = ($Arguments | ForEach-Object { '"' + $_.Replace('"', '\"') + '"' }) -join ' '
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    if (!$process.Start()) { throw 'Could not launch tool setup process' }
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (!$process.WaitForExit(600000)) { $process.Kill(); throw 'Tool setup process timed out' }
    Write-Output $stdout.Result
    if ($process.ExitCode -ne 0) { throw $stderr.Result }
}
$stage = Join-Path $Directory ('.download-' + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $stage | Out-Null
try {
    $python = Join-Path $Directory 'python\python.exe'
    if (!(Test-Path $python)) {
        Write-Output 'Downloading portable Python…'
        $archive = Join-Path $stage 'python.tar.gz'
        Invoke-WebRequest -UseBasicParsing -TimeoutSec 600 -Uri 'https://github.com/astral-sh/python-build-standalone/releases/download/20261003/cpython-3.13.16%2B20261003-x86_64-pc-windows-msvc-install_only_stripped.tar.gz' -OutFile $archive
        if ((Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant() -ne 'ec43f1a85c29f147d7ae2d13218c52c70b24a983a82ab22d6c607c0593060e10') { throw 'Python checksum mismatch' }
        Run-Quiet "$env:SystemRoot\System32\tar.exe" @('-xzf', $archive, '-C', $stage)
        Run-Quiet (Join-Path $stage 'python\python.exe') @('--version')
        Move-Item (Join-Path $stage 'python') (Join-Path $Directory 'python')
    }
    Run-Quiet $python @((Join-Path $Directory 'setup-tools.py'), $Directory, $Language)
} finally {
    Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $lock -ErrorAction SilentlyContinue
}
