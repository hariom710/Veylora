<#
.SYNOPSIS
  Packaged-app smoke test for Veylora (WebView2 CDP automation).

.DESCRIPTION
  Wipes app state for a true first-run, launches the installed exe, and
  drives the UI over the Chrome DevTools Protocol:
    login -> create vault -> recovery -> import -> image viewer ->
    vault:// fetch -> lock -> PIN unlock -> dashboard counts.
  Exits 0 only when every assertion passes; 1 otherwise.

  Requires: the app installed (default path below) and the test image is
  generated automatically into the shot directory.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke.ps1
#>
param(
  [string]$ExePath = "$env:LOCALAPPDATA\Veylora\veylora.exe",
  [string]$VaultName = 'FinalTest',
  [string]$VaultPath = "$env:USERPROFILE\Documents\FinalTest",
  [string]$Pin = '1234',
  [string]$Password = 'testpass123',
  # Seeded first-run admin account (see src-tauri/src/app_auth.rs).
  [string]$AdminEmail = 'b1865741@gmail.com',
  [string]$AdminPassword = 'Harry@12345',
  [int]$Port = 9444,
  [string]$ShotDir = (Join-Path $env:TEMP 'veylora-smoke')
)

# No Set-StrictMode: CDP responses legitimately lack keys (error, exceptionDetails).
$ErrorActionPreference = 'Stop'
$script:failures = New-Object System.Collections.Generic.List[string]
function Check([string]$name, [bool]$cond) {
  if ($cond) { Write-Output "PASS: $name" }
  else { $script:failures.Add($name); Write-Output "FAIL: $name" }
}

Add-Type -AssemblyName System.Drawing
Add-Type -Namespace Win32 -Name DPIF -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();'
[Win32.DPIF]::SetProcessDPIAware() | Out-Null
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class FGF {
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
if (-not (Test-Path $ShotDir)) { New-Item -ItemType Directory -Force $ShotDir | Out-Null }
function Capture([IntPtr]$h, [string]$name) {
  $r = New-Object FGF+RECT
  [FGF]::GetWindowRect($h, [ref]$r) | Out-Null
  $w = $r.R - $r.L; $ht = $r.B - $r.T
  if ($w -le 0 -or $ht -le 0) { Write-Output "BAD RECT"; return }
  $bmp = New-Object System.Drawing.Bitmap $w, $ht
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc()
  [FGF]::PrintWindow($h, $hdc, 2) | Out-Null
  $g.ReleaseHdc($hdc); $g.Dispose()
  $path = Join-Path $ShotDir "$name.png"
  if (Test-Path $path) { Remove-Item $path -Force }
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  Write-Output "SHOT: $path ($w x $ht)"
}

# JS single-quoted-string escaping for values embedded in Runtime.evaluate.
function Js([string]$s) { return $s.Replace('\', '\\').Replace("'", "\'") }

# --- True first-run: kill app, wipe localStorage, remove old test vault ---
Get-Process veylora -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
$ls = "$env:LOCALAPPDATA\com.veylora.app\EBWebView\Default\Local Storage"
if (Test-Path $ls) { Remove-Item $ls -Recurse -Force }
if (Test-Path $VaultPath) { Remove-Item $VaultPath -Recurse -Force }
Write-Output "first-run: localStorage=$(-not (Test-Path $ls)) oldVaultGone=$(-not (Test-Path $VaultPath))"
Check 'first-run wipe' ((-not (Test-Path $ls)) -and (-not (Test-Path $VaultPath)))
if (-not (Test-Path $ExePath)) { Write-Output "EXE NOT FOUND: $ExePath"; exit 1 }

# 64x64 PNG for the import (generated so the script has no machine deps).
$imgPath = Join-Path $ShotDir 'smoke-test-image.png'
$bmp = New-Object System.Drawing.Bitmap 64, 64
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::CornflowerBlue)
$g.Dispose()
$bmp.Save($imgPath, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()

$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
Start-Process $ExePath
# CDP can take a while on a cold first boot; poll for up to 60s instead of
# a single guess-and-hope attempt.
$list = $null
$connected = $false
$deadline = (Get-Date).AddSeconds(60)
while ((Get-Date) -lt $deadline) {
  try {
    $list = @(Invoke-RestMethod "http://127.0.0.1:$Port/json/list" -TimeoutSec 5)
    $connected = $true
    break
  } catch { Start-Sleep -Seconds 2 }
}
if (-not $connected) {
  $app = Get-Process veylora -ErrorAction SilentlyContinue
  $wv = @(Get-Process msedgewebview2 -ErrorAction SilentlyContinue)
  Write-Output "CDP never came up: veyloraRunning=$([bool]$app) webview2Processes=$($wv.Count) port=$Port"
  exit 1
}
$target = $list | Where-Object { $_.type -eq 'page' } | Select-Object -First 1
if (-not $target) { Write-Output "NO TARGET"; exit 1 }
$ws = New-Object System.Net.WebSockets.ClientWebSocket
$ct = [System.Threading.CancellationToken]::None
$ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl, $ct).Wait()
if ($ws.State -ne [System.Net.WebSockets.WebSocketState]::Open) { Write-Output "WS NOT OPEN"; exit 1 }

$script:msgId = 0
function Send-Cdp([string]$json) {
  $buf = [Text.Encoding]::UTF8.GetBytes($json)
  $seg = New-Object 'System.ArraySegment[byte]' -ArgumentList @(, $buf)
  $ws.SendAsync($seg, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $ct).Wait()
}
function Recv-Cdp {
  $ms = New-Object IO.MemoryStream
  do {
    $buf = New-Object byte[] 65536
    $seg = New-Object 'System.ArraySegment[byte]' -ArgumentList @(, $buf)
    $r = $ws.ReceiveAsync($seg, $ct); $r.Wait()
    $ms.Write($buf, 0, $r.Result.Count)
    $done = $r.Result.EndOfMessage
  } while (-not $done)
  [Text.Encoding]::UTF8.GetString($ms.ToArray())
}
function J-Eval([string]$expr) {
  $script:msgId++
  $myId = $script:msgId
  $payload = @{ id = $myId; method = 'Runtime.evaluate'; params = @{ expression = $expr; returnByValue = $true; awaitPromise = $true } } | ConvertTo-Json -Depth 6
  Send-Cdp $payload
  while ($true) {
    $resp = Recv-Cdp | ConvertFrom-Json
    if ($resp.id -eq $myId) {
      if ($resp.error) { return "CDPERR " + $resp.error.message }
      $ex = $resp.result.exceptionDetails
      if ($ex) { return "EXC " + ($ex.exception.text | ConvertTo-Json -Compress) }
      return $resp.result.result.value
    }
  }
}
function Fill([string]$id, [string]$val) {
  $e = "(()=>{const i=document.getElementById('$id');if(!i)return 'NO $id';const s=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;s.call(i,'" + (Js $val) + "');i.dispatchEvent(new Event('input',{bubbles:true}));return i.value;})()"
  Write-Output "FILL ${id}: $(J-Eval $e)"
}
function Stage([string]$label) {
  # String-coerce: J-Eval returns JS arrays as Object[], and Write-Host keeps
  # this out of the pipeline so `$x = Stage ...` captures only the text.
  $t = "$(J-Eval '[document.body.innerText.slice(0,90).replace(/\n/g," / ")]')"
  Write-Host "${label}: $t"
  return $t
}

# --- 1. Layer 1: login ---
$s0 = Stage 'S0-LOGIN'
Fill 'email' $AdminEmail
Fill 'password' $AdminPassword
Write-Output "LOGIN: $(J-Eval '[(()=>{document.querySelector("form").requestSubmit();return "fired";})()]')"
Start-Sleep -Seconds 4
$s1 = Stage 'S1-WELCOME'
Check 'login reaches Welcome' ($s1 -like '*Create Vault*')

# --- 2. Create the test vault ---
Fill 'name' $VaultName
Fill 'path' $VaultPath
Fill 'pin' $Pin
Fill 'password' $Password
Fill 'confirm-password' $Password
Write-Output "CREATE: $(J-Eval '[(()=>{const f=document.querySelectorAll("form")[0];f.requestSubmit();return "fired";})()]')"
Start-Sleep -Seconds 14
$s2 = Stage 'S2-RECOVERY'
Check 'recovery key shown once' ($s2 -like '*Recovery*')

# --- 3. Recovery: saved + Continue ---
Write-Output "CHECK: $(J-Eval '[(()=>{const c=document.querySelector("input[type=checkbox]");if(!c)return "NO BOX";c.click();return "checked="+c.checked;})()]')"
Write-Output "CONTINUE: $(J-Eval '[(()=>{const b=[...document.querySelectorAll("button")].find(b=>b.textContent.trim()==="Continue");if(!b)return "NO BTN";b.click();return "clicked";})()]')"
Start-Sleep -Seconds 4
$s3 = Stage 'S3-DASHBOARD'
Check 'dashboard after recovery' ($s3 -like '*Dashboard*')

# --- 4. Import the image, open it in the viewer ---
$import = J-Eval ('(async function(){try{const r=await window.__TAURI_INTERNALS__.invoke("file_import_cmd",{paths:["' + (Js $imgPath) + '"],folderId:null});return JSON.stringify(r);}catch(e){return "ERR "+String(e);}})()')
Write-Output "IMPORT: $import"
$fileId = if ($import -match '"file_id":"([0-9a-f]+)"') { $Matches[1] } else { $null }
Write-Output "FILE_ID: $fileId"
Check "import returns a file_id ($fileId)" ($null -ne $fileId)
Write-Output "GOTO FILES: $(J-Eval '[(()=>{const a=[...document.querySelectorAll("a")].find(a=>a.textContent.trim()==="Files");if(!a)return "NO LINK";a.click();return "clicked";})()]')"
Start-Sleep -Seconds 3
Write-Output "OPEN FILE: $(J-Eval '[(()=>{const b=[...document.querySelectorAll("button")].find(b=>b.textContent.trim()==="Open");if(!b)return "NO OPEN";b.click();return "clicked";})()]')"
Start-Sleep -Seconds 3
Write-Output "VIEWER URL: $(J-Eval '[location.pathname]')"

# --- 5. Verify the image renders and vault:// serves it ---
Start-Sleep -Seconds 2
$img = "$(J-Eval '[(()=>{const i=document.querySelector("img");if(!i)return "NO IMG";return "complete="+i.complete+" natural="+i.naturalWidth+"x"+i.naturalHeight;})()]')"
Write-Output "IMG: $img"
Check 'image viewer renders 64x64' ($img -like '*complete=true natural=64x64*')
# Build the expression first: `J-Eval 'a' + $id + 'b'` passes 3 arguments
# (only the first binds to $expr → JS SyntaxError), so concatenate explicitly.
$fetchExpr = '(async function(){try{const t=await window.__TAURI_INTERNALS__.invoke("vault_get_session_token_cmd",{});const u="http://vault.localhost/file/' + $fileId + '?token="+encodeURIComponent(String(t));const r=await fetch(u);const b=await r.arrayBuffer();return "status="+r.status+" bytes="+b.byteLength+" acao="+r.headers.get("access-control-allow-origin");}catch(e){return "ERR "+String(e);}})()'
$fetch = J-Eval $fetchExpr
Write-Output "FETCH: $fetch"
Check 'vault:// fetch returns 200' ($fetch -like '*status=200*')
$p0 = Get-Process veylora -ErrorAction SilentlyContinue
if ($p0) { Capture $p0.MainWindowHandle "smoke-viewer" }

# --- 6. Lock, then unlock with PIN ---
Write-Output "LOCK: $(J-Eval '[(()=>{const b=[...document.querySelectorAll("button")].find(b=>b.textContent.trim()==="Lock Vault");if(!b)return "NO LOCK";b.click();return "clicked";})()]')"
Start-Sleep -Seconds 3
$s5 = Stage 'S5-LOCKED'
Check 'lock shows unlock screen' ($s5 -like '*Unlock Vault*')
Fill 'pin' $Pin
Write-Output "UNLOCK: $(J-Eval '[(()=>{const i=document.getElementById("pin");if(!i)return "NO PIN";i.form.requestSubmit();return "fired";})()]')"
Start-Sleep -Seconds 6
$s6 = Stage 'S6-AFTER-UNLOCK'
Check 'PIN unlock returns to app' ($s6 -like '*Dashboard*')
Write-Output "BACKEND: $(J-Eval '(async function(){try{const r=await window.__TAURI_INTERNALS__.invoke("vault_status_cmd",{});return JSON.stringify(r);}catch(e){return "ERR "+String(e);}})()')"

# --- 7. Dashboard shows real counts ---
Write-Output "NAV DASH: $(J-Eval '[(()=>{const a=[...document.querySelectorAll("a")].find(a=>a.textContent.trim()==="Dashboard");if(!a)return "NO LINK";a.click();return "clicked";})()]')"
Start-Sleep -Seconds 3
$counts = "$(J-Eval '[(()=>{const t=document.body.innerText;const m=t.match(/\d+ files encrypted/);const s=t.match(/Storage: [^\\n]+/);return (m?m[0]:"NO COUNT")+" | "+(s?s[0]:"NO STORAGE");})()]')"
Write-Output "DASH COUNTS: $counts"
Check 'dashboard shows file count + storage' (($counts -like '*files encrypted*') -and ($counts -like '*Storage:*'))
$p = Get-Process veylora -ErrorAction SilentlyContinue
if ($p) { Capture $p.MainWindowHandle "smoke-dashboard" } else { Write-Output "APP GONE"; Check 'app still running' $false }

$ws.Dispose()
if ($script:failures.Count -gt 0) {
  Write-Output "SMOKE FAILED ($($script:failures.Count)): $($script:failures -join ', ')"
  exit 1
}
Write-Output "DONE"
exit 0
