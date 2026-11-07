// AULA.exe — lançador portátil de aulas em PDF.
//
// Sem argumentos: abre, em tela cheia, o PDF desta pasta (turma) onde a
// última aula parou e, ao fechar, registra a aula em `aula.txt`.
// Com um vídeo (arrastado sobre o .exe) ou um link `aula://...`: toca no VLC
// portátil do pendrive (ou abre a pasta no Explorer, se o link for uma pasta).
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const LOG: &str = "aula.txt";
const APPDATA: &str = ".sumatra";
const PROGRAMAS: &str = "_programas";
const VIDEO_EXT: &[&str] = &[
    "mp4", "m4v", "mkv", "avi", "mov", "wmv", "webm", "mpg", "mpeg", "flv", "3gp", "mp3", "wav",
    "ogg", "m4a",
];

fn main() {
    if let Err(e) = run() {
        aviso(&e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let exe = env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe.parent().unwrap().to_path_buf();

    let mut base: Option<PathBuf> = None;
    let mut resto = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--base" {
            base = args.next().map(PathBuf::from);
        } else {
            resto.push(a);
        }
    }

    let Some(a) = resto.first() else {
        return lancar(&exe_dir, &exe);
    };
    if a.to_ascii_lowercase().starts_with("aula:") {
        let base = base.unwrap_or_else(|| exe_dir.clone());
        return abrir_link(a, &base, &exe_dir);
    }
    let p = PathBuf::from(a);
    if p.is_dir() {
        return lancar(&p, &exe);
    }
    if p.is_file() && eh_video(&p) {
        let turma = achar_turma(&p).or_else(|| achar_turma(&exe_dir));
        return tocar(&p, turma.as_deref(), &exe_dir);
    }
    Err(format!("Não sei abrir isto:\n{a}"))
}

// ---------------------------------------------------------------- PDF / aula

fn lancar(turma: &Path, exe: &Path) -> Result<(), String> {
    let pdfs = listar_pdfs(turma);
    if pdfs.is_empty() {
        return Err(format!("Nenhum PDF na pasta:\n{}", turma.display()));
    }
    let log = fs::read_to_string(turma.join(LOG)).unwrap_or_default();
    let (pdf, pagina) = retomar(&log, &pdfs);

    let sumatra = achar_programa(&[turma, exe.parent().unwrap()], |n| {
        n.starts_with("sumatrapdf") && n.ends_with(".exe")
    })
    .ok_or("SumatraPDF não encontrado.\nColoque o SumatraPDF portátil na pasta _programas do pendrive.")?;

    let appdata = turma.join(APPDATA);
    preparar_appdata(&appdata);
    if let Some(dir) = sumatra.parent() {
        liberar_links(dir);
    }
    registrar_protocolo(exe, turma);

    let inicio = agora();
    let status = Command::new(&sumatra)
        .arg("-appdata")
        .arg(&appdata)
        .arg("-new-window")
        .arg("-presentation")
        .arg("-page")
        .arg(pagina.to_string())
        .arg(&pdf)
        .status();
    let fim = agora();
    remover_protocolo();
    status.map_err(|e| format!("Falha ao abrir o SumatraPDF: {e}"))?;

    let pag_final = pagina_salva(&appdata, &pdf).unwrap_or(pagina);
    let linha = format!(
        "{} {}-{} | {} | p. {} -> {}",
        inicio.data(),
        inicio.hora(),
        fim.hora(),
        nome(&pdf),
        pagina,
        pag_final
    );
    anotar(turma, &linha)
}

fn listar_pdfs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && ext(p) == "pdf")
        .collect();
    v.sort_by_key(|p| nome(p).to_lowercase());
    v
}

/// Decide qual PDF e página abrir, pela última linha de aula no registro.
fn retomar(log: &str, pdfs: &[PathBuf]) -> (PathBuf, u32) {
    for linha in log.lines().rev() {
        let campos: Vec<&str> = linha.split(" | ").map(str::trim).collect();
        if campos.len() < 3 || !campos[1].to_lowercase().ends_with(".pdf") {
            continue;
        }
        let Some(fim) = campos[2]
            .rsplit("->")
            .next()
            .and_then(|s| s.trim().parse::<u32>().ok())
        else {
            continue;
        };
        let Some(i) = pdfs.iter().position(|p| nome(p).eq_ignore_ascii_case(campos[1])) else {
            break; // o PDF do registro não está mais na pasta: começa do início
        };
        let terminou = paginas_pdf(&pdfs[i]).is_some_and(|n| fim >= n);
        if terminou && i + 1 < pdfs.len() {
            return (pdfs[i + 1].clone(), 1);
        }
        return (pdfs[i].clone(), fim.max(1));
    }
    (pdfs[0].clone(), 1)
}

/// Número de páginas pelo maior `/Count` do arquivo (heurística; None se não achar).
fn paginas_pdf(pdf: &Path) -> Option<u32> {
    let b = fs::read(pdf).ok()?;
    let pat = b"/Count";
    let mut max: Option<u32> = None;
    let mut i = 0;
    while i + pat.len() <= b.len() {
        if &b[i..i + pat.len()] != pat {
            i += 1;
            continue;
        }
        let mut j = i + pat.len();
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        let k = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if let Some(n) = std::str::from_utf8(&b[k..j]).ok().and_then(|s| s.parse().ok()) {
            max = Some(max.map_or(n, |m| m.max(n)));
        }
        i = j.max(i + 1);
    }
    max
}

fn preparar_appdata(appdata: &Path) {
    let _ = fs::create_dir_all(appdata);
    let cfg = appdata.join("SumatraPDF-settings.txt");
    let txt = fs::read_to_string(&cfg).unwrap_or_default();
    // Cada lançamento precisa de um Sumatra próprio; se ele reaproveitar outra
    // janela aberta, o processo termina na hora e o registro sai errado.
    let novo = if txt.is_empty() {
        "ReuseInstance = false\r\nRememberOpenedFiles = true\r\nRememberStatePerDocument = true\r\n"
            .to_string()
    } else if txt.contains("ReuseInstance = true") {
        txt.replace("ReuseInstance = true", "ReuseInstance = false")
    } else {
        return;
    };
    let _ = fs::write(&cfg, novo);
}

/// O Sumatra só segue links de protocolos permitidos (padrão: http, https,
/// mailto, file) e ignora os outros em silêncio. A lista fica em
/// `sumatrapdfrestrict.ini`, ao lado do SumatraPDF.exe; quando esse arquivo
/// existe, toda permissão não listada fica desligada, então ele é criado
/// com todas ligadas.
fn liberar_links(dir_sumatra: &Path) {
    let ini = dir_sumatra.join("sumatrapdfrestrict.ini");
    let novo = match fs::read_to_string(&ini) {
        Err(_) => RESTRICT_INI.to_string(),
        Ok(txt) => match ajustar_restrict(&txt) {
            Some(t) => t,
            None => return,
        },
    };
    let _ = fs::write(&ini, novo);
}

const RESTRICT_INI: &str = "\
; Criado pelo AULA.exe: libera o protocolo aula:// nos links dos PDFs.\r
; Com este arquivo, permissões não listadas ficam desligadas.\r
[Policies]\r
InternetAccess = 1\r
DiskAccess = 1\r
SavePreferences = 1\r
RegistryAccess = 1\r
PrinterAccess = 1\r
CopySelection = 1\r
FullscreenAccess = 1\r
LinkProtocols = http,https,mailto,file,aula\r
SafeFileTypes = audio,video,webpage\r
";

/// Acrescenta `aula` a LinkProtocols de um ini existente (None: nada a mudar).
fn ajustar_restrict(txt: &str) -> Option<String> {
    let mut achou = false;
    let mut mudou = false;
    let linhas: Vec<String> = txt
        .lines()
        .map(|l| {
            let (chave, valor) = l.split_once('=').unwrap_or((l, ""));
            if !chave.trim().eq_ignore_ascii_case("LinkProtocols") {
                return l.to_string();
            }
            achou = true;
            let tem = valor
                .split([',', ' ', ':', ';'])
                .any(|p| p.trim().eq_ignore_ascii_case("aula"));
            if tem {
                return l.to_string();
            }
            mudou = true;
            format!("{}= {},aula", chave, valor.trim())
        })
        .collect();
    if achou {
        return mudou.then(|| linhas.join("\r\n") + "\r\n");
    }
    // sem LinkProtocols: acrescenta (o padrão do Sumatra + aula) em [Policies]
    let mut saida = Vec::new();
    let mut inseriu = false;
    for l in &linhas {
        saida.push(l.clone());
        if !inseriu && l.trim().eq_ignore_ascii_case("[Policies]") {
            saida.push("LinkProtocols = http,https,mailto,file,aula".into());
            inseriu = true;
        }
    }
    if !inseriu {
        saida.push("[Policies]".into());
        saida.push("LinkProtocols = http,https,mailto,file,aula".into());
    }
    Some(saida.join("\r\n") + "\r\n")
}

/// Última página do `pdf` gravada pelo Sumatra em FileStates.
fn pagina_salva(appdata: &Path, pdf: &Path) -> Option<u32> {
    let txt = fs::read_to_string(appdata.join("SumatraPDF-settings.txt")).ok()?;
    let alvo = normalizar(&pdf.to_string_lossy());
    let mut atual = false;
    for l in txt.lines().map(str::trim) {
        if let Some(v) = l.strip_prefix("FilePath =") {
            atual = normalizar(v.trim()) == alvo;
        } else if atual {
            if let Some(v) = l.strip_prefix("PageNo =") {
                return v.trim().parse().ok();
            }
        }
    }
    None
}

fn normalizar(s: &str) -> String {
    s.replace('/', "\\").trim_start_matches("\\\\?\\").to_lowercase()
}

// ---------------------------------------------------------------- vídeos

fn abrir_link(url: &str, base: &Path, exe_dir: &Path) -> Result<(), String> {
    let resto = url.split_once(':').map_or("", |(_, r)| r);
    let rel = decodificar(resto.trim_start_matches('/').trim_end_matches('/')).replace('/', "\\");
    let mut alvo = base.join(&rel);
    if !alvo.exists() {
        // link com caminho de outro PC: procura pelo nome dentro da turma
        let nome_arq = rel.rsplit('\\').next().unwrap_or(&rel).to_string();
        alvo = procurar(base, &nome_arq, 4)
            .ok_or_else(|| format!("Não achei:\n{rel}\nem {}", base.display()))?;
    }
    if alvo.is_dir() {
        Command::new("explorer").arg(&alvo).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    tocar(&alvo, Some(base), exe_dir)
}

fn tocar(video: &Path, turma: Option<&Path>, exe_dir: &Path) -> Result<(), String> {
    let mut lugares: Vec<&Path> = vec![video.parent().unwrap_or(video), exe_dir];
    if let Some(t) = turma {
        lugares.push(t);
    }
    let vlc = achar_programa(&lugares, |n| n == "vlc.exe")
        .ok_or("VLC não encontrado.\nColoque o VLC portátil na pasta _programas do pendrive.")?;
    Command::new(vlc)
        .args(["--fullscreen", "--play-and-exit", "--no-video-title-show"])
        .arg(video)
        .spawn()
        .map_err(|e| format!("Falha ao abrir o VLC: {e}"))?;
    if let Some(t) = turma {
        let a = agora();
        let _ = anotar(t, &format!("{} {} | vídeo | {}", a.data(), a.hora(), nome(video)));
    }
    Ok(())
}

fn eh_video(p: &Path) -> bool {
    VIDEO_EXT.contains(&ext(p).as_str())
}

/// Sobe a partir do vídeo até achar a pasta da turma (com aula.txt ou PDFs).
fn achar_turma(p: &Path) -> Option<PathBuf> {
    p.ancestors()
        .filter(|d| d.is_dir())
        .find(|d| d.join(LOG).is_file() || !listar_pdfs(d).is_empty())
        .map(Path::to_path_buf)
}

/// Decodifica `%XX` de links (espaços, acentos em UTF-8).
fn decodificar(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn procurar(dir: &Path, nome_arq: &str, prof: u32) -> Option<PathBuf> {
    let mut subdirs = Vec::new();
    for e in fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if nome(&p).eq_ignore_ascii_case(nome_arq) {
            return Some(p);
        }
        if p.is_dir() {
            subdirs.push(p);
        }
    }
    if prof == 0 {
        return None;
    }
    subdirs.into_iter().find_map(|d| procurar(&d, nome_arq, prof - 1))
}

// ---------------------------------------------------------------- utilidades

/// Procura `_programas` subindo a partir de cada lugar e, dentro dela, o executável.
fn achar_programa(lugares: &[&Path], casa: impl Fn(&str) -> bool + Copy) -> Option<PathBuf> {
    for l in lugares {
        for d in l.ancestors() {
            let prog = d.join(PROGRAMAS);
            if prog.is_dir() {
                if let Some(p) = buscar_exe(&prog, casa, 3) {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn buscar_exe(dir: &Path, casa: impl Fn(&str) -> bool + Copy, prof: u32) -> Option<PathBuf> {
    let mut entradas: Vec<PathBuf> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).collect();
    entradas.sort();
    if let Some(p) = entradas.iter().find(|p| p.is_file() && casa(&nome(p).to_lowercase())) {
        return Some(p.clone());
    }
    if prof == 0 {
        return None;
    }
    entradas.iter().filter(|p| p.is_dir()).find_map(|d| buscar_exe(d, casa, prof - 1))
}

fn anotar(turma: &Path, linha: &str) -> Result<(), String> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(turma.join(LOG))
        .map_err(|e| format!("Não consegui gravar o registro: {e}"))?;
    write!(f, "{linha}\r\n").map_err(|e| e.to_string())
}

fn nome(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn ext(p: &Path) -> String {
    p.extension().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

struct Momento {
    ano: u16,
    mes: u16,
    dia: u16,
    h: u16,
    min: u16,
}

impl Momento {
    fn data(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.ano, self.mes, self.dia)
    }
    fn hora(&self) -> String {
        format!("{:02}:{:02}", self.h, self.min)
    }
}

// ---------------------------------------------------------------- Windows

#[cfg(windows)]
mod win {
    #[repr(C)]
    #[derive(Default)]
    pub struct SystemTime {
        pub ano: u16,
        pub mes: u16,
        pub dia_semana: u16,
        pub dia: u16,
        pub h: u16,
        pub min: u16,
        pub s: u16,
        pub ms: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetLocalTime(st: *mut SystemTime);
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn MessageBoxW(h: isize, txt: *const u16, tit: *const u16, tipo: u32) -> i32;
    }
}

#[cfg(windows)]
fn agora() -> Momento {
    let mut st = win::SystemTime::default();
    unsafe { win::GetLocalTime(&mut st) };
    Momento { ano: st.ano, mes: st.mes, dia: st.dia, h: st.h, min: st.min }
}

#[cfg(not(windows))]
fn agora() -> Momento {
    // fora do Windows (só nos testes): UTC
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (dias, seg) = (s.div_euclid(86400), s.rem_euclid(86400));
    // algoritmo civil de Howard Hinnant
    let z = dias + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Momento {
        ano: y as u16,
        mes: m as u16,
        dia: d as u16,
        h: (seg / 3600) as u16,
        min: (seg % 3600 / 60) as u16,
    }
}

fn aviso(msg: &str) {
    #[cfg(windows)]
    if env::var_os("AULA_SEM_JANELA").is_none() {
        let w = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        unsafe { win::MessageBoxW(0, w(msg).as_ptr(), w("Aula").as_ptr(), 0x30) };
        return;
    }
    eprintln!("{msg}");
}

#[cfg(windows)]
fn reg(args: &[&str]) {
    use std::os::windows::process::CommandExt;
    let _ = Command::new("reg")
        .args(args)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .status();
}

/// Registra `aula://` só para o usuário atual (HKCU, sem administrador).
#[cfg(windows)]
fn registrar_protocolo(exe: &Path, turma: &Path) {
    let chave = r"HKCU\Software\Classes\aula";
    let cmd = format!(
        "\"{}\" --base \"{}\" \"%1\"",
        exe.display(),
        turma.display().to_string().trim_end_matches('\\')
    );
    reg(&["add", chave, "/ve", "/d", "URL:Aula", "/f"]);
    reg(&["add", chave, "/v", "URL Protocol", "/d", "", "/f"]);
    reg(&["add", &format!(r"{chave}\shell\open\command"), "/ve", "/d", &cmd, "/f"]);
}

#[cfg(windows)]
fn remover_protocolo() {
    if env::var_os("AULA_MANTER_PROTOCOLO").is_none() {
        reg(&["delete", r"HKCU\Software\Classes\aula", "/f"]);
    }
}

#[cfg(not(windows))]
fn registrar_protocolo(_: &Path, _: &Path) {}
#[cfg(not(windows))]
fn remover_protocolo() {}

#[cfg(test)]
mod testes {
    use super::*;

    fn pdfs(nomes: &[&str]) -> Vec<PathBuf> {
        nomes.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn sem_registro_comeca_no_primeiro() {
        let v = pdfs(&["01 a.pdf", "02 b.pdf"]);
        assert_eq!(retomar("", &v), (v[0].clone(), 1));
    }

    #[test]
    fn retoma_ultima_pagina_ignorando_videos() {
        let v = pdfs(&["01 a.pdf", "02 b.pdf"]);
        let log = "2026-11-10 19:00-20:40 | 01 a.pdf | p. 1 -> 12\r\n\
                   2026-11-12 19:00-20:40 | 02 b.pdf | p. 1 -> 7\r\n\
                   2026-11-12 19:30 | vídeo | 02-15 x.mp4\r\n";
        assert_eq!(retomar(log, &v), (v[1].clone(), 7));
    }

    #[test]
    fn pdf_sumido_volta_ao_inicio() {
        let v = pdfs(&["01 a.pdf"]);
        let log = "2026-11-10 19:00-20:40 | velho.pdf | p. 1 -> 9";
        assert_eq!(retomar(log, &v), (v[0].clone(), 1));
    }

    #[test]
    fn terminou_pdf_vai_para_o_proximo() {
        let d = env::temp_dir().join("aula_teste_pdfs");
        let _ = fs::create_dir_all(&d);
        fs::write(d.join("01 a.pdf"), "%PDF-1.4\n<< /Type /Pages /Count 3 >>\n").unwrap();
        fs::write(d.join("02 b.pdf"), "%PDF-1.4\n<< /Type /Pages /Count 5 >>\n").unwrap();
        let v = listar_pdfs(&d);
        assert_eq!(retomar("x | 01 a.pdf | p. 1 -> 3", &v), (v[1].clone(), 1));
        assert_eq!(retomar("x | 01 a.pdf | p. 1 -> 2", &v), (v[0].clone(), 2));
        assert_eq!(retomar("x | 02 b.pdf | p. 2 -> 5", &v), (v[1].clone(), 5));
    }

    #[test]
    fn restrict_ini() {
        assert_eq!(ajustar_restrict("[Policies]\r\nLinkProtocols = http,aula\r\n"), None);
        assert_eq!(
            ajustar_restrict("[Policies]\nDiskAccess = 1\nLinkProtocols = http,https\n").unwrap(),
            "[Policies]\r\nDiskAccess = 1\r\nLinkProtocols = http,https,aula\r\n"
        );
        assert_eq!(
            ajustar_restrict("[Policies]\nDiskAccess = 1\n").unwrap(),
            "[Policies]\r\nLinkProtocols = http,https,mailto,file,aula\r\nDiskAccess = 1\r\n"
        );
        assert!(RESTRICT_INI.contains("DiskAccess = 1") && RESTRICT_INI.contains(",aula"));
    }

    #[test]
    fn decodifica_link() {
        assert_eq!(
            decodificar("videos/02-15%20Bomba%20centr%C3%ADfuga.mp4"),
            "videos/02-15 Bomba centrífuga.mp4"
        );
        assert_eq!(decodificar("a%2"), "a%2");
        assert_eq!(decodificar("ação%zz"), "ação%zz");
    }

    #[test]
    fn le_pagina_do_sumatra() {
        let d = env::temp_dir().join("aula_teste_cfg");
        let _ = fs::create_dir_all(&d);
        fs::write(
            d.join("SumatraPDF-settings.txt"),
            "FileStates [\n\t[\n\t\tFilePath = C:\\x\\outro.pdf\n\t\tPageNo = 3\n\t]\n\t[\n\t\tFilePath = C:\\X\\Aula.pdf\n\t\tPageNo = 34\n\t]\n]\n",
        )
        .unwrap();
        assert_eq!(pagina_salva(&d, Path::new("C:\\x\\aula.pdf")), Some(34));
    }
}
