//! Devices, settings, messages, album and saving to disk.

use crate::scene::{self, EvKind, EventFilter};
use chrono::{Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default device TCP port when the field is empty (AddDeviceByIPActivity.K1: 0x8707).
pub const DEFAULT_PORT: u16 = 34567;
/// Default login when the user field is empty (AddDeviceByIPActivity.K1).
pub const DEFAULT_USER: &str = "admin";
/// Names the original turns into "admin".
pub const ADMIN_ALIASES: [&str; 3] = ["default", "666666", "guest"];
/// Device name limits (AddDeviceByIPActivity.K1): 32 for plain ASCII, 21 otherwise.
pub const NAME_MAX_ASCII: usize = 32;
pub const NAME_MAX_OTHER: usize = 21;
/// How far back alarm messages are kept (estimated: the original's search is 1-7 days).
pub const MESSAGE_DAYS: i64 = 7;
/// Usable size of the simulated memory card in GB (estimated).
pub const CARD_GB: f64 = 29.7;
/// Recording rates in GB per hour (estimated from 1024/512 kbit/s).
pub const RATE_HD_GB_H: f64 = 0.46;
pub const RATE_SD_GB_H: f64 = 0.23;
/// Alarm recording: seconds before and after the movement (estimated).
pub const ALARM_PRE_S: i64 = 5;
pub const ALARM_POST_S: i64 = 30;
/// Record file length slider (devset_record.xml seekBar 5..120; onProgressChanged adds 5).
pub const VIDEO_LEN_MIN: u32 = 5;
pub const VIDEO_LEN_MAX: u32 = 120;
/// Alarm interval slider (AlarmIntervalDlg.p: setMax(1770), value = progress + 30 s;
/// a device value under 30 is shown as 30).
pub const ALARM_INTERVAL_MIN_S: i64 = 30;
pub const ALARM_INTERVAL_MAX_S: i64 = 30 + 1770;
/// Simulated card size in MB, as the device reports it (StorageInfo TotalSpace is in MB).
pub const CARD_MB: f64 = CARD_GB * 1024.0;

/// Sensitivity slider (DevVideoAlarmSetActivity.onStopTrackingTouch): progress > 75 snaps
/// to 97 = High (device Level 6), > 25 snaps to 50 = Middle (Level 3), else 3 = Low (Level 1).
/// Returns (snapped slider value, level 0 low / 1 middle / 2 high).
pub fn sensitivity_from_slider(progress: u8) -> (u8, u8) {
    if progress > 75 {
        (97, 2)
    } else if progress > 25 {
        (50, 1)
    } else {
        (3, 0)
    }
}
/// Device Level stored for each app level (1, 3, 6).
pub const SENSITIVITY_DEVICE_LEVEL: [u8; 3] = [1, 3, 6];
/// Slider position shown for a level read back (J1: Level ≥5 → 96, ≥3 → 50, else 4).
pub fn slider_from_level(level: u8) -> u8 {
    match level {
        2 => 96,
        1 => 50,
        _ => 4,
    }
}

/// Day/night modes (DevDayNightModeActivity, Camera.ParamEx DayNightSwitch.SwitchMode):
/// 0 automatic, 1 day, 2 night, 3 timed.
pub const DAY_NIGHT_MODES: [&str; 4] = ["Troca automática", "Modo diurno", "Modo noturno", "Troca programada"];
pub const REBOOT_SECONDS: i64 = 25;

pub fn now_f() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}
pub fn now_i() -> i64 {
    now_f().floor() as i64
}

pub fn local(ts: i64) -> chrono::DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

pub fn day_start(ts: i64) -> i64 {
    let d = local(ts).date_naive();
    date_start(d).unwrap_or(ts - ts.rem_euclid(86400))
}

pub fn date_start(d: NaiveDate) -> Option<i64> {
    Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).earliest().map(|x| x.timestamp())
}

pub fn fmt_time(ts: i64) -> String {
    local(ts).format("%H:%M:%S").to_string()
}
pub fn fmt_date(ts: i64) -> String {
    local(ts).format("%d/%m/%Y").to_string()
}
pub fn fmt_datetime(ts: i64) -> String {
    local(ts).format("%Y-%m-%d %H:%M:%S").to_string()
}
pub fn fmt_dur(s: i64) -> String {
    let s = s.max(0);
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DevKind {
    Demo,
    /// Cameras like the original app's (vendor protocol, port 34567)
    Ip,
    /// Any camera with an RTSP address (most other brands); video through FFmpeg
    Rtsp,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordMode {
    Always,
    Alarm,
    Off,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FullAction {
    Cyclic,
    Stop,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct DevSettings {
    pub flip_lr: bool,
    pub flip_ud: bool,
    pub status_light: bool,
    pub alert_tone: bool,
    pub speaker_vol: u8,
    pub mic_vol: u8,
    pub night_mode: u8,
    pub language: String,
    pub motion: bool,
    pub human: bool,
    pub sensitivity: u8,
    pub alarm_interval: i64,
    pub push: bool,
    pub alarm_bell: bool,
    pub armed: bool,
    pub record_mode: RecordMode,
    pub record_hd: bool,
    pub video_length_min: u32,
    pub full_action: FullAction,
    pub timezone: String,
    /// Day/night switch sensitivity 0..10 (DevBasicSettingActivity.y2: setMax(10),
    /// progress = (50 - DncThr) / 4).
    pub dn_sensitivity: u8,
    /// Timed day/night switch: night starts / ends at these hours.
    pub dn_night_from: u32,
    pub dn_night_to: u32,
}

impl Default for DevSettings {
    fn default() -> Self {
        DevSettings {
            flip_lr: false,
            flip_ud: false,
            status_light: true,
            alert_tone: true,
            speaker_vol: 70,
            mic_vol: 70,
            night_mode: 0,
            language: "Português".into(),
            motion: true,
            human: true,
            sensitivity: 1,
            alarm_interval: ALARM_INTERVAL_MIN_S,
            push: true,
            alarm_bell: false,
            armed: true,
            record_mode: RecordMode::Always,
            record_hd: true,
            video_length_min: 60,
            full_action: FullAction::Cyclic,
            timezone: "GMT-03:00".into(),
            dn_sensitivity: 5,
            dn_night_from: 18,
            dn_night_to: 6,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: DevKind,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub seed: u64,
    pub scene: u8,
    pub settings: DevSettings,
    pub added_at: i64,
    pub format_ts: i64,
    pub firmware: String,
    /// Stream address for RTSP cameras (rtsp://user:password@host:554/path)
    pub url: String,
    /// The app this camera came from when it was imported from another app's list
    /// (empty: added here).
    pub source: String,
    #[serde(skip)]
    pub reboot_until: i64,
}

impl Default for Device {
    fn default() -> Self {
        Device {
            id: String::new(),
            name: String::new(),
            kind: DevKind::Demo,
            host: String::new(),
            port: DEFAULT_PORT,
            user: DEFAULT_USER.into(),
            password: String::new(),
            seed: 1,
            scene: 0,
            settings: DevSettings::default(),
            added_at: 0,
            format_ts: 0,
            firmware: "V5.00.R02.000529C7.10010.140600.0000000".into(),
            url: String::new(),
            source: String::new(),
            reboot_until: 0,
        }
    }
}

impl Device {
    pub fn demo(name: &str, scene: u8, seed: u64) -> Device {
        let now = now_i();
        let sn = format!("{:016x}", scene::hash64(seed));
        Device {
            id: sn,
            name: name.to_string(),
            kind: DevKind::Demo,
            seed,
            scene,
            added_at: now,
            format_ts: now - MESSAGE_DAYS * 86400,
            ..Default::default()
        }
    }

    pub fn ip(name: &str, host: &str, port: u16, user: &str, password: &str) -> Device {
        let now = now_i();
        Device {
            id: format!("{}:{}", host, port),
            name: name.to_string(),
            kind: DevKind::Ip,
            host: host.to_string(),
            port,
            user: user.to_string(),
            password: password.to_string(),
            seed: scene::hash64(now as u64),
            added_at: now,
            format_ts: now,
            ..Default::default()
        }
    }

    /// A camera reached with an RTSP address (another brand).
    pub fn rtsp(name: &str, url: &str) -> Device {
        let now = now_i();
        let host = rtsp_host(url).unwrap_or_default();
        Device {
            // the same address is the same camera
            id: format!("rtsp:{:016x}", fnv(url.trim())),
            name: name.to_string(),
            kind: DevKind::Rtsp,
            host,
            port: 554,
            url: url.to_string(),
            seed: scene::hash64(now as u64),
            added_at: now,
            format_ts: now,
            firmware: String::new(),
            ..Default::default()
        }
    }

    pub fn is_demo(&self) -> bool {
        self.kind == DevKind::Demo
    }

    pub fn is_rtsp(&self) -> bool {
        self.kind == DevKind::Rtsp
    }

    /// Which app the camera belongs to, for the filter on the device list.
    pub fn app_name(&self) -> String {
        if !self.source.trim().is_empty() {
            return self.source.trim().to_string();
        }
        match self.kind {
            DevKind::Demo => "Demonstração".into(),
            DevKind::Ip => "iCSee (XM)".into(),
            DevKind::Rtsp => "RTSP".into(),
        }
    }

    /// Short label for the device card.
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            DevKind::Demo => "Demonstração",
            DevKind::Ip => "IP",
            DevKind::Rtsp => "RTSP",
        }
    }

    pub fn rebooting(&self, now: i64) -> bool {
        self.reboot_until > now
    }

    pub fn look(&self) -> scene::Look {
        scene::Look {
            flip_lr: self.settings.flip_lr,
            flip_ud: self.settings.flip_ud,
            night_mode: self.settings.night_mode.min(3),
            dn_sensitivity: self.settings.dn_sensitivity.min(10),
            night_from: self.settings.dn_night_from.min(23) as u8,
            night_to: self.settings.dn_night_to.min(23) as u8,
        }
    }

    pub fn filter(&self) -> EventFilter {
        EventFilter {
            motion: self.settings.motion,
            human: self.settings.human,
            sensitivity: self.settings.sensitivity.min(2),
            interval: self.settings.alarm_interval.clamp(ALARM_INTERVAL_MIN_S, ALARM_INTERVAL_MAX_S),
        }
    }

    pub fn rate_gb_h(&self) -> f64 {
        if self.settings.record_hd {
            RATE_HD_GB_H
        } else {
            RATE_SD_GB_H
        }
    }

    /// Time window whose recordings are still on the memory card.
    pub fn record_window(&self, now: i64) -> (i64, i64) {
        let from = self.format_ts;
        match self.settings.record_mode {
            RecordMode::Off => (now, now),
            RecordMode::Alarm => (from.max(now - 30 * 86400), now),
            RecordMode::Always => {
                let cap_s = (CARD_GB / self.rate_gb_h() * 3600.0) as i64;
                match self.settings.full_action {
                    FullAction::Cyclic => (from.max(now - cap_s), now),
                    FullAction::Stop => (from, now.min(from + cap_s)),
                }
            }
        }
    }

    /// Used space on the memory card in GB.
    pub fn used_gb(&self, now: i64) -> f64 {
        let (a, b) = self.record_window(now);
        match self.settings.record_mode {
            RecordMode::Off => 0.0,
            RecordMode::Always => ((b - a).max(0) as f64 / 3600.0 * self.rate_gb_h()).min(CARD_GB),
            RecordMode::Alarm => {
                let n = scene::alarm_events(self.seed, &self.filter(), a, b).len() as f64;
                (n * (ALARM_PRE_S + ALARM_POST_S) as f64 / 3600.0 * self.rate_gb_h()).min(CARD_GB)
            }
        }
    }

    /// Recording files on one day (newest last).
    pub fn segments(&self, day0: i64, now: i64) -> Vec<Segment> {
        let mut out = Vec::new();
        if !self.is_demo() {
            return out;
        }
        let (wa, wb) = self.record_window(now);
        let a = day0.max(wa);
        let b = (day0 + 86400).min(wb);
        if b <= a {
            return out;
        }
        let events = scene::alarm_events(self.seed, &self.filter(), a - ALARM_POST_S, b);
        match self.settings.record_mode {
            RecordMode::Off => {}
            RecordMode::Always => {
                let len = (self.settings.video_length_min.max(1) as i64) * 60;
                let mut s = a - (a - day0).rem_euclid(len);
                while s < b {
                    let e = (s + len).min(b);
                    let st = s.max(a);
                    let alarm = events.iter().any(|ev| ev.ts + ev.dur as i64 >= st && ev.ts < e);
                    out.push(Segment { start: st, end: e, alarm });
                    s += len;
                }
            }
            RecordMode::Alarm => {
                for ev in events {
                    let st = (ev.ts - ALARM_PRE_S).max(a);
                    let e = (ev.ts + ALARM_POST_S).min(b);
                    if e <= st {
                        continue;
                    }
                    if let Some(last) = out.last_mut() {
                        let l: &mut Segment = last;
                        if st <= l.end {
                            l.end = l.end.max(e);
                            continue;
                        }
                    }
                    out.push(Segment { start: st, end: e, alarm: true });
                }
            }
        }
        out
    }

    /// Alarm moments (for the red marks on the timeline).
    pub fn alarm_marks(&self, from: i64, to: i64) -> Vec<(i64, i64)> {
        if !self.is_demo() {
            return Vec::new();
        }
        scene::alarm_events(self.seed, &self.filter(), from, to)
            .into_iter()
            .map(|e| (e.ts, e.ts + e.dur as i64))
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub start: i64,
    pub end: i64,
    pub alarm: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MediaKind {
    Photo,
    Video,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AlbumItem {
    pub path: String,
    pub dev_id: String,
    pub dev_name: String,
    pub ts: f64,
    pub kind: MediaKind,
    pub duration: f64,
    pub seed: u64,
    pub scene: u8,
    #[serde(default)]
    pub flip_lr: bool,
    #[serde(default)]
    pub flip_ud: bool,
    #[serde(default)]
    pub night_mode: u8,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SysMsg {
    pub ts: i64,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShareRec {
    pub ts: i64,
    pub dev_id: String,
    pub dev_name: String,
    pub code: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Prefs {
    pub push: bool,
    pub dnd: bool,
    pub dnd_from: u32,
    pub dnd_to: u32,
    pub hard_decode: bool,
    pub split_default: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { push: true, dnd: false, dnd_from: 22, dnd_to: 7, hard_decode: true, split_default: false }
    }
}

impl Prefs {
    pub fn dnd_active(&self, ts: i64) -> bool {
        if !self.dnd {
            return false;
        }
        let h = scene::local_hour(ts as f64) as u32;
        if self.dnd_from <= self.dnd_to {
            h >= self.dnd_from && h < self.dnd_to
        } else {
            h >= self.dnd_from || h < self.dnd_to
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    pub devices: Vec<Device>,
    pub read: HashSet<String>,
    pub read_before: HashMap<String, i64>,
    pub deleted: HashSet<String>,
    pub album: Vec<AlbumItem>,
    pub prefs: Prefs,
    pub syslog: Vec<SysMsg>,
    pub shares: Vec<ShareRec>,
    pub seeded: bool,
    pub demos_removed: bool,
}

#[derive(Clone)]
pub struct Msg {
    pub key: String,
    pub dev_id: String,
    pub dev_name: String,
    pub ts: i64,
    pub kind: EvKind,
    pub read: bool,
}

impl Msg {
    pub fn title(&self) -> &'static str {
        match self.kind {
            EvKind::Human => "Detecção de corpo humano",
            EvKind::Motion => "Detecção de movimento",
        }
    }
}

pub fn msg_key(dev_id: &str, ts: i64) -> String {
    format!("{}|{}", dev_id, ts)
}

impl Store {
    /// No demo cameras: the list starts empty, and demo cameras saved by earlier versions are
    /// removed once (with their album entries kept as files on disk).
    pub fn seed_demo(&mut self) {
        if self.demos_removed {
            return;
        }
        self.demos_removed = true;
        self.seeded = true;
        let before = self.devices.len();
        self.devices.retain(|d| d.kind != DevKind::Demo);
        if self.devices.len() != before {
            self.log("Câmeras de demonstração removidas");
        }
    }

    pub fn log(&mut self, text: &str) {
        self.syslog.push(SysMsg { ts: now_i(), text: text.to_string() });
        if self.syslog.len() > 500 {
            let n = self.syslog.len() - 500;
            self.syslog.drain(0..n);
        }
    }

    pub fn device(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }
    pub fn device_mut(&mut self, id: &str) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }

    /// Every alarm message of the last days, newest first.
    pub fn messages(&self, now: i64) -> Vec<Msg> {
        let mut out = Vec::new();
        for d in &self.devices {
            if !d.is_demo() {
                continue;
            }
            let from = now - MESSAGE_DAYS * 86400;
            let rb = self.read_before.get(&d.id).copied().unwrap_or(i64::MIN);
            for e in scene::alarm_events(d.seed, &d.filter(), from, now + 1) {
                let key = msg_key(&d.id, e.ts);
                if self.deleted.contains(&key) {
                    continue;
                }
                let kind = if e.kind == EvKind::Human && d.settings.human { EvKind::Human } else { EvKind::Motion };
                let read = e.ts <= rb || self.read.contains(&key);
                out.push(Msg { key, dev_id: d.id.clone(), dev_name: d.name.clone(), ts: e.ts, kind, read });
            }
        }
        out.sort_by(|a, b| b.ts.cmp(&a.ts));
        out
    }

    pub fn mark_all_read(&mut self, now: i64) {
        for d in &self.devices {
            self.read_before.insert(d.id.clone(), now);
        }
        self.read.clear();
    }
}

pub fn data_dir(screenshot: bool) -> PathBuf {
    if screenshot {
        return std::env::temp_dir().join("visorcam-check");
    }
    // Windows: %APPDATA%\VisorCam; Linux: $XDG_DATA_HOME or ~/.local/share/visorcam
    if let Some(p) = std::env::var_os("APPDATA") {
        return PathBuf::from(p).join("VisorCam");
    }
    if let Some(p) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(p).join("visorcam");
    }
    match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h).join(".local").join("share").join("visorcam"),
        None => std::env::current_dir().unwrap_or_default().join("visorcam-data"),
    }
}

pub fn album_dir(screenshot: bool) -> PathBuf {
    if screenshot {
        return data_dir(true).join("album");
    }
    match std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        Some(p) => PathBuf::from(p).join("Pictures").join("VisorCam"),
        None => data_dir(false).join("album"),
    }
}

/// Open a folder (or show a file) in the system's file manager.
pub fn open_in_files(path: &std::path::Path, select: bool) -> bool {
    use std::process::Command;
    let r = if cfg!(windows) {
        if select {
            Command::new("explorer").arg(format!("/select,{}", path.display())).spawn()
        } else {
            Command::new("explorer").arg(path).spawn()
        }
    } else {
        let target = if select { path.parent().unwrap_or(path) } else { path };
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        Command::new(opener).arg(target).spawn()
    };
    r.is_ok()
}

pub fn load_store(screenshot: bool) -> Store {
    if screenshot {
        return Store::default();
    }
    let p = data_dir(false).join("dados.json");
    match std::fs::read_to_string(&p) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Store::default(),
    }
}

pub fn save_store(store: &Store) -> Result<(), String> {
    let dir = data_dir(false);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    let tmp = dir.join("dados.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join("dados.json")).map_err(|e| e.to_string())
}

/// Character class of `com.xworld.utils.v0.e`:
/// `^[(/\s/A-Za-z0-9!@#$%^&*()_|\[\]{}?/~.<>,';:-]+$`.
pub fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || "(/!@#$%^&*()_|[]{}?~.<>,';:-".contains(c))
}

/// Device name rule from the original (AddDeviceByIPActivity.K1): names made only of the
/// v0.e characters may have 32 characters, any other name 21.
pub fn name_too_long(name: &str) -> Option<usize> {
    let limit = if is_plain_name(name) { NAME_MAX_ASCII } else { NAME_MAX_OTHER };
    if name.encode_utf16().count() > limit {
        Some(limit)
    } else {
        None
    }
}

/// Login name rule from the original: empty and a few aliases become "admin".
pub fn normalize_user(user: &str) -> String {
    let u = user.trim();
    if u.is_empty() || ADMIN_ALIASES.iter().any(|a| a.eq_ignore_ascii_case(u)) {
        DEFAULT_USER.to_string()
    } else {
        u.to_string()
    }
}

/// `g3.f.X0`: a strict dotted IPv4 address,
/// `^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$`.
/// The add-by-IP screen checks this before anything else, so domain names are refused there.
pub fn valid_host(h: &str) -> bool {
    let parts: Vec<&str> = h.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty() && p.len() <= 3 && p.chars().all(|c| c.is_ascii_digit()) && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
        })
}

/// `g3.f.Z0` (`[0-9]*`) plus `Integer.parseInt`: a port made of digits only, else 34567.
pub fn parse_port(s: &str) -> u16 {
    let s = s.trim();
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
        return DEFAULT_PORT;
    }
    match s.parse::<u32>() {
        Ok(n) if n > 0 && n <= 65535 => n as u16,
        _ => DEFAULT_PORT,
    }
}

/// `com.xworld.utils.E.i`: spaces and ':' are removed from file names.
pub fn file_safe_id(id: &str) -> String {
    id.replace(' ', "").replace(':', "")
}

/// `com.xworld.utils.E.b(size, 2)`: a size given in MB, "#.00" with M, G, T or P,
/// dividing by 1024 each step.
pub fn fmt_size_mb(mb: f64) -> String {
    if mb <= 0.0 {
        return "0.00M".into();
    }
    let (v, u) = if mb < 1024.0 {
        (mb, "M")
    } else if mb < 1024.0 * 1024.0 {
        (mb / 1024.0, "G")
    } else if mb < 1024.0 * 1024.0 * 1024.0 {
        (mb / 1048576.0, "T")
    } else {
        (mb / 1073741824.0, "P")
    };
    let s = format!("{:.2}", v);
    // DecimalFormat("#.00") drops a lone leading zero: 0.50 -> .50
    let s = s.strip_prefix("0.").map(|r| format!(".{r}")).unwrap_or(s);
    format!("{s}{u}")
}

/// Host part of an rtsp:// address (after any user:password@).
pub fn rtsp_host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let auth_host = rest.split('/').next()?;
    let host_port = auth_host.rsplit('@').next()?;
    let host = host_port.split(':').next()?;
    if host.is_empty() { None } else { Some(host.to_string()) }
}

/// rtsp:// (or rtsps://, http:// for MJPEG cameras) with a host.
pub fn valid_stream_url(url: &str) -> bool {
    let u = url.trim();
    (u.starts_with("rtsp://") || u.starts_with("rtsps://") || u.starts_with("http://") || u.starts_with("https://"))
        && rtsp_host(u).is_some()
}

/// The address with the password hidden, for showing on screen.
pub fn hide_password(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        if let Some((auth, tail)) = rest.split_once('@') {
            if !auth.contains('/') {
                let user = auth.split(':').next().unwrap_or("");
                return format!("{scheme}://{user}:•••@{tail}");
            }
        }
    }
    url.to_string()
}

// ---- share codes ---------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64e(data: &[u8]) -> String {
    let mut s = String::new();
    for ch in data.chunks(3) {
        let b0 = ch[0] as u32;
        let b1 = *ch.get(1).unwrap_or(&0) as u32;
        let b2 = *ch.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        let k = ch.len() + 1;
        for i in 0..k {
            s.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    s
}

fn b64d(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        let v = B64.iter().position(|&x| x == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 255) as u8);
        }
    }
    Some(out)
}

#[derive(Serialize, Deserialize)]
struct ShareData {
    n: String,
    k: DevKind,
    h: String,
    p: u16,
    u: String,
    s: u64,
    c: u8,
    i: String,
    #[serde(default)]
    r: String,
}

pub fn share_code(d: &Device) -> String {
    let data = ShareData {
        n: d.name.clone(),
        k: d.kind,
        h: d.host.clone(),
        p: d.port,
        u: d.user.clone(),
        s: d.seed,
        c: d.scene,
        i: d.id.clone(),
        r: d.url.clone(),
    };
    format!("VC1-{}", b64e(serde_json::to_string(&data).unwrap_or_default().as_bytes()))
}

pub fn device_from_code(code: &str) -> Option<Device> {
    let body = code.trim().strip_prefix("VC1-")?;
    let bytes = b64d(body.trim())?;
    let data: ShareData = serde_json::from_slice(&bytes).ok()?;
    let now = now_i();
    Some(match data.k {
        DevKind::Demo => Device {
            id: data.i,
            name: data.n,
            kind: DevKind::Demo,
            seed: data.s,
            scene: data.c.min(2),
            added_at: now,
            format_ts: now - MESSAGE_DAYS * 86400,
            ..Default::default()
        },
        DevKind::Ip => Device::ip(&data.n, &data.h, data.p, &data.u, ""),
        DevKind::Rtsp => Device::rtsp(&data.n, &data.r),
    })
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

// ---- camera lists shared between apps ----
//
// A plain JSON file any camera app can write or read, so cameras from several apps show up in
// one list:
// {"format":"visorcam-cameras","version":1,"app":"Nome do app","cameras":[
//   {"name":"Sala","type":"ip","host":"192.168.1.10","port":34567,"user":"admin","password":""},
//   {"name":"Garagem","type":"rtsp","url":"rtsp://admin:senha@192.168.1.20:554/stream1"}]}

pub const LIST_FORMAT: &str = "visorcam-cameras";

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CameraList {
    pub format: String,
    pub version: u32,
    pub app: String,
    pub cameras: Vec<CameraEntry>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CameraEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub user: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub password: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// The app the camera originally came from, when this list passes it on.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub app: String,
}

/// This app's cameras (demos left out) as a list another app can read.
pub fn export_list(devices: &[Device]) -> String {
    let cameras = devices
        .iter()
        .filter(|d| !d.is_demo())
        .map(|d| CameraEntry {
            name: d.name.clone(),
            kind: if d.is_rtsp() { "rtsp".into() } else { "ip".into() },
            host: if d.is_rtsp() { String::new() } else { d.host.clone() },
            port: if d.is_rtsp() { None } else { Some(d.port) },
            user: if d.is_rtsp() { String::new() } else { d.user.clone() },
            password: if d.is_rtsp() { String::new() } else { d.password.clone() },
            url: d.url.clone(),
            app: d.source.clone(),
        })
        .collect();
    let list = CameraList { format: LIST_FORMAT.into(), version: 1, app: "VisorCam".into(), cameras };
    serde_json::to_string_pretty(&list).unwrap_or_default()
}

/// Read another app's camera list: the app's name, the cameras it could turn into devices,
/// and what was skipped (with the reason).
pub fn import_list(text: &str) -> Result<(String, Vec<Device>, Vec<String>), String> {
    let list: CameraList = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| "Este arquivo não é uma lista de câmeras (JSON) que o app entende.".to_string())?;
    if !list.format.is_empty() && list.format != LIST_FORMAT {
        return Err(format!("Formato desconhecido: {}", list.format));
    }
    if list.cameras.is_empty() {
        return Err("A lista não tem câmeras.".into());
    }
    let app = if list.app.trim().is_empty() { "Outro app".to_string() } else { list.app.trim().to_string() };
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for (i, c) in list.cameras.iter().enumerate() {
        let label = if c.name.trim().is_empty() { format!("câmera {}", i + 1) } else { c.name.trim().to_string() };
        let name: String = label.chars().take(32).collect();
        let mut d = match c.kind.to_lowercase().as_str() {
            "rtsp" | "onvif" | "http" => {
                if !valid_stream_url(&c.url) {
                    skipped.push(format!("{label}: endereço de vídeo inválido"));
                    continue;
                }
                Device::rtsp(&name, c.url.trim())
            }
            "" | "ip" | "xm" | "icsee" | "dvrip" => {
                if valid_host(c.host.trim()) {
                    let user = normalize_user(&c.user);
                    Device::ip(&name, c.host.trim(), c.port.unwrap_or(DEFAULT_PORT), &user, &c.password)
                } else if valid_stream_url(&c.url) {
                    Device::rtsp(&name, c.url.trim())
                } else {
                    skipped.push(format!("{label}: IP inválido"));
                    continue;
                }
            }
            other => {
                skipped.push(format!("{label}: tipo \"{other}\" ainda não suportado"));
                continue;
            }
        };
        // a camera this list passed on keeps the app it really came from
        let from = if c.app.trim().is_empty() { app.clone() } else { c.app.trim().to_string() };
        d.source = if from == "VisorCam" { String::new() } else { from };
        out.push(d);
    }
    Ok((app, out, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_lists_between_apps() {
        let text = r#"{"format":"visorcam-cameras","version":1,"app":"Meu App","cameras":[
            {"name":"Sala","type":"ip","host":"192.168.1.10","port":34567,"user":"","password":"x"},
            {"name":"Garagem","type":"rtsp","url":"rtsp://a:b@192.168.1.20:554/s1"},
            {"name":"Ruim","type":"ip","host":"999.1.1.1"},
            {"name":"Nuvem","type":"tuya"}]}"#;
        let (app, devs, skipped) = import_list(text).unwrap();
        assert_eq!(app, "Meu App");
        assert_eq!(devs.len(), 2);
        assert_eq!(devs[0].user, DEFAULT_USER);
        assert_eq!(devs[0].app_name(), "Meu App");
        assert!(devs[1].is_rtsp());
        assert_eq!(skipped.len(), 2);
        // exported and read back: same cameras, same ids, same app
        let (_, again, _) = import_list(&export_list(&devs)).unwrap();
        assert_eq!(again.len(), 2);
        assert_eq!(again[0].id, devs[0].id);
        assert_eq!(again[1].id, devs[1].id);
        assert_eq!(again[1].app_name(), "Meu App");
        assert!(import_list("{}").is_err());
    }

    #[test]
    fn share_roundtrip() {
        let d = Device::demo("Câmera ç", 1, 99);
        let back = device_from_code(&share_code(&d)).unwrap();
        assert_eq!(back.id, d.id);
        assert_eq!(back.name, d.name);
    }

    #[test]
    fn rules() {
        assert_eq!(normalize_user(""), "admin");
        assert_eq!(normalize_user("Guest"), "admin");
        assert_eq!(normalize_user("joao"), "joao");
        assert!(valid_host("192.168.1.10"));
        assert!(!valid_host("192.168.1.300"));
        assert!(!valid_host("cam.example.com"));
        assert_eq!(parse_port(""), 34567);
        assert_eq!(parse_port("80a"), 34567);
        assert_eq!(parse_port("8080"), 8080);
        assert_eq!(file_safe_id("192.168.1.2:34567"), "192.168.1.234567");
        assert_eq!(fmt_size_mb(30412.0), "29.70G");
        assert_eq!(fmt_size_mb(0.0), "0.00M");
        assert_eq!(sensitivity_from_slider(80), (97, 2));
        assert_eq!(sensitivity_from_slider(26), (50, 1));
        assert_eq!(sensitivity_from_slider(25), (3, 0));
        assert_eq!(rtsp_host("rtsp://admin:x@192.168.1.20:554/stream1").as_deref(), Some("192.168.1.20"));
        assert!(valid_stream_url("rtsp://192.168.1.20/live"));
        assert!(!valid_stream_url("192.168.1.20"));
        assert_eq!(hide_password("rtsp://admin:segredo@cam/1"), "rtsp://admin:•••@cam/1");
        assert_eq!(name_too_long(&"a".repeat(33)), Some(32));
        assert_eq!(name_too_long(&"ç".repeat(22)), Some(21));
    }
}
