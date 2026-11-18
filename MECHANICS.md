# VisorCam — rules and values from the original

Sources:
- **Native program** `lib/arm64-v8a/libFunSDK.so` from split_config.arm64_v8a.apk (the vendor
  SDK that talks to cameras; sha256 625ea30d…). Opened with core_target_open, functions found
  with native_functions_search / native_strings_find and read with native_function_pseudocode
  (Ghidra). Addresses below are the built-in engine's (Ghidra shows them +0x100000).
- **Java/Kotlin code** of base.apk (com.xm.csee 7.9.6), which the native tools refuse; read
  with android_method_show (DEX offsets) and JADX 1.5.6. "Address" = class.method @offset.

## Camera login protocol — libFunSDK.so (used for cameras added by IP)
| Rule | Value | Function |
|---|---|---|
| Header | 20 bytes: 0xFF, version 1, 0, 0, SessionID u32, Sequence u32, Channel u8, EndFlag u8, MessageId u16, BodyLength u32 (little-endian) | `CProtocolNetIP::InitMsg` 0xfa0bd8 |
| Body | JSON text followed by one 0 byte; length counts the 0 | `NewLoginPTL` 0xfa4efc (len+1), `InitMsg` |
| Reply check | first byte must be 0xFF ("HeadFlag err" otherwise); length at offset 0x10 | `CProtocolNetIP::CheckPTL` 0xfa1674 |
| Login message | id 1000, JSON `EncryptType` "MD5", `LoginType` "DVRIP-Web", `UserName`, `PassWord` (in that order) | `NewLoginPTL` 0xfa4efc |
| Login reply | id 1001 (0x3e9); JSON `Ret`, `SessionID` read with "0x%x"; no `Ret` counts as success | `CProtocolNetIP::ParsePTL` 0xfa1de0 |
| Password | MD5, then for i in 0..8: n = (d[2i]+d[2i+1]) mod 62 → '0'+n (n<10), 'A'+n−10 (n<36), 'a'+n−36 — empty password gives "tlJwpbo6" | `XMMD5Encrypt` 0xf8f578 (via `MD5Encrypt` 0xf8f54c, `MD5EncryptEx` 0xf8fed0) |
| Session id in later messages | rewritten as text "0x%010x" for every message except 1000 | `InitMsg` 0xfa0bd8 |
| Device info | message 1020 (0x3fc) with `{"Name":"SystemInfo"}`; reply object `SystemInfo` (SerialNo, SoftWareVersion, BuildTime, DeviceRunTime, HardWare, DeviceModel — SystemInfoBean) | `CDeviceV2::UpdateDevConfig` 0xbe49d0, `DEVAPI::GetPTL_SystemInfo` 0xc27270 |
| Keep-alive | message 1006 (0x3ee), `{"Name":"KeepAlive","SessionID":"0x%010X"}` (not needed for one-shot checks) | `NewHeartbeatPacketPTL` 0xfa3d20 |
| Result codes | 100 → success; 106 and 203 → the same SDK error (-0x2700) = wrong user/password (shown "Senha incorreta"); other codes shown with their number | `CProtocolNetIP::ParseError` 0xfa3648 |
| Device state / speed | `FUN_SysGetDevState` 0xc48db8 queues message 0x1391 (5009); `FUN_MediaSetPlaySpeed` 0xc5d7f8 queues 0x157c (5500) with the speed level | both only forward to worker objects |

How the desktop app uses it: a camera added by IP is logged in to (2 s timeout, again every
30 s); the card shows Conectado / Off-line / Senha incorreta / Falha no login (código) /
Dispositivo incompatível; adding needs a successful login, and like
`AddDeviceByIPActivity.OnFunSDKResult` 5128 the device id becomes the SystemInfo SerialNo
(an existing serial gives "O dispositivo já existe"). Sobre o disp. shows the camera's own
serial, firmware, build date, model and run time ("0x…" read as minutes — unit estimated).
## Live video — libFunSDK.so (cameras added by IP)
| Rule | Value | Function |
|---|---|---|
| Request body | `{"Name":"OPMonitor","OPMonitor":{"Action":…,"Parameter":{"Channel":0,"CombinMode":"NONE","StreamType":…,"TransMode":"TCP"}},"SessionID":"0x%010X"}`; Claim adds `"Action1":"Start"` | `CProtocolNetIP::NewMediaControlPTL` 0xfa63b4 |
| Claim / Start | Claim = message 1413 (0x585) on a second connection; Start = 1410 (0x582) | `NewMediaControlPTL` 0xfa63b4 |
| Stream type | "Main" (HD) / "Extra1" (SD; index 1 is the default when out of range) | `NewMediaControlPTL` (table of strings), strings "Main"/"Extra1" |
| Media packets | message 1412 (0x584) | `CProtocolNetIP::IsDevMediaDataPTL` 0xfa15a0 |
| Frame sizes | 00 00 01 FC/FE: 16 + 24-bit LE at 12; FD: 8 + 24-bit at 4; F9/FA: 8 + u16 at 6; EC: byte4+5 + u32 at 0x14; ED: byte4+5 + 24-bit at 0xc; EE: byte4+5 + u32 at 0x10; EA: byte4+5 + u16 at 0x12; anything else = "Stream Data Loss" (resync) | `CSTDStream::GePacketLen` 0xb01c58 |
| Key-frame header (FC) | codec = byte4 & 0xF; fps = byte5 & 0x1F; width = byte6<<3 \| ((byte4>>4)&3)<<11; height = byte7<<3 \| (byte4>>6)<<11; date u32 at 8 | `CSTDStream::ParseHead` 0xb009a4 |
| Codec numbers | 0 MJPEG, 2 H.264, 3 H.265 (FFmpeg ids 7 / 0x1b / 0xad) | `CMediaFile::ToFFMpegCodecID` 0xd05bf4 |
| Keep-alive | message 1006 "KeepAlive" every 20 s on the command connection (period estimated) | `NewHeartbeatPacketPTL` 0xfa3d20 |

## PTZ on real cameras — NewPTZControlPTL + MonitorBottomPresenter
- Message 1400 (0x578), `{"Name":"OPPTZControl","OPPTZControl":{"Command":…,"Parameter":{"AUX":{"Number":0,"Status":"On"},"Channel":0,"MenuOpts":"Enter","POINT":{…0},"Pattern":"SetBegin","Preset":-1|65535,"Step":speed,"Tour":0}},"SessionID":"0x%010X"}`
  — Preset -1 starts a move, 65535 stops it (`CProtocolNetIP::NewPTZControlPTL` 0xfaa4b4).
- Commands: DirectionUp/Down/Left/Right/LeftUp/RightUp/LeftDown/RightDown, ZoomTile/ZoomWide,
  FocusNear/Far, IrisSmall/Large.
- Speed (`MonitorBottomPresenter.ptzCtrl`, `d0`): c = 1.0 below 4× picture zoom (0.75 from 4×),
  left/right Step = max(1, c×4), other directions max(1, c×2).
- The app sends start on press and stop on release over the live stream's command connection.
  Zoom in the app is digital (crops the picture), as the original's pinch zoom.

## Switching a camera to H.264 — DevEncodeConfigActivity + libFunSDK.so
- Config message numbers: `DEVAPI::GetCmdIdByCmdName` 0xc277cc — names outside its 14-row table
  read with 1042 (0x412) and write with 1040 (0x410); "EncodeCapability" is row 2 → 1360 (0x550)
  (table bytes at 0x207fbc0, names matched through their relocations). Bodies built by
  `CDeviceV2::GetDevConfigJson` 0xc0614c / `SetDevConfigJson` 0xc06894: `{"Name":X, X:[…]}`.
- `EncodeCapability.EncodeInfo[0].CompressionMask`: bit n = `ResolutionInfo.getEncodeMode(n)`;
  bit 7 = "H.264", bit 8 = "H.265" ("H.265X" also counts as 7).
- The original sets `Simplify.Encode` → `MainFormat.Video.Compression` and writes it back
  (`DevEncodeConfigActivity`). The desktop app does the same and also changes ExtraFormat when it
  is H.265 (difference, so the SD stream works too); refuses when the mask has no H.264 bit.
- When the HD stream is H.265 the app first tries the SD stream automatically.

H.265 (codec 3) is decoded by the free FFmpeg program when it is on this computer (next to the
app, on PATH or in C:\ffmpeg\bin): frames go to `ffmpeg -f hevc -i pipe:0 … -f rawvideo
-pix_fmt rgb24 pipe:1`, scaled to the header's width × height, with no window. The original uses
its own built-in FFmpeg (CFFMpegDec via ToFFMpegCodecID 3 → 0xad, `VideoDecManager::NewDecode`
0xce7214); this app can't ship that library, so it uses the installed program. The person's camera
(192.168.18.140) reports a CompressionMask without the H.264 bit, so switching to H.264 is refused
there, and FFmpeg is what shows its picture.

The desktop app decodes H.264 (OpenH264) and MJPEG; an H.265 camera gets a clear message
("mude a codificação para H.264"). Snapshots of real cameras save the decoded picture; recording
real cameras to the computer is not built yet. Tested against encoded H.264 samples, not yet
against a real camera.

## Adding a device by IP — `AddDeviceByIPActivity.K1` (classes5.dex)
| Rule | Value | Where |
|---|---|---|
| Name length (checked first) | ≤ 32 if every char matches `v0.e`, else ≤ 21; message `devname_null` "Digitar nome do dispositivo (1 e %s caracteres)" | K1 @0x004d–0x009a, `com.xworld.utils.v0.e` @0x0008 regex `^[(/\s/A-Za-z0-9!@#$%^&*()_|\[\]{}?/~.<>,';:-]+$` |
| IP check (second) | strict IPv4 only, `^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(…)$`; domain names are refused here; message "Por favor, insira um IP" | K1 @0x009b, `g3.f.X0` @0x0004 |
| Port | digits only (`g3.f.Z0` `[0-9]*`) and parseable, else 34567 (0x8707), written back into the field | K1 @0x00c5–0x00f6, `g3.f.Z0` @0x0000 |
| Empty name | becomes the IP | K1 @0x0126 |
| Login | empty, "default", "666666", "guest" → "admin" | K1 @0x0140–0x017f |
| Device id | `ip:port` | K1 @0x0107 |
| Add button | enabled only while the IP field is not empty | `AddDeviceByIPActivity$b.afterTextChanged` |
| Device must answer | SysGetDevState; result 5009 arg1>0 → added ("Dispositivo adicionado"), else "O dispositivo pode estar off-line. Tente mais tarde" (Device_NotExist); already in list → "O dispositivo já existe" | `OnFunSDKResult` 5009/5004, `F1` |
| Wrong login | -11301/-11318 "Senha incorreta", -11302 "Usuário inexistente" | `OnFunSDKResult` 5128 |

The desktop version answers "does the device respond" with a TCP connection to ip:port
(timeout 2 s), since the vendor SDK is not available.

## Alarm settings — `DevVideoAlarmSetActivity`, `AlarmIntervalDlg`
- Sensitivity slider 0–100 (`onProgressChanged`): label High if >75, Middle if >25, else Low.
- On release (`onStopTrackingTouch`): snaps to 97 / 50 / 3 and stores device Level 6 / 3 / 1.
- Reading back (`J1`): Level ≥5 → 96 High, ≥3 → 50 Middle, else 4 Low.
- Alarm interval (`AlarmIntervalDlg.p`): slider setMax(1770), value = progress + 30 s
  → 30 s … 1800 s (30 min), shown mm:ss ("Intervalo (mm:ss)"); a stored value under 30 is
  shown as 30 (`DevVideoAlarmSetActivity.OnFunSDKResult` 5131, `AlarmIntervalDlg.n`).

## Playback speed — `PlayBackActivity.onSpeedPlay` (classes5.dex)
- Level 0 = normal (dir 0 @0x0007). Slow (dir 2 @0x0015): if −2 < level < 0 then level−1,
  else −1. Fast (dir 1 @0x0048): if 0 < level < 2 then level+1, else 1.
- Label = "speed_slow_play"/"speed_fast_play" + level×2 + "x" (@0x0037, @0x0068):
  "Avanço lento -2x/-4x", "Avanço rápido 2x/4x". Layout layout_playback_speed.xml lists
  -4X, -2X, 1X, 2X, 4X. Level n plays at 2^n (¼, ½, 1, 2, 4) — speed factor per level is
  passed to the SDK (`MediaSetPlaySpeed`), so the 2^n mapping is estimated.

## Record settings — `DevRecordSettingActivity`, devset_record.xml
- File length slider 5…120 min: `onProgressChanged` @0x0020 adds 5 to progress; layout
  seekBarBottomLeftText=5, seekBarBottomRightText=120. Default 60 min is estimated.
- Modes: TR_Record_Always / TR_Record_Alarm / TR_Record_Close.

## Storage — `DevStorageSettingActivity.U1`, `T1`, `V`, `V1`
- Total = Σ TotalSpace, free = Σ RemainSpace over partitions; DirverType 0 or 5 = video
  partition, 4 = picture partition. Free % = min(remain×100/total, 100), "#0.0".
- Sizes (MB) formatted by `com.xworld.utils.E.b(size, 2)`: 0 → "0.00M"; <1024 → "#.00M";
  <1024² → /1024 "G"; <1024³ → /1024² "T"; else "P".
- Full: "OverWrite" = Gravação cíclica, "StopRecord" = Parar gravação (`T1`).
- Format: confirm "Esta ação excluirá todos os arquivos no cartão de memória. Continuar?"
  (format_tip), clears every partition (OPStorageManager Clear/Data, `V1`), then
  "Formatado com sucesso" (format_s); with no card "Nenhum cartão de memória detectado".
- Card size 29.7 GB and bitrates 0.46 / 0.23 GB/h are estimated (they come from the device).

## Basic settings — `DevBasicSettingActivity`, `DevDayNightModeActivity`
- Speaker / MIC volume sliders setMax(100) (`DevBasicSettingActivity` lines for a0/b0).
- Day/night mode (Camera.ParamEx DayNightSwitch.SwitchMode): 0 "Troca automática",
  1 "Modo diurno", 2 "Modo noturno", 3 "Troca programada" (start/end time)
  (`DevDayNightModeActivity.O1/P1/Q1/R1`, activity_day_night_mode.xml).
- Auto switch sensitivity 0–10: progress = (50 − DncThr)/4 (`DevBasicSettingActivity.y2`,
  setMax(10)). How DncThr changes the switching point lives in the camera; the app uses
  "night when daylight < 0.10 + 0.05 × sensitivity" (estimated).
- Image flip / mirror: PictureFlip / PictureMirror (`DevBasicSettingActivity` @1135).

## Live view — `XMMediaPlayer`
- Snapshot file: `<devId>_<channel>_<currentTimeMillis>.jpg`, name passed through
  `E.i` which removes spaces and ':' (`XMMediaPlayer.i` @0x003a–0x0085, `E.i` @0x0006).
- Recording file: `<devId>_<channel>_<millis>.mp4` (`XMMediaPlayer.S1` @0x0069–0x00f3); the
  timer ticks every 1000 ms (@0x0121). This version stores a cover picture
  `…_mp4.png` and replays the scene instead of writing MP4.
- Snapshot and record only while video plays, else "Você poderá realizar as ações somente
  quando o vídeo estiver reproduzindo..." (TR_Need_Video_Play_Can_Operation, S1 @0x0011,
  i @0x0010). Pausing stops a running recording.

## Messages — `AlarmPicVideoShowActivity`
- Search period at most 7 days (`selectedDates.size() > 7` → TR_Search_Info_Max_Tip "O período
  de consulta não deve ser superior a sete dias"); the app keeps 7 days.

## Screens and colours
- Tabs (activity_main.xml): Dispositivo · Mensagem · (Serviço hidden) · Eu; desktop adds Álbum.
- Device card (adapter_device_list.xml): name pill "Camera01", offline overlay + Ajuda,
  row Compartilhar / Mensagem / Conf. (+ Proteção = main_dev_mode).
- Colours (colors.xml): #00BFA5 theme, #F5F5F5 background, #333/#666/#999 text,
  #F04438 alert, #FF9500 warning.

## Simulation (no vendor cloud) — estimated
Demo cameras draw a scene from (seed, time); every 10-minute slot has a 22% chance of a
movement (person 14 s, pet/car 7 s). Live view, recordings, alarm messages and album
replays use the same function. Alarm recording 5 s before / 30 s after (estimated).

## Outros tipos de câmera (RTSP)

Não vem do app original: é uma extensão deste app. Uma câmera RTSP guarda só um nome e o
endereço. O FFmpeg abre o endereço (`-rtsp_transport tcp`, sem buffer de entrada) e envia
quadros RGB de 1280×720, com a imagem reduzida para caber e faixas pretas quando a
proporção é outra. A câmera aparece "On-line" até o vídeo falhar; clicar no cartão tenta
de novo. Endereços aceitos: rtsp://, rtsps://, http:// e https:// com um host. O código de
compartilhamento de uma câmera RTSP leva o endereço inteiro (com a senha, se houver).

## QR code (SnAddDevActivity.onActivityResult)

O original lê o resultado "Scan_QrCode_Result": se for um número de série válido (g3.f.j1)
usa-o direto; sem "http" passa por FunSDK.DecDevInfo ("sn,usuário,senha,tipo[,hora]", que
expira em 1800 s); com link lê "shareInfo" (DecGeneralDevInfo) ou "sv" (DecQRCodeDevInfo,
chaves sn/user/pwd). Os decodificadores são da biblioteca fechada, então aqui só valem o
número de série puro (g3.f.j1: exatamente 16 caracteres hexadecimais `^[a-fA-F0-9]{16}$` ou 20 letras e dígitos `^[a-zA-Z0-9]{20}$`) e links com "sn=". Também aceitos:
códigos VisorCam "VC1-" e endereços rtsp://. Fotos grandes são reduzidas a 1600 px antes da
leitura.

Códigos curtos de compartilhamento do iCSee (ex.: 13 letras e números): GoogleScanQRCodeActivity
tenta FunSDK.DecDevInfo (XDeCodeStr, decodificação local em base 62); se não sai
"sn,usuário,senha,tipo", chama requestCheckCode, que pergunta ao servidor do fabricante
(`https://tools.xmeye.net/deviceSuperPassword?text=..&uuid=..&appkey=..`, g3.f.m0). Este app
não usa a nuvem nem a chave do fabricante, então explica isso e indica a rede local ou o IP.
