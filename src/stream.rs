//! Real live video from a camera added by IP, following libFunSDK.so:
//! - request: CProtocolNetIP::NewMediaControlPTL @0xfa63b4 ("OPMonitor", Claim 1413 / Start 1410)
//! - media packets: IsDevMediaDataPTL @0xfa15a0 (1412)
//! - frame layout: CSTDStream::GePacketLen @0xb01c58 and CSTDStream::ParseHead @0xb009a4
//! - codec numbers: CMediaFile::ToFFMpegCodecID @0xd05bf4 (0 MJPEG, 2 H.264, 3 H.265)

use crate::net::{self, packet, NetState};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MSG_MONITOR_START: u16 = 1410; // 0x582
pub const MSG_MONITOR_DATA: u16 = 1412; // 0x584
pub const MSG_MONITOR_CLAIM: u16 = 1413; // 0x585
pub const MSG_KEEPALIVE: u16 = 1006; // 0x3ee
/// OPPTZControl (CProtocolNetIP::NewPTZControlPTL @0xfaa4b4 passes 0x578).
pub const MSG_PTZ: u16 = 1400;

#[derive(Clone, Debug, PartialEq)]
pub struct PtzCmd {
    pub command: String,
    pub stop: bool,
    pub step: i32,
}

/// OPPTZControl body as NewPTZControlPTL builds it: Preset -1 starts, 65535 stops; Step = speed.
pub fn ptz_body(c: &PtzCmd, session: u32) -> String {
    serde_json::json!({
        "Name": "OPPTZControl",
        "OPPTZControl": {
            "Command": c.command,
            "Parameter": {
                "AUX": { "Number": 0, "Status": "On" },
                "Channel": 0,
                "MenuOpts": "Enter",
                "POINT": { "bottom": 0, "left": 0, "right": 0, "top": 0 },
                "Pattern": "SetBegin",
                "Preset": if c.stop { 65535 } else { -1 },
                "Step": c.step,
                "Tour": 0,
            }
        },
        "SessionID": format!("0x{:010X}", session),
    })
    .to_string()
}

/// PTZ speed like MonitorBottomPresenter: c = 1.0 below 4× zoom (0.75 at 4× and more);
/// left/right use max(1, c×4), the other directions max(1, c×2).
pub fn ptz_step(command: &str, zoom: f32) -> i32 {
    let c = if zoom >= 4.0 { 0.75 } else { 1.0 };
    let horizontal = command == "DirectionLeft" || command == "DirectionRight";
    ((if horizontal { c * 4.0 } else { c * 2.0 }) as i32).max(1)
}
/// Keep-alive period on the command connection (estimated; the SDK schedules it elsewhere).
pub const KEEPALIVE_SECS: u64 = 20;
/// How long to wait for video before giving up.
pub const DATA_TIMEOUT_SECS: u64 = 10;

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Connecting,
    Playing { codec: &'static str, w: usize, h: usize, fps: u32 },
    /// The camera sends a format this version can't draw.
    Unsupported(String),
    Failed(String),
}

pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
    pub seq: u64,
}

pub struct LiveStream {
    pub frame: Arc<Mutex<Option<Frame>>>,
    pub status: Arc<Mutex<Status>>,
    stop: Arc<AtomicBool>,
    /// PTZ commands waiting to go out on the command connection.
    ptz: Arc<Mutex<Vec<PtzCmd>>>,
    pub last_used: Instant,
}

impl Drop for LiveStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl LiveStream {
    /// main: true for "Main" (HD), false for "Extra1" (SD; the SDK's default index 1).
    pub fn start(host: &str, port: u16, user: &str, pw: &str, main: bool) -> LiveStream {
        let frame = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(Status::Connecting));
        let stop = Arc::new(AtomicBool::new(false));
        let (h, u, p) = (host.to_string(), user.to_string(), pw.to_string());
        let ptz = Arc::new(Mutex::new(Vec::new()));
        let (f2, s2, st2, q2) = (frame.clone(), status.clone(), stop.clone(), ptz.clone());
        std::thread::spawn(move || {
            if let Err(e) = run(&h, port, &u, &p, main, &f2, &s2, &st2, &q2) {
                if !st2.load(Ordering::SeqCst) {
                    if let Ok(mut s) = s2.lock() {
                        if !matches!(*s, Status::Unsupported(_)) {
                            *s = Status::Failed(e);
                        }
                    }
                }
            }
        });
        LiveStream { frame, status, stop, ptz, last_used: Instant::now() }
    }

    /// Queue a PTZ start or stop (sent by the stream's command-connection thread).
    pub fn ptz(&self, command: &str, stop: bool, step: i32) {
        if let Ok(mut q) = self.ptz.lock() {
            q.push(PtzCmd { command: command.to_string(), stop, step });
        }
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or(Status::Connecting)
    }
}

fn state_text(s: NetState) -> String {
    match s {
        NetState::Offline => "O dispositivo pode estar off-line. Tente mais tarde".into(),
        NetState::WrongPassword => "Senha incorreta".into(),
        NetState::Refused(c) => format!("O dispositivo recusou o pedido (código {c})"),
        NetState::NotCamera => "Este endereço não responde como uma câmera compatível".into(),
        _ => "Falha na conexão".into(),
    }
}

/// OPMonitor body as NewMediaControlPTL builds it for live view (type 0).
pub fn monitor_body(action: &str, claim: bool, main: bool, session: u32) -> String {
    let mut op = serde_json::Map::new();
    op.insert("Action".into(), action.into());
    if claim {
        op.insert("Action1".into(), "Start".into());
    }
    op.insert(
        "Parameter".into(),
        serde_json::json!({
            "Channel": 0,
            "CombinMode": "NONE",
            "StreamType": if main { "Main" } else { "Extra1" },
            "TransMode": "TCP",
        }),
    );
    serde_json::json!({
        "Name": "OPMonitor",
        "OPMonitor": serde_json::Value::Object(op),
        "SessionID": format!("0x{:010X}", session),
    })
    .to_string()
}

#[allow(clippy::too_many_arguments)]
fn run(
    host: &str,
    port: u16,
    user: &str,
    pw: &str,
    main: bool,
    frame: &Arc<Mutex<Option<Frame>>>,
    status: &Arc<Mutex<Status>>,
    stop: &Arc<AtomicBool>,
    ptz: &Arc<Mutex<Vec<PtzCmd>>>,
) -> Result<(), String> {
    use std::io::Write;
    let t = Duration::from_secs(4);
    let (mut cmd, session, _) = net::open_session(host, port, user, pw, t).map_err(state_text)?;
    // second connection carries the media (Claim)
    let mut media = net::connect(host, port, t).map_err(state_text)?;
    media
        .write_all(&packet(session, 0, MSG_MONITOR_CLAIM, &monitor_body("Claim", true, main, session)))
        .map_err(|_| "Falha ao pedir o vídeo".to_string())?;
    net::reply_ret(&mut media).map_err(state_text)?;
    cmd.write_all(&packet(session, 2, MSG_MONITOR_START, &monitor_body("Start", false, main, session)))
        .map_err(|_| "Falha ao pedir o vídeo".to_string())?;
    net::reply_ret(&mut cmd).map_err(state_text)?;

    // keep the command connection alive (NewHeartbeatPacketPTL: "KeepAlive") and send PTZ
    let stop_ka = stop.clone();
    let ptz_q = ptz.clone();
    let mut cmd_ka: TcpStream = cmd.try_clone().map_err(|e| e.to_string())?;
    let _ = cmd_ka.set_read_timeout(Some(Duration::from_millis(1500)));
    std::thread::spawn(move || {
        let mut seq = 3u32;
        let mut last = Instant::now();
        while !stop_ka.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(40));
            let cmds: Vec<PtzCmd> = ptz_q.lock().map(|mut q| q.drain(..).collect()).unwrap_or_default();
            for c in cmds {
                if cmd_ka.write_all(&packet(session, seq, MSG_PTZ, &ptz_body(&c, session))).is_err() {
                    return;
                }
                seq += 1;
                let _ = net::read_raw(&mut cmd_ka);
            }
            if last.elapsed() >= Duration::from_secs(KEEPALIVE_SECS) {
                last = Instant::now();
                let body = serde_json::json!({ "Name": "KeepAlive", "SessionID": format!("0x{:010X}", session) }).to_string();
                if cmd_ka.write_all(&packet(session, seq, MSG_KEEPALIVE, &body)).is_err() {
                    break;
                }
                seq += 1;
                let _ = net::read_raw(&mut cmd_ka);
            }
        }
    });

    let _ = media.set_read_timeout(Some(Duration::from_secs(DATA_TIMEOUT_SECS)));
    let mut parser = FrameParser::default();
    let mut dec = Decoder::new();
    let mut seq = 0u64;
    while !stop.load(Ordering::SeqCst) {
        let (msg, body) = net::read_raw(&mut media).map_err(|_| "A câmera parou de enviar vídeo".to_string())?;
        if msg != MSG_MONITOR_DATA {
            continue;
        }
        parser.push(&body);
        while let Some(vf) = parser.next_frame() {
            match dec.decode(&vf) {
                Ok(Some((w, h, rgb))) => {
                    seq += 1;
                    if let Ok(mut f) = frame.lock() {
                        *f = Some(Frame { w, h, rgb, seq });
                    }
                    if let Ok(mut s) = status.lock() {
                        *s = Status::Playing { codec: codec_name(vf.codec), w, h, fps: dec.fps.max(vf.fps) };
                    }
                }
                Ok(None) => {
                    if vf.fps > 0 {
                        dec.fps = vf.fps;
                    }
                }
                Err(e) => {
                    if let Ok(mut s) = status.lock() {
                        *s = Status::Unsupported(e);
                    }
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

pub fn codec_name(c: u8) -> &'static str {
    match c {
        0 => "MJPEG",
        2 => "H.264",
        3 => "H.265",
        _ => "?",
    }
}

/// One video frame taken out of the stream.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoFrame {
    pub key: bool,
    pub codec: u8,
    pub fps: u32,
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

/// Splits the media byte stream into frames (CSTDStream::GePacketLen / ParseHead).
#[derive(Default)]
pub struct FrameParser {
    buf: Vec<u8>,
    codec: u8,
    fps: u32,
    w: usize,
    h: usize,
}

fn u24(b: &[u8], i: usize) -> usize {
    b[i] as usize | (b[i + 1] as usize) << 8 | (b[i + 2] as usize) << 16
}
fn u32le(b: &[u8], i: usize) -> usize {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize
}

impl FrameParser {
    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
        if self.buf.len() > 16 << 20 {
            self.buf.clear();
        }
    }

    /// Size of the frame at the start of the buffer, and where its data begins.
    /// None: need more bytes. Some(Err): not a frame start ("Stream Data Loss").
    fn frame_len(&self) -> Option<Result<(usize, usize), ()>> {
        let b = &self.buf;
        if b.len() < 24 {
            return None;
        }
        if b[0] != 0 || b[1] != 0 || b[2] != 1 {
            return Some(Err(()));
        }
        Some(Ok(match b[3] {
            0xFC | 0xFE => (u24(b, 12) + 16, 16),
            0xFD => (u24(b, 4) + 8, 8),
            0xF9 | 0xFA => ((u16::from_le_bytes([b[6], b[7]]) as usize) + 8, 8),
            0xEA => ((u16::from_le_bytes([b[0x12], b[0x13]]) as usize) + b[4] as usize + 5, b[4] as usize + 5),
            0xEC => (u32le(b, 0x14) + b[4] as usize + 5, b[4] as usize + 5),
            0xED => (u24(b, 0xc) + b[4] as usize + 5, b[4] as usize + 5),
            0xEE => (u32le(b, 0x10) + b[4] as usize + 5, b[4] as usize + 5),
            _ => return Some(Err(())),
        }))
    }

    pub fn next_frame(&mut self) -> Option<VideoFrame> {
        loop {
            match self.frame_len()? {
                Err(()) => {
                    // resynchronise on the next 00 00 01 Ex/Fx marker
                    let pos = self.buf.windows(4).skip(1).position(|w| w[0] == 0 && w[1] == 0 && w[2] == 1 && (w[3] >= 0xE0));
                    match pos {
                        Some(p) => {
                            self.buf.drain(..p + 1);
                        }
                        None => {
                            let keep = self.buf.len().saturating_sub(3);
                            self.buf.drain(..keep);
                            return None;
                        }
                    }
                }
                Ok((total, head)) => {
                    if self.buf.len() < total {
                        return None;
                    }
                    let b: Vec<u8> = self.buf.drain(..total).collect();
                    let kind = b[3];
                    match kind {
                        0xFC | 0xFE => {
                            // ParseHead: codec byte4 & 0xf, fps byte5 & 0x1f, size from bytes 6/7 and byte4
                            self.codec = b[4] & 0x0f;
                            self.fps = (b[5] & 0x1f) as u32;
                            self.w = ((b[6] as usize) << 3) | (((b[4] >> 4) & 3) as usize) << 11;
                            self.h = ((b[7] as usize) << 3) | ((b[4] >> 6) as usize) << 11;
                            return Some(self.video(true, &b[head..]));
                        }
                        0xEC | 0xEE => {
                            let v = if kind == 0xEC { u32le(&b, 0xc) } else { u32le(&b, 8) };
                            self.codec = ((v >> 26) & 0xf) as u8;
                            self.fps = ((v >> 20) & 0x3f) as u32;
                            self.w = (v & 0x3ff) << 3;
                            self.h = ((v >> 10) & 0x3ff) << 3;
                            return Some(self.video(true, &b[head..]));
                        }
                        0xFD | 0xED => return Some(self.video(false, &b[head..])),
                        _ => {} // audio and information frames are skipped
                    }
                }
            }
        }
    }

    fn video(&self, key: bool, data: &[u8]) -> VideoFrame {
        VideoFrame { key, codec: self.codec, fps: self.fps, w: self.w, h: self.h, data: data.to_vec() }
    }
}

pub struct Decoder {
    h264: Option<openh264::decoder::Decoder>,
    hevc: Option<HevcPipe>,
    got_key: bool,
    pub fps: u32,
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder { h264: None, hevc: None, got_key: false, fps: 0 }
    }

    /// Ok(Some(w, h, rgb)) for a picture, Ok(None) when nothing to show yet,
    /// Err(text) when the camera's format can't be drawn here.
    pub fn decode(&mut self, f: &VideoFrame) -> Result<Option<(usize, usize, Vec<u8>)>, String> {
        if f.key {
            self.got_key = true;
            if f.fps > 0 {
                self.fps = f.fps;
            }
        }
        if !self.got_key {
            return Ok(None);
        }
        match f.codec {
            2 => {
                if self.h264.is_none() {
                    self.h264 = Some(openh264::decoder::Decoder::new().map_err(|e| format!("Decodificador H.264: {e}"))?);
                }
                let dec = self.h264.as_mut().unwrap();
                let mut out = None;
                for nal in openh264::nal_units(&f.data) {
                    if let Ok(Some(yuv)) = dec.decode(nal) {
                        use openh264::formats::YUVSource;
                        let (w, h) = yuv.dimensions();
                        let mut rgb = vec![0u8; w * h * 3];
                        yuv.write_rgb8(&mut rgb);
                        out = Some((w, h, rgb));
                    }
                }
                Ok(out)
            }
            0 => match image::load_from_memory(&f.data) {
                Ok(img) => {
                    let img = img.to_rgb8();
                    Ok(Some((img.width() as usize, img.height() as usize, img.into_raw())))
                }
                Err(_) => Ok(None),
            },
            3 => {
                if f.w == 0 || f.h == 0 {
                    return Ok(None);
                }
                let restart = self.hevc.as_ref().map(|p| (p.w, p.h) != (f.w, f.h) || p.dead()).unwrap_or(true);
                if restart {
                    self.hevc = Some(HevcPipe::start(f.w, f.h)?);
                }
                let pipe = self.hevc.as_mut().unwrap();
                pipe.write(&f.data)?;
                Ok(pipe.take())
            }
            c => Err(format!("A câmera envia um formato de vídeo desconhecido (código {c}).")),
        }
    }
}

/// Where the free FFmpeg program is: next to this app, on PATH, or in C:\ffmpeg\bin.
pub fn find_ffmpeg() -> Option<std::path::PathBuf> {
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf());
            dirs.push(d.join("ffmpeg"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.push(std::path::PathBuf::from(r"C:\ffmpeg\bin"));
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

pub const NO_FFMPEG: &str = r"A câmera envia vídeo H.265. Para mostrá-lo, o app usa o programa gratuito FFmpeg, que não foi encontrado neste computador (procure por ffmpeg.exe ao lado do app, no PATH ou em C:\ffmpeg\bin).";

/// H.265 pictures decoded by an FFmpeg child process: Annex-B frames go in through stdin,
/// RGB pictures (w × h × 3 bytes) come back through stdout and the newest is kept.
pub struct HevcPipe {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    out: Arc<Mutex<(Option<Vec<u8>>, u64)>>,
    alive: Arc<AtomicBool>,
    last: u64,
    pub w: usize,
    pub h: usize,
}

impl HevcPipe {
    pub fn start(w: usize, h: usize) -> Result<HevcPipe, String> {
        use std::io::Read;
        use std::process::{Command, Stdio};
        let exe = find_ffmpeg().ok_or_else(|| NO_FFMPEG.to_string())?;
        let mut cmd = Command::new(exe);
        cmd.args([
            "-hide_banner", "-loglevel", "error",
            "-fflags", "nobuffer", "-flags", "low_delay",
            "-probesize", "32", "-analyzeduration", "0", "-threads", "1",
            "-f", "hevc", "-i", "pipe:0",
            "-vf", &format!("scale={w}:{h}"),
            "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| format!("Não foi possível abrir o FFmpeg: {e}"))?;
        let stdin = child.stdin.take().ok_or("FFmpeg sem entrada")?;
        let mut stdout = child.stdout.take().ok_or("FFmpeg sem saída")?;
        let out = Arc::new(Mutex::new((None, 0u64)));
        let alive = Arc::new(AtomicBool::new(true));
        let (o2, a2) = (out.clone(), alive.clone());
        std::thread::spawn(move || {
            let mut buf = vec![0u8; w * h * 3];
            while stdout.read_exact(&mut buf).is_ok() {
                if let Ok(mut m) = o2.lock() {
                    m.0 = Some(buf.clone());
                    m.1 += 1;
                }
            }
            a2.store(false, Ordering::SeqCst);
        });
        Ok(HevcPipe { child, stdin, out, alive, last: 0, w, h })
    }

    pub fn dead(&self) -> bool {
        !self.alive.load(Ordering::SeqCst)
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), String> {
        use std::io::Write;
        self.stdin.write_all(data).map_err(|_| "O FFmpeg parou de decodificar o vídeo H.265".to_string())
    }

    /// The newest picture, if one arrived since the last call.
    pub fn take(&mut self) -> Option<(usize, usize, Vec<u8>)> {
        let m = self.out.lock().ok()?;
        if m.1 == self.last {
            return None;
        }
        self.last = m.1;
        m.0.clone().map(|rgb| (self.w, self.h, rgb))
    }
}

impl Drop for HevcPipe {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_frames_across_packets() {
        // I-frame: 00 00 01 FC, codec 2 (H.264), fps 25, w=640 (80<<3), h=360 (45<<3)
        let payload = [0u8, 0, 0, 1, 0x67, 1, 2, 3];
        let mut i = vec![0, 0, 1, 0xFC, 0x02, 25, 80, 45, 0, 0, 0, 0];
        i.extend_from_slice(&[payload.len() as u8, 0, 0, 0]);
        i.extend_from_slice(&payload);
        let mut p = vec![0, 0, 1, 0xFD, 3, 0, 0, 0];
        p.extend_from_slice(&[9, 9, 9]);
        let mut a = vec![0, 0, 1, 0xFA, 0x0E, 2, 4, 0];
        a.extend_from_slice(&[7, 7, 7, 7]);
        let mut all = i.clone();
        all.extend(a);
        all.extend(p);
        all.extend(vec![0u8; 30]); // padding so the last header can be read
        let mut fp = FrameParser::default();
        fp.push(&all[..10]);
        assert!(fp.next_frame().is_none());
        fp.push(&all[10..]);
        let f1 = fp.next_frame().unwrap();
        assert!(f1.key);
        assert_eq!((f1.codec, f1.fps, f1.w, f1.h), (2, 25, 640, 360));
        assert_eq!(f1.data, payload);
        let f2 = fp.next_frame().unwrap();
        assert!(!f2.key);
        assert_eq!(f2.data, vec![9, 9, 9]);
    }

    #[test]
    fn ptz_request_shape() {
        let v: serde_json::Value = serde_json::from_str(&ptz_body(&PtzCmd { command: "DirectionUp".into(), stop: false, step: 2 }, 7)).unwrap();
        assert_eq!(v["Name"], "OPPTZControl");
        assert_eq!(v["OPPTZControl"]["Command"], "DirectionUp");
        assert_eq!(v["OPPTZControl"]["Parameter"]["Preset"], -1);
        assert_eq!(v["OPPTZControl"]["Parameter"]["Step"], 2);
        let v: serde_json::Value = serde_json::from_str(&ptz_body(&PtzCmd { command: "DirectionUp".into(), stop: true, step: 2 }, 7)).unwrap();
        assert_eq!(v["OPPTZControl"]["Parameter"]["Preset"], 65535);
        assert_eq!(ptz_step("DirectionLeft", 1.0), 4);
        assert_eq!(ptz_step("DirectionUp", 1.0), 2);
    }

    #[test]
    fn monitor_request_shape() {
        let v: serde_json::Value = serde_json::from_str(&monitor_body("Claim", true, false, 0x1d)).unwrap();
        assert_eq!(v["Name"], "OPMonitor");
        assert_eq!(v["OPMonitor"]["Action"], "Claim");
        assert_eq!(v["OPMonitor"]["Action1"], "Start");
        assert_eq!(v["OPMonitor"]["Parameter"]["StreamType"], "Extra1");
        assert_eq!(v["OPMonitor"]["Parameter"]["TransMode"], "TCP");
        assert_eq!(v["SessionID"], "0x000000001D");
    }
    #[test]
    fn decodes_h264_key_frame() {
        // a real H.264 picture made by OpenH264's encoder, wrapped like the camera does
        let mut enc = openh264::encoder::Encoder::new().unwrap();
        let yuv = openh264::formats::YUVBuffer::new(64, 64);
        let bits = enc.encode(&yuv).unwrap().to_vec();
        let mut frame = vec![0, 0, 1, 0xFC, 0x02, 15, 8, 8, 0, 0, 0, 0];
        let n = bits.len();
        frame.extend_from_slice(&[(n & 0xff) as u8, ((n >> 8) & 0xff) as u8, ((n >> 16) & 0xff) as u8, 0]);
        frame.extend_from_slice(&bits);
        frame.extend(vec![0u8; 30]);
        let mut fp = FrameParser::default();
        fp.push(&frame);
        let vf = fp.next_frame().unwrap();
        assert_eq!((vf.codec, vf.w, vf.h), (2, 64, 64));
        let mut dec = Decoder::new();
        let (w, h, rgb) = dec.decode(&vf).unwrap().expect("picture");
        assert_eq!((w, h, rgb.len()), (64, 64, 64 * 64 * 3));
    }

    #[test]
    fn h265_goes_through_ffmpeg() {
        // with FFmpeg: an H.265 picture made by FFmpeg itself comes back as RGB; without it, a clear message
        let vf_empty = VideoFrame { key: true, codec: 3, fps: 15, w: 64, h: 64, data: vec![] };
        let Some(ff) = find_ffmpeg() else {
            assert_eq!(Decoder::new().decode(&vf_empty).unwrap_err(), NO_FFMPEG);
            return;
        };
        let out = std::process::Command::new(ff)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=64x64:rate=5", "-frames:v", "20", "-c:v", "libx265", "-x265-params", "bframes=0:log-level=none", "-f", "hevc", "pipe:1"])
            .output();
        let Ok(out) = out else { return };
        if !out.status.success() || out.stdout.is_empty() {
            return; // this FFmpeg build can't encode H.265; decoding is still used for cameras
        }
        let mut dec = Decoder::new();
        let vf = VideoFrame { data: out.stdout, ..vf_empty };
        let mut got = dec.decode(&vf).unwrap();
        for _ in 0..50 {
            if got.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            // a camera keeps sending; send the clip again so FFmpeg keeps going
            let _ = dec.hevc.as_mut().unwrap().write(&vf.data);
            got = dec.hevc.as_mut().unwrap().take();
        }
        let (w, h, rgb) = got.expect("picture from ffmpeg");
        assert_eq!((w, h, rgb.len()), (64, 64, 64 * 64 * 3));
    }
}

