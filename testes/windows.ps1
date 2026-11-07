# Teste de ponta a ponta no Windows (GitHub Actions).
# Monta um "pendrive" falso com _programas\ (Sumatra + VLC) e uma turma,
# roda o AULA.exe, fecha o Sumatra como um usuário faria e confere o aula.txt.
param([string]$Exe = "target\release\aula.exe")
$ErrorActionPreference = "Stop"

$raiz = Join-Path $env:RUNNER_TEMP "pendrive"
$prog = Join-Path $raiz "_programas"
$turma = Join-Path $raiz "Hidráulica - Noite"
New-Item -ItemType Directory -Force $prog, $turma, (Join-Path $turma "videos") | Out-Null

$ver = "3.6.1"
Invoke-WebRequest "https://www.sumatrapdfreader.org/dl/rel/$ver/SumatraPDF-$ver-64.zip" -OutFile "$env:RUNNER_TEMP\s.zip"
Expand-Archive "$env:RUNNER_TEMP\s.zip" -DestinationPath (Join-Path $prog "SumatraPDF")
Invoke-WebRequest "https://get.videolan.org/vlc/3.0.21/win64/vlc-3.0.21-win64.zip" -OutFile "$env:RUNNER_TEMP\v.zip"
Expand-Archive "$env:RUNNER_TEMP\v.zip" -DestinationPath $prog

Copy-Item "testes\01 Introducao.pdf", "testes\02 Elevacao.pdf" $turma
Set-Content (Join-Path $turma "videos\02-15 Bomba centrífuga.mp4") "não é vídeo de verdade"
$aula = Join-Path $turma "AULA.exe"
Copy-Item $Exe $aula
$log = Join-Path $turma "aula.txt"
$env:AULA_SEM_JANELA = "1"

function Falha($msg) { Write-Host "::error::$msg"; if (Test-Path $log) { Get-Content $log }; exit 1 }

function Rodada($esperado, [switch]$checaProtocolo) {
    $p = Start-Process $aula -PassThru
    $s = $null
    for ($i = 0; $i -lt 30 -and -not $s; $i++) {
        Start-Sleep 1
        $s = Get-Process SumatraPDF -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0
    }
    if (-not $s) { Falha "Sumatra não abriu" }
    if ($p.HasExited) { Falha "AULA.exe terminou antes do Sumatra fechar (instância reaproveitada?)" }
    Write-Host "Sumatra aberto: $($s.MainWindowTitle)"
    if ($checaProtocolo) {
        $cmd = (Get-ItemProperty "HKCU:\Software\Classes\aula\shell\open\command").'(default)'
        Write-Host "aula:// -> $cmd"
        if ($cmd -notlike "*AULA.exe*--base*%1*") { Falha "protocolo aula:// mal registrado" }
    }
    Start-Sleep 2
    $s | ForEach-Object { $_.CloseMainWindow() | Out-Null }
    if (-not $p.WaitForExit(30000)) { Falha "AULA.exe não terminou após fechar o Sumatra" }
    $ultima = (Get-Content $log -Encoding UTF8)[-1]
    Write-Host "registro: $ultima"
    if ($ultima -notlike "*$esperado") { Falha "esperado '...$esperado', veio '$ultima'" }
}

# 1) sem registro: primeiro PDF, página 1
Rodada "| 01 Introducao.pdf | p. 1 -> 1" -checaProtocolo
if (Test-Path "HKCU:\Software\Classes\aula") { Falha "protocolo não foi removido ao fechar" }

# 2) parou no meio: retoma na página 2 (confere que o -page chegou ao Sumatra e voltou no FileStates)
Add-Content $log "2026-01-01 10:00-11:00 | 01 Introducao.pdf | p. 1 -> 2"
Rodada "| 01 Introducao.pdf | p. 2 -> 2"

# 3) terminou o PDF (3 de 3): próxima aula começa no seguinte
Add-Content $log "2026-01-02 10:00-11:00 | 01 Introducao.pdf | p. 2 -> 3"
Rodada "| 02 Elevacao.pdf | p. 1 -> 1"

# 4) link aula:// (como no clique do slide): resolve o caminho e abre o VLC
$p = Start-Process $aula -ArgumentList "--base", "`"$turma`"", "aula://videos/02-15%20Bomba%20centr%C3%ADfuga.mp4" -PassThru -Wait
if ($p.ExitCode -ne 0) { Falha "link aula:// falhou (código $($p.ExitCode))" }
Start-Sleep 3
if (-not (Get-Process vlc -ErrorAction SilentlyContinue)) { Falha "VLC não abriu" }
Get-Process vlc | Stop-Process -Force
$ultima = (Get-Content $log -Encoding UTF8)[-1]
if ($ultima -notlike "*| vídeo | 02-15 Bomba centrífuga.mp4") { Falha "vídeo não registrado: $ultima" }

# 5) vídeo arrastado sobre o .exe (TOCAR.exe)
$tocar = Join-Path $turma "videos\TOCAR.exe"
Copy-Item $Exe $tocar
$p = Start-Process $tocar -ArgumentList "`"$(Join-Path $turma 'videos\02-15 Bomba centrífuga.mp4')`"" -PassThru -Wait
if ($p.ExitCode -ne 0) { Falha "TOCAR.exe falhou (código $($p.ExitCode))" }
Start-Sleep 2
Get-Process vlc -ErrorAction SilentlyContinue | Stop-Process -Force
if ((Get-Content $log -Encoding UTF8 | Select-String "vídeo").Count -ne 2) { Falha "TOCAR.exe não registrou na turma" }

# 6) pasta sem PDF: erro claro, sem travar
$vazia = Join-Path $raiz "vazia"; New-Item -ItemType Directory -Force $vazia | Out-Null
Copy-Item $Exe (Join-Path $vazia "AULA.exe")
$p = Start-Process (Join-Path $vazia "AULA.exe") -PassThru -Wait
if ($p.ExitCode -ne 1) { Falha "pasta vazia deveria dar erro" }

Write-Host "`nRegistro final:"; Get-Content $log -Encoding UTF8
Write-Host "OK"
