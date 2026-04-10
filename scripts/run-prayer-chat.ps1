#!/usr/bin/env pwsh
# NOTE: Untested on Windows. Please report any issues so we can tighten this up.

[CmdletBinding()]
param(
  [ValidateSet("google", "openai")]
  [string]$Provider = "google",
  [string]$Model = "gemma-4-31b-it",
  [string]$LlmBaseUrl = "https://api.openai.com/v1",
  [string]$ApiKey = "",
  [string]$ApiBind = "127.0.0.1:7777",
  [string]$McpBind = "127.0.0.1:5000"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Info {
  param([string]$Message)
  Write-Host "[run-prayer-chat.ps1] $Message"
}

function Parse-Bind {
  param([string]$Bind)
  $parts = $Bind.Split(":")
  if ($parts.Count -ne 2) {
    throw "Invalid bind '$Bind'. Expected host:port."
  }
  return @{ Host = $parts[0]; Port = [int]$parts[1] }
}

function Wait-ForTcp {
  param(
    [string]$Host,
    [int]$Port,
    [int]$TimeoutSeconds = 20
  )

  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    $client = New-Object System.Net.Sockets.TcpClient
    try {
      $result = $client.BeginConnect($Host, $Port, $null, $null)
      if ($result.AsyncWaitHandle.WaitOne(250)) {
        $client.EndConnect($result)
        return
      }
    } catch {
      # Keep polling until timeout.
    } finally {
      $client.Dispose()
    }
    Start-Sleep -Milliseconds 100
  }

  throw "Timed out waiting for $Host`:$Port"
}

if (-not $ApiKey) {
  if ($Provider -eq "google") {
    $ApiKey = $env:GEMINI_API_KEY
  } else {
    $ApiKey = $env:OPENAI_API_KEY
  }
}

if (-not $ApiKey) {
  if ($Provider -eq "google") {
    throw "Missing API key. Set GEMINI_API_KEY or pass -ApiKey."
  }
  throw "Missing API key. Set OPENAI_API_KEY or pass -ApiKey."
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "cargo not found in PATH."
}

$rootDir = Split-Path -Parent $PSScriptRoot
$apiProcess = $null
$mcpProcess = $null

try {
  Set-Location $rootDir
  $env:PRAYER_RS_BIND = $ApiBind

  $apiEndpoint = Parse-Bind $ApiBind
  $mcpEndpoint = Parse-Bind $McpBind

  Write-Info "Starting prayer-api on $ApiBind..."
  $apiProcess = Start-Process -FilePath "cargo" -ArgumentList @("run", "-q", "-p", "prayer-api") -PassThru
  Wait-ForTcp -Host $apiEndpoint.Host -Port $apiEndpoint.Port -TimeoutSeconds 20

  Write-Info "Starting prayer-mcp on $McpBind..."
  $mcpProcess = Start-Process -FilePath "cargo" -ArgumentList @(
    "run", "-q", "-p", "prayer-mcp", "--",
    "--prayer-url", "http://$ApiBind",
    "--transport", "sse",
    "--bind", $McpBind
  ) -PassThru
  Wait-ForTcp -Host $mcpEndpoint.Host -Port $mcpEndpoint.Port -TimeoutSeconds 20

  Write-Info "Launching prayer-mcp-client chat..."
  $clientArgs = @(
    "run", "-q", "-p", "prayer-mcp-client", "--", "chat",
    "--provider", $Provider,
    "--model", $Model,
    "--api-key", $ApiKey,
    "--mcp-url", "http://$McpBind/mcp"
  )
  if ($Provider -eq "openai") {
    $clientArgs += @("--llm-base-url", $LlmBaseUrl)
  }

  & cargo @clientArgs
} finally {
  if ($mcpProcess -and -not $mcpProcess.HasExited) {
    Stop-Process -Id $mcpProcess.Id -Force -ErrorAction SilentlyContinue
  }
  if ($apiProcess -and -not $apiProcess.HasExited) {
    Stop-Process -Id $apiProcess.Id -Force -ErrorAction SilentlyContinue
  }
}
