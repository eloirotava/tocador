// Embute o ícone e as informações de versão no AULA.exe.
fn main() {
    println!("cargo:rerun-if-changed=icone/aula.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("icone/aula.ico")
        .set("ProductName", "Aula")
        .set("FileDescription", "Lançador de aulas em PDF")
        .set("OriginalFilename", "AULA.exe")
        .set("LegalCopyright", "Eloi Rotava");
    if let Err(e) = res.compile() {
        panic!("falha ao embutir o ícone: {e}");
    }
}
