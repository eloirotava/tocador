# Tocador de aulas

Lançador portátil para dar aula com PDF em computador ruim: roda direto do
pendrive, abre a apresentação **onde a última aula parou** e guarda um diário
de cada aula num `aula.txt`.

## Pendrive

```
PENDRIVE\
  _programas\
    SumatraPDF\SumatraPDF-3.6.1-64.exe   (portátil)
    vlc-3.0.21\vlc.exe                    (portátil)
  Hidráulica - Noite\                     ← uma pasta por turma
    AULA.exe
    01 Introducao.pdf
    02 Elevacao.pdf
    videos\
      TOCAR.exe                           ← cópia do AULA.exe (opcional)
      02-15 Bomba centrífuga.mp4
    aula.txt                              ← criado sozinho
```

- **SumatraPDF portátil:** <https://www.sumatrapdfreader.org/download-free-pdf-viewer> (versão *portable*, 64 bits).
- **VLC portátil:** <https://get.videolan.org/vlc/last/win64/> (arquivo `.zip`).
- `AULA.exe`: aba [Releases](../../releases).

## Uso

1. Dois cliques no `AULA.exe` da turma: abre em modo apresentação o PDF e a
   página certos. Os PDFs seguem a ordem do nome (`01`, `02`...); ao chegar
   na última página de um, a próxima aula começa no seguinte.
2. Ao fechar o Sumatra (Esc e depois Ctrl+Q, ou fechar a janela), a aula vai
   para o registro:

   ```
   2026-11-12 19:05-20:40 | 02 Elevacao.pdf | p. 12 -> 34
   2026-11-12 19:32 | vídeo | 02-15 Bomba centrífuga.mp4
   ```

   O `aula.txt` é texto comum: para mudar onde a próxima aula começa, basta
   editar a última linha.

## Vídeos

- **Link no slide** (PowerPoint → Inserir link → endereço):
  `aula://videos/02-15 Bomba centrífuga.mp4` toca no VLC do pendrive em tela
  cheia; `aula://videos/` abre a pasta no Explorer. Ao exportar para PDF o
  link continua valendo. O endereço `aula://` só existe enquanto o `AULA.exe`
  está aberto, registrado no usuário do Windows (sem administrador).
- O Sumatra ignora links de protocolos fora da lista dele; o `AULA.exe` cria
  `sumatrapdfrestrict.ini` ao lado do `SumatraPDF.exe` com `aula` liberado
  (e as demais permissões ligadas). Se você já tiver esse arquivo, ele só
  acrescenta `aula` em `LinkProtocols`.
- Links para arquivo ou pasta comuns (sem `aula://`) não tocam nada: o
  Sumatra só abre o Explorer com o arquivo selecionado.
- **Arrastar:** solte o vídeo sobre o `TOCAR.exe` (ou o `AULA.exe`). Clicar
  duas vezes no vídeo abre o player do PC, não o VLC.

## Compilar

```
cargo build --release                                   # no Windows
cargo build --release --target x86_64-pc-windows-gnu    # no Linux, com mingw-w64
```

O GitHub Actions compila e testa num Windows real com Sumatra e VLC
(`testes/windows.ps1`); tags `v*` publicam o `AULA.exe`.
