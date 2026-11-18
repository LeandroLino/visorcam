# VisorCam

Aplicativo de código aberto para ver câmeras de segurança no computador: lista de
dispositivos, vídeo ao vivo (H.264, MJPEG e H.265), PTZ, instantâneos, reprodução,
mensagens de alarme, álbum e configurações. Inclui câmeras de demonstração simuladas.

Câmeras reais são acessadas pela rede local (IP, porta 34567). A nuvem do fabricante não é
usada.

## Compilar

Precisa do [Rust](https://rustup.rs) e de um compilador C/C++ (o decodificador H.264 é
compilado junto).

- **Windows:** instale o Rust e o "Visual Studio Build Tools" (C++). Depois:
  `cargo build --release` → `target\release\visorcam.exe`
- **Linux:** instale o Rust e os pacotes de compilação (Debian/Ubuntu:
  `sudo apt install build-essential pkg-config libx11-dev libxkbcommon-dev libgl1-mesa-dev`).
  Depois: `cargo build --release` → `target/release/visorcam`

Para vídeo **H.265**, instale o programa gratuito [FFmpeg](https://ffmpeg.org) (no PATH, ao
lado do app ou em `C:\ffmpeg\bin`). Sem ele, H.264 e MJPEG continuam funcionando.

Onde ficam os dados:
- Windows: `%APPDATA%\VisorCam` e fotos em `Imagens\VisorCam`
- Linux: `~/.local/share/visorcam` e fotos em `~/Pictures/VisorCam`

## Opções

- `--screenshot arquivo.png` abre, desenha a janela e salva uma imagem dela (usado em testes).
- `cargo test` roda os testes (protocolo, vídeo, regras).

## Como funciona

As regras (protocolo da câmera, formatos de vídeo, valores de configuração) foram estudadas
no app original e estão descritas em `MECHANICS.md`, com a origem de cada valor.

## Próximos passos

- Versão Android (.apk): o código da interface (egui) pode rodar no Android, mas precisa de
  um projeto Android separado e de adaptações para tela de toque.

## Publicar no GitHub

Esta pasta já é um repositório Git (ramo `main`). Para publicar:

1. Entre na sua conta: `gh auth login`
2. Crie o repositório público e envie o código (dentro desta pasta):
   `gh repo create visorcam --public --source . --remote origin --push`

Sem o `gh`: crie um repositório vazio em github.com/new e rode
`git remote add origin https://github.com/SEU_USUARIO/visorcam.git` e `git push -u origin main`.

Depois de mudar algo: `git add -A`, `git commit -m "o que mudou"`, `git push`.

## Licença

MIT — veja `LICENSE`.
