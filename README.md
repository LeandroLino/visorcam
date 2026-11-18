# VisorCam

Aplicativo de código aberto para ver câmeras de segurança no computador: lista de
dispositivos, vídeo ao vivo (H.264, MJPEG e H.265), PTZ, instantâneos, reprodução,
mensagens de alarme, álbum e configurações. Inclui câmeras de demonstração simuladas.

Câmeras reais são acessadas pela rede local (IP, porta 34567). A nuvem do fabricante não é
usada.

## Tipos de câmera

Cada câmera tem um tipo (`DevKind` em `src/model.rs`), e o vídeo de cada tipo é aberto em
`src/stream.rs`:

- **IP (protocolo XM/Sofia, porta 34567):** as câmeras do app original. Login, vídeo,
  PTZ e informações do aparelho.
- **RTSP:** câmeras de outras marcas (Intelbras, Hikvision, Dahua, Tapo, Reolink...) pelo
  endereço `rtsp://usuario:senha@IP:554/...`. O vídeo é aberto pelo FFmpeg; PTZ ainda não.

Para um tipo novo (ONVIF, outro app de câmera...): acrescente uma variante em `DevKind`, um
`LiveStream::start_...` em `stream.rs`, uma aba em "Adicionar dispositivo" (`devices.rs`) e
o estado da conexão em `App::dev_state` (`app.rs`).

## Câmeras de vários apps

A lista de dispositivos junta câmeras de todos os apps e mostra um filtro por app quando há
mais de um. Para trazer as câmeras de outro projeto, ele salva um arquivo `.json` neste
formato e você usa **Importar câmeras de outro app** (o **Exportar lista** faz o inverso):

```json
{
  "format": "visorcam-cameras",
  "version": 1,
  "app": "Nome do outro app",
  "cameras": [
    { "name": "Sala", "type": "ip", "host": "192.168.1.10", "port": 34567, "user": "admin", "password": "" },
    { "name": "Garagem", "type": "rtsp", "url": "rtsp://admin:senha@192.168.1.20:554/stream1" }
  ]
}
```

Tipos aceitos: `ip` (protocolo destas câmeras) e `rtsp`. Câmeras já existentes não são
duplicadas. Um tipo novo entra como descrito em "Tipos de câmera".

## QR code

"Adicionar dispositivo › QR code" lê o QR de uma foto ou imagem (escolhida ou arrastada para a
janela): o código de compartilhamento do VisorCam adiciona a câmera; um endereço `rtsp://`
abre a aba RTSP; o número de série da etiqueta da câmera vira o nome, e falta só achar o IP na
rede. Os QR criptografados do app original (nuvem do fabricante) não podem ser lidos. Em
"Compartilhar", o QR mostrado é real e pode ser salvo como imagem.

## Compilar

Precisa do [Rust](https://rustup.rs) e de um compilador C/C++ (o decodificador H.264 é
compilado junto).

- **Windows:** instale o Rust e o "Visual Studio Build Tools" (C++). Depois:
  `cargo build --release` → `target\release\visorcam.exe`
- **Linux:** instale o Rust e os pacotes de compilação (Debian/Ubuntu:
  `sudo apt install build-essential pkg-config libx11-dev libxkbcommon-dev libgl1-mesa-dev`).
  Depois: `cargo build --release` → `target/release/visorcam`

Para vídeo **H.265** e câmeras **RTSP**, instale o programa gratuito [FFmpeg](https://ffmpeg.org) (no PATH, ao
lado do app ou em `C:\ffmpeg\bin`). Sem ele, H.264 e MJPEG das câmeras IP continuam funcionando.

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
