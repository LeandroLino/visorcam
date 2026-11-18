//! Real network work with cameras: logging in to a camera on its port with the vendor's
//! local protocol (read from libFunSDK.so), and finding cameras on the local network.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NetState {
    Unknown,
    Checking,
    /// Logged in; round trip in ms.
    Online(u32),
    /// Nothing answers on the port.
    Offline,
    /// The camera refused the user name or password (Ret 106 / 203).
    WrongPassword,
    /// The camera answered with another result code.
    Refused(i32),
    /// Something answers, but not with this protocol.
    NotCamera,
}

impl NetState {
    pub fn is_online(&self) -> bool {
        matches!(self, NetState::Online(_))
    }
}

// ---- protocol (libFunSDK.so, MNetSDK::CProtocolNetIP) ---------------------------------

/// Header size: InitMsg clears 0x14 bytes and writes fields up to offset 0x13.
pub const HEAD_SIZE: usize = 20;
/// Login request (NewLoginPTL @0xfa4efc passes 1000 to InitMsg) and its reply (ParsePTL 0x3e9).
pub const MSG_LOGIN: u16 = 1000;
pub const MSG_LOGIN_REPLY: u16 = 1001;
/// SystemInfo request (CDeviceV2::UpdateDevConfig @0xbe49d0, Dev_SendPTL 0x3fc).
pub const MSG_SYSINFO: u16 = 1020;
/// Device result code meaning success (CProtocolNetIP::ParseError @0xfa3648: 100 → 0).
pub const RET_OK: i64 = 100;
/// Codes ParseError maps to the same SDK error (-0x2700): wrong user name or password.
pub const RET_WRONG_PASSWORD: [i64; 2] = [106, 203];

/// XMMD5Encrypt @0xf8f578: MD5 of the password folded to 8 characters. Each character comes
/// from (byte[2i] + byte[2i+1]) mod 62: 0-9 digits, 10-35 'A'-'Z', 36-61 'a'-'z'.
pub fn password_hash(pw: &str) -> String {
    let d = md5::compute(pw.as_bytes()).0;
    (0..8)
        .map(|i| {
            let n = ((d[2 * i] as u32 + d[2 * i + 1] as u32) % 62) as u8;
            match n {
                0..=9 => (b'0' + n) as char,
                10..=35 => (b'A' + n - 10) as char,
                _ => (b'a' + n - 36) as char,
            }
        })
        .collect()
}

/// InitMsg @0xfa0bd8: 0xFF, version 1, two zero bytes, session id (u32 LE), sequence (u32),
/// channel, end flag, message id (u16), body length (u32) — the body is JSON plus a 0 byte.
pub fn packet(session: u32, seq: u32, msg: u16, json: &str) -> Vec<u8> {
    let mut body = json.as_bytes().to_vec();
    body.push(0);
    let mut p = Vec::with_capacity(HEAD_SIZE + body.len());
    p.extend_from_slice(&[0xFF, 0x01, 0x00, 0x00]);
    p.extend_from_slice(&session.to_le_bytes());
    p.extend_from_slice(&seq.to_le_bytes());
    p.extend_from_slice(&[0x00, 0x00]);
    p.extend_from_slice(&msg.to_le_bytes());
    p.extend_from_slice(&(body.len() as u32).to_le_bytes());
    p.extend_from_slice(&body);
    p
}

/// CheckPTL @0xfa1674: first byte must be 0xFF; length at offset 0x10.
fn read_packet(s: &mut TcpStream) -> Result<(u16, u32, String), &'static str> {
    let mut head = [0u8; HEAD_SIZE];
    s.read_exact(&mut head).map_err(|_| "no reply")?;
    if head[0] != 0xFF {
        return Err("not camera");
    }
    let session = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
    let msg = u16::from_le_bytes([head[14], head[15]]);
    let len = u32::from_le_bytes([head[16], head[17], head[18], head[19]]) as usize;
    if len > 1 << 20 {
        return Err("not camera");
    }
    let mut body = vec![0u8; len];
    s.read_exact(&mut body).map_err(|_| "no reply")?;
    let text = String::from_utf8_lossy(&body).trim_end_matches('\0').to_string();
    Ok((msg, session, text))
}

/// What a camera says about itself (SystemInfoBean fields).
#[derive(Clone, Default, Debug)]
pub struct SysInfo {
    pub serial: String,
    pub software: String,
    pub build_time: String,
    pub run_time: String,
    pub hardware: String,
    pub model: String,
}

fn get_ci<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    v.as_object()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

fn str_ci(v: &serde_json::Value, key: &str) -> String {
    get_ci(v, key).map(|x| x.as_str().map(|s| s.to_string()).unwrap_or_else(|| x.to_string())).unwrap_or_default()
}

#[allow(dead_code)]
pub struct LoginOk {
    pub ms: u32,
    pub session: u32,
    pub info: Option<SysInfo>,
}

/// Log in like NewLoginPTL (EncryptType "MD5", LoginType "DVRIP-Web", UserName, PassWord)
/// and then ask for SystemInfo. ParsePTL reads "Ret" and "SessionID" ("0x%x") from replies.
pub fn login(host: &str, port: u16, user: &str, pw: &str, timeout: Duration) -> Result<LoginOk, NetState> {
    let (mut s, session, ms) = open_session(host, port, user, pw, timeout)?;
    // InitMsg rewrites SessionID as "0x%010x" for every message except login.
    let req = serde_json::json!({ "Name": "SystemInfo", "SessionID": format!("0x{:010x}", session) });
    let info = (|| {
        s.write_all(&packet(session, 1, MSG_SYSINFO, &req.to_string())).ok()?;
        let (_, _, text) = read_packet(&mut s).ok()?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        let si = get_ci(&v, "SystemInfo").unwrap_or(&v);
        Some(SysInfo {
            serial: str_ci(si, "SerialNo"),
            software: str_ci(si, "SoftWareVersion"),
            build_time: str_ci(si, "BuildTime"),
            run_time: str_ci(si, "DeviceRunTime"),
            hardware: str_ci(si, "HardWare"),
            model: str_ci(si, "DeviceModel"),
        })
    })();
    Ok(LoginOk { ms, session, info })
}

pub fn connect(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, NetState> {
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs().map_err(|_| NetState::Offline)?.collect();
    for a in addrs {
        if let Ok(s) = TcpStream::connect_timeout(&a, timeout) {
            let _ = s.set_read_timeout(Some(timeout));
            let _ = s.set_write_timeout(Some(timeout));
            let _ = s.set_nodelay(true);
            return Ok(s);
        }
    }
    Err(NetState::Offline)
}

/// Read one reply and turn its "Ret" into a state (ParsePTL / ParseError rules).
pub fn reply_ret(s: &mut TcpStream) -> Result<(u16, u32, serde_json::Value), NetState> {
    let (msg, sid, text) = read_packet(s).map_err(|_| NetState::NotCamera)?;
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    let ret = get_ci(&v, "Ret").and_then(|r| r.as_i64()).unwrap_or(RET_OK);
    if RET_WRONG_PASSWORD.contains(&ret) {
        return Err(NetState::WrongPassword);
    }
    if ret != RET_OK {
        return Err(NetState::Refused(ret as i32));
    }
    Ok((msg, sid, v))
}

pub fn read_raw(s: &mut TcpStream) -> std::io::Result<(u16, Vec<u8>)> {
    let mut head = [0u8; HEAD_SIZE];
    s.read_exact(&mut head)?;
    if head[0] != 0xFF {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "HeadFlag err"));
    }
    let msg = u16::from_le_bytes([head[14], head[15]]);
    let len = u32::from_le_bytes([head[16], head[17], head[18], head[19]]) as usize;
    if len > 8 << 20 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "too long"));
    }
    let mut body = vec![0u8; len];
    s.read_exact(&mut body)?;
    Ok((msg, body))
}

/// Config read / write (DEVAPI::GetCmdIdByCmdName @0xc277cc: names not in its table use
/// 1042 to read and 1040 to write; "EncodeCapability" is row 2 of the table → 1360).
pub const MSG_CONFIG_GET: u16 = 1042;
pub const MSG_CONFIG_SET: u16 = 1040;
pub const MSG_ABILITY_GET: u16 = 1360;

fn request(s: &mut TcpStream, session: u32, seq: u32, msg: u16, body: serde_json::Value) -> Result<serde_json::Value, String> {
    s.write_all(&packet(session, seq, msg, &body.to_string())).map_err(|_| "A câmera não respondeu".to_string())?;
    match reply_ret(s) {
        Ok((_, _, v)) => Ok(v),
        Err(NetState::Refused(c)) => Err(format!("A câmera recusou o pedido (código {c})")),
        Err(_) => Err("A câmera não respondeu".into()),
    }
}

/// Like DevEncodeConfigActivity: read EncodeCapability (CompressionMask bit 7 = "H.264",
/// ResolutionInfo.getEncodeMode) and Simplify.Encode, set MainFormat.Video.Compression to
/// "H.264" and write Simplify.Encode back. Returns what changed.
pub fn set_h264(host: &str, port: u16, user: &str, pw: &str) -> Result<String, String> {
    let t = Duration::from_secs(6);
    let (mut s, session, _) = open_session(host, port, user, pw, t).map_err(|e| match e {
        NetState::WrongPassword => "Senha incorreta".to_string(),
        NetState::Offline => "O dispositivo pode estar off-line. Tente mais tarde".to_string(),
        _ => "Falha na conexão".to_string(),
    })?;
    let sid = format!("0x{:010x}", session);
    let cap = request(&mut s, session, 1, MSG_ABILITY_GET, serde_json::json!({"Name": "EncodeCapability", "SessionID": sid}))?;
    let mask_txt = get_ci(&cap, "EncodeCapability")
        .and_then(|c| get_ci(c, "EncodeInfo"))
        .and_then(|e| e.as_array().and_then(|a| a.first().cloned()))
        .map(|e| str_ci(&e, "CompressionMask"))
        .unwrap_or_default();
    if let Ok(mask) = u32::from_str_radix(mask_txt.trim_start_matches("0x").trim_start_matches("0X"), 16) {
        if mask != 0 && mask & (1 << 7) == 0 {
            return Err("Esta câmera só grava em H.265; ela não aceita H.264.".into());
        }
    }
    let mut enc = request(&mut s, session, 2, MSG_CONFIG_GET, serde_json::json!({"Name": "Simplify.Encode", "SessionID": sid}))?;
    let list = enc
        .as_object_mut()
        .and_then(|o| o.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case("Simplify.Encode")).map(|(_, v)| v))
        .and_then(|v| v.as_array_mut())
        .ok_or("A câmera não informou a codificação")?;
    let mut changed = Vec::new();
    for item in list.iter_mut() {
        for fmt in ["MainFormat", "ExtraFormat"] {
            if let Some(c) = item.get_mut(fmt).and_then(|f| f.get_mut("Video")).and_then(|v| v.get_mut("Compression")) {
                let cur = c.as_str().unwrap_or("").to_string();
                // the original changes MainFormat; the extra stream is changed too when it is H.265
                if cur.starts_with("H.265") && (fmt == "MainFormat" || !changed.is_empty()) {
                    *c = serde_json::Value::String("H.264".into());
                    changed.push(format!("{fmt}: {cur} → H.264"));
                }
            }
        }
    }
    if changed.is_empty() {
        return Ok("A câmera já está em H.264.".into());
    }
    let body = serde_json::json!({
        "Name": "Simplify.Encode",
        "Simplify.Encode": serde_json::Value::Array(list.clone()),
        "SessionID": sid,
    });
    request(&mut s, session, 3, MSG_CONFIG_SET, body)?;
    Ok(format!("Codificação alterada ({}).", changed.join(", ")))
}

/// Connect and log in; keeps the connection open for later requests.
pub fn open_session(host: &str, port: u16, user: &str, pw: &str, timeout: Duration) -> Result<(TcpStream, u32, u32), NetState> {
    let t0 = Instant::now();
    let mut s = connect(host, port, timeout)?;
    let body = serde_json::json!({
        "EncryptType": "MD5",
        "LoginType": "DVRIP-Web",
        "UserName": user,
        "PassWord": password_hash(pw),
    });
    s.write_all(&packet(0, 0, MSG_LOGIN, &body.to_string())).map_err(|_| NetState::Offline)?;
    let (msg, head_sid, text) = read_packet(&mut s).map_err(|e| if e == "not camera" { NetState::NotCamera } else { NetState::NotCamera })?;
    let ms = t0.elapsed().as_millis().max(1) as u32;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|_| NetState::NotCamera)?;
    if msg != MSG_LOGIN_REPLY {
        return Err(NetState::NotCamera);
    }
    // ParsePTL: a reply without "Ret" counts as success.
    let ret = get_ci(&v, "Ret").and_then(|r| r.as_i64()).unwrap_or(RET_OK);
    if RET_WRONG_PASSWORD.contains(&ret) {
        return Err(NetState::WrongPassword);
    }
    if ret != RET_OK {
        return Err(NetState::Refused(ret as i32));
    }
    let sid_txt = str_ci(&v, "SessionID");
    let session = u32::from_str_radix(sid_txt.trim_start_matches("0x").trim_start_matches("0X"), 16).unwrap_or(head_sid);
    Ok((s, session, ms))
}

#[derive(Default, Clone)]
pub struct ScanState {
    pub running: bool,
    pub done: u32,
    pub total: u32,
    pub subnet: String,
    pub found: Vec<(String, u16, u32)>,
    pub error: Option<String>,
}

#[derive(Clone, Default)]
pub struct Net {
    states: Arc<Mutex<HashMap<String, (NetState, Instant)>>>,
    infos: Arc<Mutex<HashMap<String, SysInfo>>>,
    scan: Arc<Mutex<ScanState>>,
}

/// How long a connection attempt may take.
pub const CHECK_TIMEOUT_MS: u64 = 2000;
pub const SCAN_TIMEOUT_MS: u64 = 350;
/// How often a camera added by IP is checked again.
pub const RECHECK_SECONDS: u64 = 30;

pub fn probe(host: &str, port: u16, timeout: Duration) -> Option<u32> {
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs().ok()?.collect();
    for a in addrs {
        let t0 = Instant::now();
        if TcpStream::connect_timeout(&a, timeout).is_ok() {
            return Some(t0.elapsed().as_millis().max(1) as u32);
        }
    }
    None
}

pub fn local_ipv4() -> Option<[u8; 4]> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    // No packet is sent: connecting a UDP socket only picks the outgoing interface.
    s.connect("8.8.8.8:80").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(v4) => Some(v4.octets()),
        _ => None,
    }
}

impl Net {
    pub fn state(&self, id: &str) -> NetState {
        self.states.lock().ok().and_then(|m| m.get(id).map(|x| x.0)).unwrap_or(NetState::Unknown)
    }

    pub fn info(&self, id: &str) -> Option<SysInfo> {
        self.infos.lock().ok().and_then(|m| m.get(id).cloned())
    }

    /// Start a login check unless one ran recently.
    pub fn ensure(&self, id: &str, host: &str, port: u16, user: &str, pw: &str) {
        let need = match self.states.lock() {
            Ok(m) => match m.get(id) {
                None => true,
                Some((NetState::Checking, _)) => false,
                Some((_, t)) => t.elapsed() > Duration::from_secs(RECHECK_SECONDS),
            },
            Err(_) => false,
        };
        if need {
            self.check(id, host, port, user, pw);
        }
    }

    /// Log in to the camera in the background and remember the result.
    pub fn check(&self, id: &str, host: &str, port: u16, user: &str, pw: &str) {
        if let Ok(mut m) = self.states.lock() {
            m.insert(id.to_string(), (NetState::Checking, Instant::now()));
        }
        let states = self.states.clone();
        let infos = self.infos.clone();
        let id = id.to_string();
        let host = host.to_string();
        let user = user.to_string();
        let pw = pw.to_string();
        std::thread::spawn(move || {
            let st = match login(&host, port, &user, &pw, Duration::from_millis(CHECK_TIMEOUT_MS)) {
                Ok(ok) => {
                    if let (Some(info), Ok(mut m)) = (ok.info, infos.lock()) {
                        m.insert(id.clone(), info);
                    }
                    NetState::Online(ok.ms)
                }
                Err(e) => e,
            };
            if let Ok(mut m) = states.lock() {
                m.insert(id, (st, Instant::now()));
            }
        });
    }

    pub fn scan_state(&self) -> ScanState {
        self.scan.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn start_scan(&self, port: u16) {
        let Some(ip) = local_ipv4() else {
            if let Ok(mut s) = self.scan.lock() {
                *s = ScanState { error: Some("Não foi possível descobrir a rede deste computador.".into()), ..Default::default() };
            }
            return;
        };
        if let Ok(mut s) = self.scan.lock() {
            if s.running {
                return;
            }
            *s = ScanState {
                running: true,
                done: 0,
                total: 254,
                subnet: format!("{}.{}.{}.x", ip[0], ip[1], ip[2]),
                found: Vec::new(),
                error: None,
            };
        }
        let scan = self.scan.clone();
        let next = Arc::new(AtomicU32::new(1));
        let finished = Arc::new(AtomicU32::new(0));
        for _ in 0..32 {
            let scan = scan.clone();
            let next = next.clone();
            let finished = finished.clone();
            std::thread::spawn(move || {
                loop {
                    let n = next.fetch_add(1, Ordering::SeqCst);
                    if n > 254 {
                        break;
                    }
                    let host = format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], n);
                    let r = if n as u8 == ip[3] { None } else { probe(&host, port, Duration::from_millis(SCAN_TIMEOUT_MS)) };
                    if let Ok(mut s) = scan.lock() {
                        s.done += 1;
                        if let Some(ms) = r {
                            s.found.push((host, port, ms));
                            s.found.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then(a.0.cmp(&b.0)));
                        }
                    }
                }
                if finished.fetch_add(1, Ordering::SeqCst) == 31 {
                    if let Ok(mut s) = scan.lock() {
                        s.running = false;
                        s.done = s.total;
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_of_empty_password() {
        // MD5("") = d41d8cd9 8f00b204 ... folded by XMMD5Encrypt
        assert_eq!(password_hash(""), "tlJwpbo6");
    }

    #[test]
    fn header_layout() {
        let p = packet(0x12, 3, MSG_SYSINFO, "{}");
        assert_eq!(&p[0..4], &[0xFF, 1, 0, 0]);
        assert_eq!(u32::from_le_bytes([p[4], p[5], p[6], p[7]]), 0x12);
        assert_eq!(u32::from_le_bytes([p[8], p[9], p[10], p[11]]), 3);
        assert_eq!(u16::from_le_bytes([p[14], p[15]]), 1020);
        assert_eq!(u32::from_le_bytes([p[16], p[17], p[18], p[19]]), 3);
        assert_eq!(p.len(), HEAD_SIZE + 3);
    }
    fn fake_camera(ret: i64) -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let (msg, _, text) = read_packet(&mut s).unwrap();
            assert_eq!(msg, MSG_LOGIN);
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["LoginType"], "DVRIP-Web");
            assert_eq!(v["EncryptType"], "MD5");
            assert_eq!(v["UserName"], "admin");
            let ok = v["PassWord"] == password_hash("abc");
            let r = if ok { ret } else { 203 };
            let reply = format!("{{\"Ret\":{r},\"SessionID\":\"0x0000000007\"}}");
            s.write_all(&packet(7, 0, MSG_LOGIN_REPLY, &reply)).unwrap();
            if r != RET_OK {
                return;
            }
            let (msg, sid, text) = read_packet(&mut s).unwrap();
            assert_eq!(msg, MSG_SYSINFO);
            assert_eq!(sid, 7);
            assert!(text.contains("\"SessionID\":\"0x0000000007\""));
            let reply = r#"{"Name":"SystemInfo","Ret":100,"SystemInfo":{"SerialNo":"abc123","SoftWareVersion":"V5.1"}}"#;
            s.write_all(&packet(7, 1, MSG_SYSINFO + 1, reply)).unwrap();
        });
        port
    }

    #[test]
    fn login_against_fake_camera() {
        let port = fake_camera(RET_OK);
        let ok = login("127.0.0.1", port, "admin", "abc", Duration::from_secs(2)).ok().expect("login");
        assert_eq!(ok.session, 7);
        let info = ok.info.expect("system info");
        assert_eq!(info.serial, "abc123");
        assert_eq!(info.software, "V5.1");
        let port = fake_camera(RET_OK);
        assert_eq!(login("127.0.0.1", port, "admin", "wrong", Duration::from_secs(2)).err(), Some(NetState::WrongPassword));
    }
    #[test]
    fn switches_camera_to_h264() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let _ = read_packet(&mut s).unwrap();
            s.write_all(&packet(9, 0, MSG_LOGIN_REPLY, r#"{"Ret":100,"SessionID":"0x00000009"}"#)).unwrap();
            let (msg, _, _) = read_packet(&mut s).unwrap();
            assert_eq!(msg, MSG_ABILITY_GET);
            s.write_all(&packet(9, 1, msg + 1, r#"{"Name":"EncodeCapability","Ret":100,"EncodeCapability":{"EncodeInfo":[{"CompressionMask":"0x180"}]}}"#)).unwrap();
            let (msg, _, _) = read_packet(&mut s).unwrap();
            assert_eq!(msg, MSG_CONFIG_GET);
            s.write_all(&packet(9, 2, msg + 1, r#"{"Name":"Simplify.Encode","Ret":100,"Simplify.Encode":[{"MainFormat":{"Video":{"Compression":"H.265","FPS":15}},"ExtraFormat":{"Video":{"Compression":"H.265"}}}]}"#)).unwrap();
            let (msg, _, text) = read_packet(&mut s).unwrap();
            assert_eq!(msg, MSG_CONFIG_SET);
            *seen2.lock().unwrap() = text;
            s.write_all(&packet(9, 3, msg + 1, r#"{"Name":"Simplify.Encode","Ret":100}"#)).unwrap();
        });
        let r = set_h264("127.0.0.1", port, "admin", "", ).unwrap();
        assert!(r.contains("H.264"));
        let sent: serde_json::Value = serde_json::from_str(&seen.lock().unwrap()).unwrap();
        assert_eq!(sent["Name"], "Simplify.Encode");
        assert_eq!(sent["Simplify.Encode"][0]["MainFormat"]["Video"]["Compression"], "H.264");
        assert_eq!(sent["Simplify.Encode"][0]["MainFormat"]["Video"]["FPS"], 15);
        assert_eq!(sent["Simplify.Encode"][0]["ExtraFormat"]["Video"]["Compression"], "H.264");
    }
}

