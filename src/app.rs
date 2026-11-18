//! The window: navigation, shared state, dialogs and notices.

use crate::model::{self, Device, Msg, Store};
use crate::net::Net;
use crate::scene::{self, View};
use crate::ui::{self, Icon, BG, TEXT2, THEME};
use eframe::egui::{self, Color32, CornerRadius, Rect, Sense};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Devices,
    Messages,
    Album,
    Me,
}

#[derive(Clone, PartialEq)]
pub enum Page {
    Main,
    Live,
    Playback,
    Settings(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddMode {
    Ip,
    Lan,
    Code,
    #[allow(dead_code)]
    Demo,
}

pub struct AddDlg {
    pub mode: AddMode,
    pub name: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub password: String,
    pub code: String,
    pub scene: u8,
    pub error: Option<String>,
    pub test_id: Option<String>,
    pub pending: Option<Device>,
}

impl AddDlg {
    pub fn new(mode: AddMode) -> Self {
        AddDlg {
            mode,
            name: String::new(),
            host: String::new(),
            port: String::new(),
            user: String::new(),
            password: String::new(),
            code: String::new(),
            scene: 0,
            error: None,
            test_id: None,
            pending: None,
        }
    }
}

pub enum Confirm {
    DeleteDevice(String),
    Format(String),
    Reboot(String),
    DeleteMessages(Vec<String>),
    DeleteMedia(Vec<String>),
}

pub struct LiveState {
    pub slots: Vec<Option<String>>,
    pub active: usize,
    pub split4: bool,
    pub views: HashMap<String, View>,
    pub hd: HashMap<String, bool>,
    pub muted: bool,
    pub paused_at: Option<f64>,
    pub recording: Option<(String, f64)>,
    pub talking: bool,
    pub side_tab: u8,
    pub pick_slot: Option<usize>,
    /// PTZ command being held on a real camera: (device id, command).
    pub ptz_active: Option<(String, String)>,
}

pub struct PlaybackState {
    pub dev: String,
    pub day0: i64,
    pub t: f64,
    pub playing: bool,
    pub speed: f32,
    pub speed_level: i32,
    pub filter: u8,
    pub span_h: f32,
    pub center: f64,
    pub cal_open: bool,
    pub cal_month: (i32, u32),
}

pub struct MsgUi {
    pub system: bool,
    pub dev: Option<String>,
    pub day: Option<i64>,
    pub kind: u8,
    pub selected: Option<String>,
    pub editing: bool,
    pub checked: std::collections::HashSet<String>,
}

pub struct AlbumUi {
    pub kind: u8,
    pub dev: Option<String>,
    pub viewer: Option<String>,
    pub play_t: f64,
    pub playing: bool,
    pub editing: bool,
    pub checked: std::collections::HashSet<String>,
}

pub struct App {
    pub store: Store,
    pub screenshot: Option<(PathBuf, u32, bool)>,
    pub dirty: bool,
    last_save: Instant,
    pub tab: Tab,
    pub page: Page,
    pub net: Net,
    pub feeds: ui::Feeds,
    pub images: ui::Images,
    pub toasts: Vec<(String, f64, bool)>,
    pub add: Option<AddDlg>,
    pub share: Option<String>,
    pub help: bool,
    pub confirm: Option<Confirm>,
    pub live: LiveState,
    pub pb: PlaybackState,
    pub msg_ui: MsgUi,
    pub album_ui: AlbumUi,
    pub settings_section: u8,
    pub rename_buf: String,
    pub pwd: (String, String, String),
    pub search: String,
    pub fullscreen: bool,
    msgs: (i64, Vec<Msg>),
    last_alarm_check: i64,
    pub frame: u64,
    pub streams: HashMap<String, crate::stream::LiveStream>,
    pub stream_tex: HashMap<String, (egui::TextureHandle, u64)>,
    /// Switching a camera to H.264 in the background: (device id, result when done).
    pub codec_job: Option<(String, std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>)>,
    /// Cameras whose HD stream is H.265, shown in SD instead.
    pub sd_fallback: std::collections::HashSet<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        ui::setup_style(&cc.egui_ctx);
        let shot = screenshot.is_some();
        let mut store = model::load_store(shot);
        store.seed_demo();
        let now = model::now_i();
        let first = store.devices.first().map(|d| d.id.clone());
        App {
            store,
            screenshot: screenshot.map(|p| (p, 0, false)),
            dirty: !shot,
            last_save: Instant::now(),
            tab: Tab::Devices,
            page: Page::Main,
            net: Net::default(),
            feeds: ui::Feeds::default(),
            images: ui::Images::default(),
            toasts: Vec::new(),
            add: None,
            share: None,
            help: false,
            confirm: None,
            live: LiveState {
                slots: vec![first.clone(), None, None, None],
                active: 0,
                split4: false,
                views: HashMap::new(),
                hd: HashMap::new(),
                muted: true,
                paused_at: None,
                recording: None,
                talking: false,
                side_tab: 0,
                pick_slot: None,
                ptz_active: None,
            },
            pb: PlaybackState {
                dev: first.unwrap_or_default(),
                day0: model::day_start(now),
                t: now as f64 - 600.0,
                playing: false,
                speed: 1.0,
                speed_level: 0,
                filter: 0,
                span_h: 24.0,
                center: model::day_start(now) as f64 + 43200.0,
                cal_open: false,
                cal_month: (0, 0),
            },
            msg_ui: MsgUi { system: false, dev: None, day: None, kind: 0, selected: None, editing: false, checked: Default::default() },
            album_ui: AlbumUi { kind: 0, dev: None, viewer: None, play_t: 0.0, playing: false, editing: false, checked: Default::default() },
            settings_section: 0,
            rename_buf: String::new(),
            pwd: Default::default(),
            search: String::new(),
            fullscreen: false,
            msgs: (0, Vec::new()),
            last_alarm_check: now,
            frame: 0,
            streams: HashMap::new(),
            stream_tex: HashMap::new(),
            codec_job: None,
            sd_fallback: Default::default(),
        }
        .start_at(std::env::var("VISORCAM_START").unwrap_or_default().as_str())
    }

    /// Optional first screen (used to check each screen).
    fn start_at(mut self, page: &str) -> Self {
        let first = self.store.devices.first().map(|d| d.id.clone());
        match (page, first) {
            ("live", Some(id)) => self.open_live(&[id]),
            ("live4", Some(_)) => {
                let ids: Vec<String> = self.store.devices.iter().map(|d| d.id.clone()).collect();
                self.open_live(&ids);
            }
            ("playback", Some(id)) => self.open_playback(&id, model::now_f() - 1800.0),
            ("settings", Some(id)) => self.open_settings(&id),
            ("messages", _) => self.tab = Tab::Messages,
            ("album", _) => self.tab = Tab::Album,
            ("me", _) => self.tab = Tab::Me,
            ("add", _) => self.add = Some(AddDlg::new(AddMode::Ip)),
            ("addcode", _) => self.add = Some(AddDlg::new(AddMode::Code)),
            _ => {}
        }
        self
    }

    pub fn is_check(&self) -> bool {
        self.screenshot.is_some()
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push((text.into(), model::now_f() + 3.0, false));
    }
    pub fn alert(&mut self, text: impl Into<String>) {
        self.toasts.push((text.into(), model::now_f() + 5.0, true));
    }

    pub fn messages(&mut self) -> Vec<Msg> {
        let now = model::now_i();
        if self.msgs.0 != now || self.dirty {
            self.msgs = (now, self.store.messages(now));
        }
        self.msgs.1.clone()
    }

    pub fn unread_for(&mut self, dev_id: Option<&str>) -> usize {
        let now = model::now_i();
        if self.msgs.0 != now {
            self.msgs = (now, self.store.messages(now));
        }
        self.msgs.1.iter().filter(|m| !m.read && dev_id.map(|d| d == m.dev_id).unwrap_or(true)).count()
    }

    pub fn open_live(&mut self, ids: &[String]) {
        if ids.len() > 1 {
            self.live.split4 = true;
            self.live.slots = (0..4).map(|i| ids.get(i).cloned()).collect();
        } else {
            self.live.split4 = self.store.prefs.split_default;
            if let Some(id) = ids.first() {
                if let Some(pos) = self.live.slots.iter().position(|s| s.as_deref() == Some(id.as_str())) {
                    self.live.active = pos;
                } else {
                    self.live.slots[0] = Some(id.clone());
                    self.live.active = 0;
                }
            }
        }
        self.live.paused_at = None;
        self.page = Page::Live;
    }

    pub fn open_playback(&mut self, dev: &str, t: f64) {
        self.pb.dev = dev.to_string();
        self.pb.day0 = model::day_start(t as i64);
        self.pb.t = t;
        self.pb.center = t;
        self.pb.span_h = 2.0;
        self.pb.playing = true;
        self.pb.cal_open = false;
        self.page = Page::Playback;
    }

    pub fn open_settings(&mut self, dev: &str) {
        if let Some(d) = self.store.device(dev) {
            self.rename_buf = d.name.clone();
        }
        self.pwd = Default::default();
        self.settings_section = 0;
        self.page = Page::Settings(dev.to_string());
    }

    pub fn album_path(&self) -> PathBuf {
        model::album_dir(self.is_check())
    }

    /// Save a still picture of a camera into the album.
    pub fn snapshot(&mut self, dev_id: &str, t: f64, view: View) {
        let Some(d) = self.store.device(dev_id).cloned() else { return };
        if !d.is_demo() {
            self.toast("Erro na cópia inst.: sem imagem deste dispositivo");
            return;
        }
        let img = scene::render(d.seed, d.scene, t, 1280, 720, &d.look(), &view);
        let dir = self.album_path();
        // XMMediaPlayer.i: <devId>_<channel>_<currentTimeMillis>.jpg, name passed through E.i
        let millis = (model::now_f() * 1000.0) as i64;
        let name = format!("{}.jpg", model::file_safe_id(&format!("{}_0_{}", d.id, millis)));
        let path = dir.join(&name);
        match save_png(&path, &img) {
            Ok(()) => {
                self.store.album.push(model::AlbumItem {
                    path: path.to_string_lossy().to_string(),
                    dev_id: d.id.clone(),
                    dev_name: d.name.clone(),
                    ts: t,
                    kind: model::MediaKind::Photo,
                    duration: 0.0,
                    seed: d.seed,
                    scene: d.scene,
                    flip_lr: d.settings.flip_lr,
                    flip_ud: d.settings.flip_ud,
                    night_mode: d.settings.night_mode,
                });
                self.dirty = true;
                self.toast("Instantâneo realizado com sucesso");
            }
            Err(e) => self.alert(format!("Erro na cópia inst.: {e}")),
        }
    }

    /// Finish a recording: keep its cover picture and replay data in the album.
    pub fn stop_recording(&mut self) {
        let Some((dev_id, start)) = self.live.recording.take() else { return };
        let Some(d) = self.store.device(&dev_id).cloned() else { return };
        let dur = (model::now_f() - start).max(1.0);
        let view = self.live.views.get(&dev_id).copied().unwrap_or_default();
        let img = scene::render(d.seed, d.scene, start, 640, 360, &d.look(), &view);
        // XMMediaPlayer.S1 names recordings <devId>_<channel>_<millis>.mp4; this version keeps
        // a cover picture under that name and replays the scene, so ".mp4" becomes "_mp4.png".
        let millis = (start * 1000.0) as i64;
        let path = self.album_path().join(format!("{}_mp4.png", model::file_safe_id(&format!("{}_0_{}", d.id, millis))));
        if let Err(e) = save_png(&path, &img) {
            self.alert(format!("Falha ao salvar a gravação: {e}"));
            return;
        }
        self.store.album.push(model::AlbumItem {
            path: path.to_string_lossy().to_string(),
            dev_id: d.id.clone(),
            dev_name: d.name.clone(),
            ts: start,
            kind: model::MediaKind::Video,
            duration: dur,
            seed: d.seed,
            scene: d.scene,
            flip_lr: d.settings.flip_lr,
            flip_ud: d.settings.flip_ud,
            night_mode: d.settings.night_mode,
        });
        self.dirty = true;
        self.toast(format!("Vídeo salvo no álbum ({})", model::fmt_dur(dur as i64)));
    }

    pub fn add_device(&mut self, d: Device) -> Result<(), String> {
        if self.store.device(&d.id).is_some() {
            return Err("O dispositivo já existe".into());
        }
        self.store.log(&format!("Dispositivo adicionado: {}", d.name));
        if !d.is_demo() {
            self.net.check(&d.id, &d.host, d.port, &d.user, &d.password);
        }
        let id = d.id.clone();
        self.store.devices.push(d);
        if self.live.slots.iter().all(|s| s.is_none()) {
            self.live.slots[0] = Some(id);
        }
        self.dirty = true;
        Ok(())
    }

    pub fn delete_device(&mut self, id: &str) {
        if let Some(pos) = self.store.devices.iter().position(|d| d.id == id) {
            let d = self.store.devices.remove(pos);
            self.store.log(&format!("Dispositivo excluído: {}", d.name));
            for s in self.live.slots.iter_mut() {
                if s.as_deref() == Some(id) {
                    *s = None;
                }
            }
            if matches!(&self.live.recording, Some((r, _)) if r == id) {
                self.live.recording = None;
            }
            self.dirty = true;
            self.page = Page::Main;
            self.toast("Dispositivo excluído");
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        let now = model::now_i();
        // reachability of cameras added by IP
        for d in &self.store.devices {
            if !d.is_demo() {
                self.net.ensure(&d.id, &d.host, d.port, &d.user, &d.password);
            }
        }
        // new alarms become notices
        if now > self.last_alarm_check {
            let from = self.last_alarm_check;
            self.last_alarm_check = now;
            let quiet = !self.store.prefs.push || self.store.prefs.dnd_active(now);
            let mut notes = Vec::new();
            for d in &self.store.devices {
                if !d.is_demo() || !d.settings.armed || !d.settings.push {
                    continue;
                }
                for e in scene::alarm_events(d.seed, &d.filter(), from, now) {
                    let label = if e.kind == scene::EvKind::Human && d.settings.human { "Detecção de corpo humano" } else { "Detecção de movimento" };
                    notes.push(format!("Alarme: {} — {}", label, d.name));
                }
            }
            if !quiet {
                for n in notes {
                    self.alert(n);
                }
            }
        }
        if self.dirty && !self.is_check() && self.last_save.elapsed().as_secs_f32() > 1.0 {
            if let Err(e) = model::save_store(&self.store) {
                self.alert(format!("Não foi possível salvar: {e}"));
            }
            self.dirty = false;
            self.last_save = Instant::now();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.fullscreen {
            self.fullscreen = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
    }

    fn nav(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .exact_width(92.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(egui::Margin::symmetric(6, 14)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(46.0, 46.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(12), THEME);
                    ui::draw_icon(ui.painter(), Icon::Camera, r.shrink(11.0), Color32::WHITE);
                    ui.label(egui::RichText::new("VisorCam").small().strong().color(ui::TEXT));
                    ui.add_space(14.0);
                    let unread = self.unread_for(None);
                    let tabs = [
                        (Tab::Devices, Icon::Camera, "Dispositivo", 0),
                        (Tab::Messages, Icon::Bell, "Mensagem", unread),
                        (Tab::Album, Icon::Image, "Álbum", 0),
                        (Tab::Me, Icon::User, "Eu", 0),
                    ];
                    for (tab, icon, label, badge) in tabs {
                        let sel = self.tab == tab && self.page == Page::Main;
                        let col = if sel { THEME } else { TEXT2 };
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(80.0, 62.0), Sense::click());
                        if sel {
                            ui.painter().rect_filled(rect, CornerRadius::same(10), ui::THEME_SOFT);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, CornerRadius::same(10), Color32::from_rgb(0xF4, 0xF6, 0xF7));
                        }
                        let ir = Rect::from_center_size(rect.center() - egui::vec2(0.0, 9.0), egui::vec2(22.0, 22.0));
                        ui::draw_icon(ui.painter(), icon, ir, col);
                        ui.painter().text(rect.center_bottom() - egui::vec2(0.0, 8.0), egui::Align2::CENTER_BOTTOM, label, egui::FontId::proportional(12.5), col);
                        if badge > 0 {
                            let bp = ir.right_top() + egui::vec2(6.0, 0.0);
                            let txt = if badge > 99 { "99+".into() } else { badge.to_string() };
                            let w = 8.0 + 6.5 * txt.len() as f32;
                            ui.painter().rect_filled(Rect::from_center_size(bp, egui::vec2(w, 16.0)), CornerRadius::same(8), ui::RED);
                            ui.painter().text(bp, egui::Align2::CENTER_CENTER, txt, egui::FontId::proportional(10.5), Color32::WHITE);
                        }
                        if resp.clicked() {
                            if self.live.recording.is_some() && self.page == Page::Live {
                                self.stop_recording();
                            }
                            self.tab = tab;
                            self.page = Page::Main;
                        }
                        ui.add_space(4.0);
                    }
                });
            });
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        let now = model::now_f();
        self.toasts.retain(|t| t.1 > now);
        if self.toasts.len() > 4 {
            let n = self.toasts.len() - 4;
            self.toasts.drain(0..n);
        }
        let screen = ctx.screen_rect();
        let mut y = screen.bottom() - 28.0;
        for (text, _, alert) in self.toasts.iter().rev() {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("toasts")));
            let galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(14.5), Color32::WHITE);
            let size = galley.size() + egui::vec2(32.0, 18.0);
            let rect = Rect::from_min_size(egui::pos2(screen.center().x - size.x / 2.0 + 46.0, y - size.y), size);
            painter.rect_filled(rect, CornerRadius::same(18), if *alert { Color32::from_rgb(0xE6, 0x5C, 0x00) } else { Color32::from_rgba_unmultiplied(30, 34, 49, 235) });
            painter.galley(rect.min + egui::vec2(16.0, 9.0), galley, Color32::WHITE);
            y -= size.y + 8.0;
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, frames, asked)) = self.screenshot.as_mut() else { return };
        *frames += 1;
        let mut done = false;
        ctx.input(|i| {
            for e in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = e {
                    let _ = save_png(path, image);
                    done = true;
                }
            }
        });
        if done || *frames > 600 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if *frames >= 120 && !*asked {
            *asked = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        self.tick(ctx);
        if !self.fullscreen {
            self.nav(ctx);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| match self.page.clone() {
                Page::Main => match self.tab {
                    Tab::Devices => self.devices_page(ui),
                    Tab::Messages => self.messages_page(ui),
                    Tab::Album => self.album_page(ui),
                    Tab::Me => self.me_page(ui),
                },
                Page::Live => self.live_page(ui),
                Page::Playback => self.playback_page(ui),
                Page::Settings(id) => self.settings_page(ui, &id),
            });
        self.dialogs(ctx);
        self.toasts(ctx);
        self.feeds.end_frame();
        // close camera streams no screen has shown for 2 s
        self.streams.retain(|_, s| s.last_used.elapsed().as_secs_f32() < 2.0);
        let keys: std::collections::HashSet<String> = self.streams.keys().cloned().collect();
        self.stream_tex.retain(|k, _| keys.contains(k));
        self.handle_screenshot(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(40));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.live.recording.is_some() {
            self.stop_recording();
        }
        if !self.is_check() {
            let _ = model::save_store(&self.store);
        }
    }
}

/// Save a picture; the file type follows the extension (.png or .jpg).
pub fn save_png(path: &std::path::Path, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let [w, h] = img.size;
    let mut raw = Vec::with_capacity(w * h * 3);
    for c in &img.pixels {
        raw.extend_from_slice(&[c.r(), c.g(), c.b()]);
    }
    let buf = image::RgbImage::from_raw(w as u32, h as u32, raw).ok_or("imagem inválida")?;
    buf.save(path).map_err(|e| e.to_string())
}
