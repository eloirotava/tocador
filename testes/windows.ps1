# Teste de ponta a ponta no Windows (GitHub Actions).
# Monta um "pendrive" falso com _programas\ (Sumatra + VLC) e uma turma,
# roda o AULA.exe, fecha o Sumatra como um usuário faria e confere o aula.txt.
param([string]$Exe = "target\release\aula.exe")
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
trap { Write-Host "::error::$($_ | Out-String)"; Write-Host $_.ScriptStackTrace; exit 1 }

function Baixar($url, $destino) {
    $zip = Join-Path $env:RUNNER_TEMP ([IO.Path]::GetRandomFileName() + ".zip")
    curl.exe -fsSL --retry 3 -o $zip $url
    if ($LASTEXITCODE) { throw "download falhou: $url" }
    New-Item -ItemType Directory -Force $destino | Out-Null
    tar.exe -xf $zip -C $destino
    if ($LASTEXITCODE) { throw "descompactar falhou: $url" }
}

$raiz = Join-Path $env:RUNNER_TEMP "pendrive"
$prog = Join-Path $raiz "_programas"
$turma = Join-Path $raiz "Hidráulica - Noite"
New-Item -ItemType Directory -Force $prog, $turma, (Join-Path $turma "videos") | Out-Null

$ver = "3.6.1"
Baixar "https://www.sumatrapdfreader.org/dl/rel/$ver/SumatraPDF-$ver-64.zip" (Join-Path $prog "SumatraPDF")
Baixar "https://get.videolan.org/vlc/3.0.21/win64/vlc-3.0.21-win64.zip" $prog
Get-ChildItem $prog -Recurse -Include SumatraPDF*.exe, vlc.exe | ForEach-Object { Write-Host "programa: $($_.FullName)" }

Copy-Item "testes\01 Introducao.pdf", "testes\02 Elevacao.pdf" $turma
# "vídeo" de teste: 10 s de silêncio em WAV (o VLC reconhece pelo conteúdo)
$taxa = 8000; $dados = $taxa * 10
$wav = New-Object IO.MemoryStream; $w = New-Object IO.BinaryWriter $wav
$w.Write([Text.Encoding]::ASCII.GetBytes("RIFF")); $w.Write([int](36 + $dados)); $w.Write([Text.Encoding]::ASCII.GetBytes("WAVEfmt "))
$w.Write([int]16); $w.Write([int16]1); $w.Write([int16]1); $w.Write([int]$taxa); $w.Write([int]$taxa); $w.Write([int16]1); $w.Write([int16]8)
$silencio = [byte[]]::new($dados); [Array]::Fill($silencio, [byte]128)
$w.Write([Text.Encoding]::ASCII.GetBytes("data")); $w.Write([int]$dados); $w.Write($silencio)
$w.Flush(); [IO.File]::WriteAllBytes((Join-Path $turma "videos\02-15 Bomba centrífuga.mp4"), $wav.ToArray())

function EsperaVlc {
    # o VLC é aberto e o AULA.exe sai na hora; procura o processo por até 10 s
    for ($i = 0; $i -lt 100; $i++) {
        $v = Get-CimInstance Win32_Process -Filter "Name = 'vlc.exe'"
        if ($v) {
            Write-Host "VLC: $($v[0].CommandLine)"
            if ($v[0].CommandLine -notlike "*02-15 Bomba centrífuga.mp4*") { Falha "VLC com arquivo errado" }
            $v | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
            return
        }
        Start-Sleep -Milliseconds 100
    }
    Falha "VLC não abriu"
}
$aula = Join-Path $turma "AULA.exe"
Copy-Item $Exe $aula
$log = Join-Path $turma "aula.txt"
$env:AULA_SEM_JANELA = "1"

Add-Type @"
using System; using System.Collections.Generic; using System.Runtime.InteropServices; using System.Text;
public static class Janelas {
    delegate bool Cb(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(Cb cb, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    public static List<string> Listar(int[] pids) {
        var r = new List<string>();
        EnumWindows((h, l) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (Array.IndexOf(pids, (int)pid) >= 0 && IsWindowVisible(h)) {
                var t = new StringBuilder(256); var c = new StringBuilder(256);
                GetWindowText(h, t, 256); GetClassName(h, c, 256);
                r.Add(h.ToInt64() + " [" + c + "] " + t);
            }
            return true;
        }, IntPtr.Zero);
        return r;
    }
    public static void Fechar(IntPtr h) { PostMessage(h, 0x0010, IntPtr.Zero, IntPtr.Zero); }
}
"@

function Falha($msg) { Write-Host "::error::$msg"; if (Test-Path $log) { Get-Content $log }; exit 1 }

function Rodada($esperado, [switch]$checaProtocolo) {
    $p = Start-Process $aula -PassThru
    $s = $null
    for ($i = 0; $i -lt 30 -and -not $s; $i++) {
        Start-Sleep 1
        $s = Get-Process SumatraPDF* -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0
    }
    if (-not $s) {
        Write-Host "AULA.exe terminou: $($p.HasExited) código: $(if ($p.HasExited) { $p.ExitCode })"
        Get-CimInstance Win32_Process -Filter "Name like 'Sumatra%'" | ForEach-Object { Write-Host "processo: $($_.ProcessId) $($_.CommandLine)" }
        Get-Process SumatraPDF* -ErrorAction SilentlyContinue | ForEach-Object { Write-Host "janela: $($_.Id) handle=$($_.MainWindowHandle) sessão=$($_.SessionId)" }
        Write-Host "sessão do teste: $((Get-Process -Id $PID).SessionId)"
        $cfg = Join-Path $turma ".sumatra\SumatraPDF-settings.txt"
        if (Test-Path $cfg) { Get-Content $cfg | Select-Object -First 15 }
        Falha "Sumatra não abriu"
    }
    if ($p.HasExited) { Falha "AULA.exe terminou antes do Sumatra fechar (instância reaproveitada?)" }
    Write-Host "Sumatra aberto: $($s.MainWindowTitle)"
    if ($checaProtocolo) {
        $cmd = (Get-ItemProperty "HKCU:\Software\Classes\aula\shell\open\command").'(default)'
        Write-Host "aula:// -> $cmd"
        if ($cmd -notlike "*AULA.exe*--base*%1*") { Falha "protocolo aula:// mal registrado" }
    }
    Start-Sleep 2
    # Fecha como o usuário: WM_CLOSE em todas as janelas visíveis do Sumatra
    # (a janela principal do PowerShell pode ser uma auxiliar).
    $ids = @($s | ForEach-Object Id)
    for ($i = 0; $i -lt 15 -and -not $p.HasExited; $i++) {
        foreach ($j in [Janelas]::Listar($ids)) {
            if ($i -eq 0 -or $i -eq 14) { Write-Host "  janela: $j" }
            [Janelas]::Fechar([IntPtr][long]($j -split ' ')[0])
        }
        Start-Sleep 2
    }
    if (-not $p.WaitForExit(5000)) { Falha "AULA.exe não terminou após fechar o Sumatra" }
    $ultima = @(Get-Content $log -Encoding UTF8)[-1]
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
EsperaVlc
$ultima = @(Get-Content $log -Encoding UTF8)[-1]
if ($ultima -notlike "*| vídeo | 02-15 Bomba centrífuga.mp4") { Falha "vídeo não registrado: $ultima" }

# 5) vídeo arrastado sobre o .exe (TOCAR.exe)
$tocar = Join-Path $turma "videos\TOCAR.exe"
Copy-Item $Exe $tocar
$p = Start-Process $tocar -ArgumentList "`"$(Join-Path $turma 'videos\02-15 Bomba centrífuga.mp4')`"" -PassThru -Wait
if ($p.ExitCode -ne 0) { Falha "TOCAR.exe falhou (código $($p.ExitCode))" }
EsperaVlc
if ((Get-Content $log -Encoding UTF8 | Select-String "vídeo").Count -ne 2) { Falha "TOCAR.exe não registrou na turma" }

# 6) pasta sem PDF: erro claro, sem travar
$vazia = Join-Path $raiz "vazia"; New-Item -ItemType Directory -Force $vazia | Out-Null
Copy-Item $Exe (Join-Path $vazia "AULA.exe")
$p = Start-Process (Join-Path $vazia "AULA.exe") -PassThru -Wait
if ($p.ExitCode -ne 1) { Falha "pasta vazia deveria dar erro" }

Write-Host "`nRegistro final:"; Get-Content $log -Encoding UTF8
Write-Host "OK"
