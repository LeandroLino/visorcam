//! Live view (1 or 4 pictures, PTZ, talk, snapshot, record) and playback.

use crate::app::{App, Page};
use crate::model::{self, Device};
use crate::net::NetState;
use crate::scene::{View, PAN_LIMIT, TILT_LIMIT, ZOOM_MAX};
use crate::ui::{self, Icon, TEXT, TEXT2, TEXT3, THEME, VIDEO_BAR};
use chrono::{Datelike, NaiveDate};
use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Stroke};

/// PTZ movement per second while a direction is held (fraction of the picture).
pub const PTZ_SPEED: f32 = 0.25;
/// TR_Need_Video_Play_Can_Operation (pt_BR)
pub const NEED_PLAYING: &str = "Você poderá realizar as ações somente quando o vídeo estiver reproduzindo...";

/// Playback speed levels (PlayBackActivity.onSpeedPlay): level 0 is normal; the slow button
/// cycles -1 → -2 → -1, the fast button 1 → 2 → 1, and the label is level × 2 followed by "x"
/// (slow -2x / -4x, fast 2x / 4x). Level n plays at 2^n (¼, ½, 1, 2, 4).
/// dir: 0 normal, 1 fast, 2 slow.
pub fn next_speed_level(level: i32, dir: u8) -> i32 {
    match dir {
        0 => 0,
        2 => {
            if level < 0 && level > -2 {
                level - 1
            } else {
                -1
            }
        }
        _ => {
            if level > 0 && level < 2 {
                level + 1
            } else {
                1
            }
        }
    }
}

pub fn speed_of_level(level: i32) -> f32 {
    2f32.powi(level)
}

pub fn speed_label(level: i32) -> String {
    match level {
        0 => String::new(),
        l if l < 0 => format!("Avanço lento {}x", l * 2),
        l => format!("Avanço rápido {}x", l * 2),
    }
}

impl App {
    fn active_dev(&self) -> Option<Device> {
        self.live.slots.get(self.live.active).cloned().flatten().and_then(|id| self.store.device(&id).cloned())
    }

    pub fn live_page(&mut self, ui: &mut egui::Ui) {
        self.poll_codec_job();
        let dev = self.active_dev();
        let title = if self.live.split4 { "Streaming em tempo real · 4 telas".to_string() } else { dev.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "Streaming em tempo real".into()) };
        let mut go_pb = false;
        let mut go_set = false;
        let mut go_msg = false;
        if !self.fullscreen {
            let back = ui::top_bar(ui, &title, true, |ui| {
                if dev.is_some() {
                    if ui::secondary(ui, "Conf.").clicked() {
                        go_set = true;
                    }
                    if ui::secondary(ui, "Mensagem").clicked() {
                        go_msg = true;
                    }
                    if ui::primary(ui, "Reprodução").clicked() {
                        go_pb = true;
                    }
                }
            });
            if back {
                self.stop_recording();
                self.page = Page::Main;
                return;
            }
        }
        if let Some(d) = &dev {
            if go_pb {
                self.stop_recording();
                self.open_playback(&d.id, model::now_f() - 120.0);
                return;
            }
            if go_set {
                self.stop_recording();
                self.open_settings(&d.id);
                return;
            }
            if go_msg {
                self.stop_recording();
                self.tab = crate::app::Tab::Messages;
                self.msg_ui.dev = Some(d.id.clone());
                self.msg_ui.system = false;
                self.page = Page::Main;
                return;
            }
        }

        let full = ui.available_rect_before_wrap();
        let side_w = if self.fullscreen { 0.0 } else { 310.0 };
        let video_area = Rect::from_min_max(full.min, egui::pos2(full.right() - side_w, full.bottom()));
        let side = Rect::from_min_max(egui::pos2(full.right() - side_w, full.top()), full.max);
        ui.allocate_rect(full, Sense::hover());

        // video + bar
        let bar_h = 58.0;
        let pics = Rect::from_min_max(video_area.min + egui::vec2(12.0, 12.0), egui::pos2(video_area.right() - 12.0, video_area.bottom() - bar_h - 12.0));
        let bar = Rect::from_min_max(egui::pos2(video_area.left() + 12.0, video_area.bottom() - bar_h - 8.0), egui::pos2(video_area.right() - 12.0, video_area.bottom() - 8.0));
        ui.painter().rect_filled(Rect::from_min_max(pics.min, bar.max), CornerRadius::same(10), Color32::from_rgb(0x12, 0x14, 0x1A));
        let t_live = self.live.paused_at.unwrap_or_else(model::now_f);
        if self.live.split4 {
            let grid = ui::fit_16_9(pics);
            let cell = egui::vec2(grid.width() / 2.0, grid.height() / 2.0);
            for i in 0..4 {
                let r = Rect::from_min_size(grid.min + egui::vec2(cell.x * (i % 2) as f32, cell.y * (i / 2) as f32), cell).shrink(2.0);
                self.video_slot(ui, i, r, t_live);
            }
        } else {
            let r = ui::fit_16_9(pics);
            let a = self.live.active;
            self.video_slot(ui, a, r, t_live);
        }
        self.control_bar(ui, bar, dev.as_ref());

        if !self.fullscreen {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink2(egui::vec2(10.0, 12.0))));
            self.live_side(&mut child, dev.as_ref());
        }
        if let Some(slot) = self.live.pick_slot {
            self.pick_device(ui.ctx(), slot);
        }
    }

    fn video_slot(&mut self, ui: &mut egui::Ui, slot: usize, r: Rect, t: f64) {
        let p = ui.painter().clone();
        let id = self.live.slots.get(slot).cloned().flatten();
        let resp = ui.interact(r, ui.id().with(("slot", slot)), Sense::click_and_drag());
        let dev = id.as_ref().and_then(|i| self.store.device(i).cloned());
        let Some(d) = dev else {
            p.rect_filled(r, CornerRadius::same(6), Color32::from_rgb(0x1C, 0x1F, 0x27));
            p.circle_stroke(r.center(), 20.0, Stroke::new(2.0_f32, Color32::from_gray(140)));
            ui::draw_icon(&p, Icon::Plus, Rect::from_center_size(r.center(), egui::vec2(20.0, 20.0)), Color32::from_gray(160));
            p.text(r.center() + egui::vec2(0.0, 36.0), egui::Align2::CENTER_CENTER, "Adicionar dispositivo à tela", egui::FontId::proportional(13.0), Color32::from_gray(160));
            if resp.clicked() {
                self.live.active = slot;
                self.live.pick_slot = Some(slot);
            }
            return;
        };
        if resp.clicked() {
            self.live.active = slot;
        }
        if resp.double_clicked() && self.live.split4 {
            self.live.split4 = false;
            self.live.active = slot;
        }
        let now = model::now_i();
        let mut view = self.live.views.get(&d.id).copied().unwrap_or_default();
        if d.is_demo() && !d.rebooting(now) {
            // wheel zoom + drag pan (digital)
            if resp.hovered() {
                let dz = ui.input(|i| i.smooth_scroll_delta.y);
                if dz != 0.0 {
                    view.zoom = (view.zoom * (1.0 + dz * 0.002)).clamp(1.0, ZOOM_MAX);
                }
            }
            if resp.dragged() {
                let dl = resp.drag_delta();
                view.pan = (view.pan - dl.x / r.width() / view.zoom).clamp(-PAN_LIMIT, PAN_LIMIT);
                view.tilt = (view.tilt - dl.y / r.height() / view.zoom).clamp(-TILT_LIMIT, TILT_LIMIT);
            }
            self.live.views.insert(d.id.clone(), view);
            let hd = *self.live.hd.get(&d.id).unwrap_or(&true);
            let size = if hd { [640, 360] } else { [320, 180] };
            let tex = self.feeds.get(ui.ctx(), &format!("live:{}:{}", slot, d.id), d.seed, d.scene, t, size, d.look(), view, 12.0, self.live.paused_at.is_some());
            p.image(tex, r, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
            ui::osd_text(&p, r.left_top() + egui::vec2(10.0, 8.0), egui::Align2::LEFT_TOP, &model::fmt_datetime(t as i64), if r.width() > 600.0 { 15.0 } else { 12.0 });
            ui::osd_text(&p, r.right_bottom() - egui::vec2(10.0, 8.0), egui::Align2::RIGHT_BOTTOM, &d.name, if r.width() > 600.0 { 15.0 } else { 12.0 });
            let q = if hd { "HD" } else { "SD" };
            p.text(r.right_top() + egui::vec2(-10.0, 10.0), egui::Align2::RIGHT_TOP, q, egui::FontId::proportional(12.0), Color32::from_white_alpha(200));
            if view.zoom > 1.01 {
                ui::osd_text(&p, r.left_bottom() + egui::vec2(10.0, -8.0), egui::Align2::LEFT_BOTTOM, &format!("{:.1}x", view.zoom), 13.0);
            }
            if let Some((rid, start)) = &self.live.recording {
                if *rid == d.id {
                    let blink = (model::now_f() * 2.0) as i64 % 2 == 0;
                    let pos = r.left_top() + egui::vec2(12.0, 34.0);
                    if blink {
                        p.circle_filled(pos + egui::vec2(5.0, 7.0), 5.0, ui::RED);
                    }
                    ui::osd_text(&p, pos + egui::vec2(16.0, 0.0), egui::Align2::LEFT_TOP, &format!("REC {}", model::fmt_dur((model::now_f() - start) as i64)), 13.0);
                }
            }
            if self.live.paused_at.is_some() {
                p.circle_filled(r.center(), 30.0, Color32::from_black_alpha(130));
                ui::draw_icon(&p, Icon::Play, Rect::from_center_size(r.center() + egui::vec2(3.0, 0.0), egui::vec2(26.0, 26.0)), Color32::WHITE);
            }
            if self.live.talking && slot == self.live.active {
                ui::osd_text(&p, r.center_bottom() - egui::vec2(0.0, 14.0), egui::Align2::CENTER_BOTTOM, "Falando...", 14.0);
            }
        } else {
            p.rect_filled(r, CornerRadius::same(4), Color32::from_rgb(0x1C, 0x1F, 0x27));
            let state = self.net.state(&d.id);
            let msg: Option<String> = if d.rebooting(now) {
                Some("Reiniciando...".into())
            } else {
                match state {
                    NetState::Online(_) => {
                        // real live video (OPMonitor), see stream.rs
                        let hd = *self.live.hd.get(&d.id).unwrap_or(&true);
                        let frozen = self.live.paused_at.is_some();
                        let (tex, st) = self.camera_video(ui.ctx(), &d, hd, frozen);
                        if let Some((id, size)) = tex {
                            let fit = {
                                let a = size.x / size.y.max(1.0);
                                let w = r.width().min(r.height() * a);
                                Rect::from_center_size(r.center(), egui::vec2(w, w / a))
                            };
                            // digital zoom: wheel or slider zooms, dragging moves the zoomed picture
                            let mut view = self.live.views.get(&d.id).copied().unwrap_or_default();
                            if resp.hovered() {
                                let dz = ui.input(|i| i.smooth_scroll_delta.y);
                                if dz != 0.0 {
                                    view.zoom = (view.zoom * (1.0 + dz * 0.002)).clamp(1.0, ZOOM_MAX);
                                }
                            }
                            let half = 0.5 / view.zoom.max(1.0);
                            if resp.dragged() {
                                let dl = resp.drag_delta();
                                view.pan -= dl.x / fit.width() / view.zoom;
                                view.tilt -= dl.y / fit.height() / view.zoom;
                            }
                            view.pan = view.pan.clamp(-(0.5 - half), 0.5 - half);
                            view.tilt = view.tilt.clamp(-(0.5 - half), 0.5 - half);
                            self.live.views.insert(d.id.clone(), view);
                            let (cx, cy) = (0.5 + view.pan, 0.5 + view.tilt);
                            let uv = Rect::from_min_max(egui::pos2(cx - half, cy - half), egui::pos2(cx + half, cy + half));
                            p.image(id, fit, uv, Color32::WHITE);
                            if view.zoom > 1.01 {
                                ui::osd_text(&p, fit.left_bottom() + egui::vec2(10.0, -8.0), egui::Align2::LEFT_BOTTOM, &format!("{:.1}x", view.zoom), 13.0);
                            }
                        }
                        use crate::stream::Status;
                        match st {
                            Status::Playing { codec, w, h, fps } => {
                                ui::osd_text(&p, r.left_top() + egui::vec2(10.0, 8.0), egui::Align2::LEFT_TOP, &model::fmt_datetime(now), if r.width() > 600.0 { 15.0 } else { 12.0 });
                                p.text(r.right_top() + egui::vec2(-10.0, 10.0), egui::Align2::RIGHT_TOP, format!("{} · {}×{} · {} fps · {}", if hd { "HD" } else { "SD" }, w, h, fps, codec), egui::FontId::proportional(12.0), Color32::from_white_alpha(200));
                                if frozen {
                                    p.circle_filled(r.center(), 30.0, Color32::from_black_alpha(130));
                                    ui::draw_icon(&p, Icon::Play, Rect::from_center_size(r.center() + egui::vec2(3.0, 0.0), egui::vec2(26.0, 26.0)), Color32::WHITE);
                                }
                                None
                            }
                            Status::Connecting => Some("Abrindo vídeo...".into()),
                            Status::Unsupported(t) if t.contains("H.265") && !t.contains("FFmpeg") => {
                                if hd && !self.sd_fallback.contains(&d.id) {
                                    // many of these cameras send H.265 in HD but H.264 in SD
                                    self.sd_fallback.insert(d.id.clone());
                                    self.live.hd.insert(d.id.clone(), false);
                                    self.toast("O HD desta câmera está em H.265; tentando o SD...");
                                    Some("Abrindo vídeo em SD...".into())
                                } else {
                                    self.h265_panel(ui, r, &d);
                                    None
                                }
                            }
                            Status::Unsupported(t) | Status::Failed(t) => Some(t),
                        }
                    }
                    NetState::Offline => Some("Off-line".into()),
                    NetState::WrongPassword => Some("Senha incorreta".into()),
                    NetState::Refused(c) => Some(format!("Falha no login (código {c})")),
                    NetState::NotCamera => Some("Dispositivo incompatível".into()),
                    _ => Some("Conectando...".into()),
                }
            };
            if let Some(msg) = msg {
                let galley = p.layout(msg, egui::FontId::proportional(14.0), Color32::from_gray(210), (r.width() - 40.0).max(100.0));
                let pos = r.center() - galley.size() / 2.0;
                p.rect_filled(Rect::from_min_size(pos, galley.size()).expand(8.0), CornerRadius::same(6), Color32::from_black_alpha(140));
                p.galley(pos, galley, Color32::from_gray(210));
            }
            ui::osd_text(&p, r.right_bottom() - egui::vec2(10.0, 8.0), egui::Align2::RIGHT_BOTTOM, &d.name, 12.0);
        }
        if self.live.split4 && slot == self.live.active {
            p.rect_stroke(r, CornerRadius::same(4), Stroke::new(2.0_f32, THEME), egui::StrokeKind::Inside);
        }
    }

    /// Real video of a camera added by IP: start (or keep) its stream and return the newest
    /// picture as a texture, plus the stream's state.
    pub fn camera_video(&mut self, ctx: &egui::Context, d: &Device, hd: bool, frozen: bool) -> (Option<(egui::TextureId, egui::Vec2)>, crate::stream::Status) {
        let key = format!("{}|{}", d.id, if hd { "main" } else { "extra1" });
        let s = self
            .streams
            .entry(key.clone())
            .or_insert_with(|| crate::stream::LiveStream::start(&d.host, d.port, &d.user, &d.password, hd));
        s.last_used = std::time::Instant::now();
        let status = s.status();
        if !frozen {
            if let Ok(f) = s.frame.lock() {
                if let Some(fr) = f.as_ref() {
                    let fresh = self.stream_tex.get(&key).map(|(_, seq)| *seq != fr.seq).unwrap_or(true);
                    if fresh && fr.rgb.len() == fr.w * fr.h * 3 {
                        let img = egui::ColorImage::from_rgb([fr.w, fr.h], &fr.rgb);
                        match self.stream_tex.get_mut(&key) {
                            Some((tex, seq)) => {
                                tex.set(img, egui::TextureOptions::LINEAR);
                                *seq = fr.seq;
                            }
                            None => {
                                let tex = ctx.load_texture(format!("cam:{key}"), img, egui::TextureOptions::LINEAR);
                                self.stream_tex.insert(key.clone(), (tex, fr.seq));
                            }
                        }
                    }
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
        (self.stream_tex.get(&key).map(|(t, _)| (t.id(), t.size_vec2())), status)
    }

    /// Shown when the camera's video is H.265: explain and offer to switch it to H.264
    /// the way the original's "Configurações de codificação" does (net::set_h264).
    fn h265_panel(&mut self, ui: &mut egui::Ui, r: Rect, d: &Device) {
        let running = self.codec_job.as_ref().map(|(id, _)| *id == d.id).unwrap_or(false);
        let w = (r.width() - 40.0).clamp(200.0, 520.0);
        let area = Rect::from_center_size(r.center(), egui::vec2(w, 150.0));
        ui.painter().rect_filled(area, CornerRadius::same(10), Color32::from_black_alpha(170));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area.shrink(14.0)).layout(egui::Layout::top_down(egui::Align::Center)));
        child.label(egui::RichText::new("A câmera envia vídeo em H.265, formato que esta versão ainda não mostra.").color(Color32::WHITE));
        child.label(egui::RichText::new("O app pode mudar a câmera para H.264, como na opção Configurações de codificação do app original. A imagem fica um pouco maior na rede.").small().color(Color32::from_gray(200)));
        child.add_space(6.0);
        if running {
            child.label(egui::RichText::new("Mudando a codificação da câmera...").color(ui::ORANGE));
        } else if ui::primary(&mut child, "Mudar a câmera para H.264").clicked() {
            let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
            let s2 = slot.clone();
            let (host, port, user, pw) = (d.host.clone(), d.port, d.user.clone(), d.password.clone());
            std::thread::spawn(move || {
                let r = crate::net::set_h264(&host, port, &user, &pw);
                if let Ok(mut m) = s2.lock() {
                    *m = Some(r);
                }
            });
            self.codec_job = Some((d.id.clone(), slot));
        }
    }

    /// Finish a codec switch: on success restart the camera's streams in HD.
    fn poll_codec_job(&mut self) {
        let done = self.codec_job.as_ref().and_then(|(id, slot)| slot.lock().ok().and_then(|mut m| m.take()).map(|r| (id.clone(), r)));
        if let Some((id, r)) = done {
            self.codec_job = None;
            match r {
                Ok(msg) => {
                    self.streams.retain(|k, _| !k.starts_with(&format!("{id}|")));
                    self.sd_fallback.remove(&id);
                    self.live.hd.insert(id.clone(), true);
                    if let Some(name) = self.store.device(&id).map(|d| d.name.clone()) {
                        self.store.log(&format!("Codificação alterada para H.264: {name}"));
                    }
                    self.toast(format!("{msg} Abrindo o vídeo de novo..."));
                }
                Err(e) => self.alert(e),
            }
        }
    }

    /// Send a PTZ start/stop through the camera's open live stream.
    fn send_ptz(&mut self, dev_id: &str, command: &str, stop: bool, zoom: f32) {
        let hd = *self.live.hd.get(dev_id).unwrap_or(&true);
        let key = format!("{}|{}", dev_id, if hd { "main" } else { "extra1" });
        match self.streams.get(&key) {
            Some(s) => s.ptz(command, stop, crate::stream::ptz_step(command, zoom)),
            None if !stop => self.toast("Abra o vídeo da câmera para usar o PTZ."),
            None => {}
        }
    }

    /// Save the picture currently shown for a camera added by IP.
    pub fn camera_snapshot(&mut self, d: &Device, hd: bool) {
        let key = format!("{}|{}", d.id, if hd { "main" } else { "extra1" });
        let img = self.streams.get(&key).and_then(|s| {
            let f = s.frame.lock().ok()?;
            let fr = f.as_ref()?;
            Some(egui::ColorImage::from_rgb([fr.w, fr.h], &fr.rgb))
        });
        let Some(img) = img else {
            self.toast(NEED_PLAYING);
            return;
        };
        let millis = (model::now_f() * 1000.0) as i64;
        let path = self.album_path().join(format!("{}.jpg", model::file_safe_id(&format!("{}_0_{}", d.id, millis))));
        match crate::app::save_png(&path, &img) {
            Ok(()) => {
                self.store.album.push(model::AlbumItem {
                    path: path.to_string_lossy().to_string(),
                    dev_id: d.id.clone(),
                    dev_name: d.name.clone(),
                    ts: model::now_f(),
                    kind: model::MediaKind::Photo,
                    duration: 0.0,
                    seed: d.seed,
                    scene: d.scene,
                    flip_lr: false,
                    flip_ud: false,
                    night_mode: 0,
                });
                self.dirty = true;
                self.toast("Instantâneo realizado com sucesso");
            }
            Err(e) => self.alert(format!("Erro na cópia inst.: {e}")),
        }
    }

    fn control_bar(&mut self, ui: &mut egui::Ui, bar: Rect, dev: Option<&Device>) {
        ui.painter().rect_filled(bar, CornerRadius::same(10), VIDEO_BAR);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(egui::vec2(12.0, 9.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let ui = &mut child;
        let w = Color32::WHITE;
        let paused = self.live.paused_at.is_some();
        if ui::bar_button(ui, if paused { Icon::Play } else { Icon::Pause }, if paused { "Continuar" } else { "Pausar" }, false, w).clicked() {
            if !paused {
                self.stop_recording();
            }
            self.live.paused_at = if paused { None } else { Some(model::now_f()) };
        }
        let muted = self.live.muted;
        if ui::bar_button(ui, if muted { Icon::Mute } else { Icon::Speaker }, if muted { "Ativar som" } else { "Mudo" }, false, w).clicked() {
            self.live.muted = !muted;
            if muted {
                self.toast("Som ativado (as câmeras de demonstração não têm áudio)");
            }
        }
        if let Some(d) = dev {
            let hd = *self.live.hd.get(&d.id).unwrap_or(&true);
            if ui::text_button(ui, if hd { "HD" } else { "SD" }, "Alternar entre HD e SD", false).clicked() {
                self.live.hd.insert(d.id.clone(), !hd);
            }
            let rec = matches!(&self.live.recording, Some((r, _)) if *r == d.id);
            if ui::bar_button(ui, Icon::Record, if rec { "Parar gravação" } else { "Gravar" }, rec, if rec { ui::RED } else { w }).clicked() {
                if rec {
                    self.stop_recording();
                } else if !d.is_demo() {
                    self.toast("Gravar no computador ainda não está disponível para câmeras reais; use o Instantâneo.");
                } else if self.live.paused_at.is_some() || d.rebooting(model::now_i()) {
                    // XMMediaPlayer.S1: TR_Need_Video_Play_Can_Operation when not playing
                    self.toast(NEED_PLAYING);
                } else {
                    self.stop_recording();
                    self.live.recording = Some((d.id.clone(), model::now_f()));
                }
            }
            if ui::bar_button(ui, Icon::Snapshot, "Instantâneo", false, w).clicked() {
                if self.live.paused_at.is_some() || d.rebooting(model::now_i()) {
                    // XMMediaPlayer.i: same rule for snapshots
                    self.toast(NEED_PLAYING);
                } else if !d.is_demo() {
                    let hd = *self.live.hd.get(&d.id).unwrap_or(&true);
                    self.camera_snapshot(d, hd);
                } else {
                    let view = self.live.views.get(&d.id).copied().unwrap_or_default();
                    self.snapshot(&d.id, model::now_f(), view);
                }
            }
            let talk = ui::bar_button(ui, Icon::Mic, "Pressione para falar", self.live.talking, w);
            self.live.talking = talk.is_pointer_button_down_on();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let fs = self.fullscreen;
            if ui::bar_button(ui, Icon::Fullscreen, if fs { "Sair da tela cheia (Esc)" } else { "Tela cheia" }, fs, w).clicked() {
                self.fullscreen = !fs;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
            }
            let s4 = self.live.split4;
            if ui::bar_button(ui, if s4 { Icon::Single } else { Icon::Grid }, if s4 { "Uma tela" } else { "4 telas" }, false, w).clicked() {
                self.live.split4 = !s4;
            }
        });
    }

    fn live_side(&mut self, ui: &mut egui::Ui, dev: Option<&Device>) {
        ui.horizontal(|ui| {
            if ui::chip(ui, self.live.side_tab == 0, "PTZ").clicked() {
                self.live.side_tab = 0;
            }
            if ui::chip(ui, self.live.side_tab == 1, "Intercomunicação").clicked() {
                self.live.side_tab = 1;
            }
        });
        ui.add_space(8.0);
        let Some(d) = dev else {
            ui.label(egui::RichText::new("Escolha um dispositivo na tela.").color(TEXT2));
            return;
        };
        ui::card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.live.side_tab == 0 {
                self.ptz_pad(ui, d);
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(120.0, 120.0), Sense::click_and_drag());
                    let down = resp.is_pointer_button_down_on();
                    self.live.talking = down || (self.live.talking && ui.input(|i| i.pointer.any_down()));
                    let pulse = if down { 6.0 + ((model::now_f() * 6.0).sin() as f32) * 4.0 } else { 0.0 };
                    ui.painter().circle_filled(r.center(), 54.0 + pulse, if down { THEME } else { ui::THEME_SOFT });
                    ui::draw_icon(ui.painter(), Icon::Mic, Rect::from_center_size(r.center(), egui::vec2(40.0, 40.0)), if down { Color32::WHITE } else { THEME });
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(if down { "Falando..." } else { "Pressione para falar" }).color(TEXT));
                    ui.label(egui::RichText::new("O microfone do computador não é transmitido nesta versão.").small().color(TEXT3));
                    ui.add_space(6.0);
                });
            }
        });
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Mensagens de alarme").strong().color(TEXT));
        let msgs: Vec<model::Msg> = self.messages().into_iter().filter(|m| m.dev_id == d.id).take(6).collect();
        if msgs.is_empty() {
            ui.label(egui::RichText::new("Nenhuma mensagem").color(TEXT3));
        }
        for m in msgs {
            let resp = ui::card_frame()
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if !m.read {
                            let (dr, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                            ui.painter().circle_filled(dr.center(), 3.5, ui::RED);
                        }
                        ui.label(egui::RichText::new(m.title()).color(TEXT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(model::fmt_time(m.ts)).small().color(TEXT3));
                        });
                    });
                })
                .response
                .interact(Sense::click())
                .on_hover_text("Ver a gravação deste alarme");
            if resp.clicked() {
                self.store.read.insert(m.key.clone());
                self.dirty = true;
                self.stop_recording();
                self.open_playback(&m.dev_id, m.ts as f64 - 3.0);
            }
        }
    }

    fn ptz_pad(&mut self, ui: &mut egui::Ui, d: &Device) {
        let mut view = self.live.views.get(&d.id).copied().unwrap_or_default();
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        // OPPTZControl command names for the 8 arrows (NewPTZControlPTL @0xfaa4b4)
        const PTZ_CMDS: [&str; 8] = [
            "DirectionUp", "DirectionRightUp", "DirectionRight", "DirectionRightDown",
            "DirectionDown", "DirectionLeftDown", "DirectionLeft", "DirectionLeftUp",
        ];
        let mut pressed: Option<&str> = None;
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(170.0, 170.0), Sense::hover());
            let p = ui.painter().clone();
            p.circle_filled(r.center(), 84.0, Color32::from_rgb(0xF2, 0xF4, 0xF6));
            p.circle_stroke(r.center(), 84.0, Stroke::new(1.0_f32, ui::DIVIDER));
            let dirs: [(f32, f32, &str); 8] = [(0.0, -1.0, "Cima"), (1.0, -1.0, ""), (1.0, 0.0, "Direita"), (1.0, 1.0, ""), (0.0, 1.0, "Baixo"), (-1.0, 1.0, ""), (-1.0, 0.0, "Esquerda"), (-1.0, -1.0, "")];
            for (i, (dx, dy, tip)) in dirs.iter().enumerate() {
                let n = (dx * dx + dy * dy).sqrt();
                let c = r.center() + egui::vec2(dx / n, dy / n) * 58.0;
                let br = Rect::from_center_size(c, egui::vec2(34.0, 34.0));
                let resp = ui.interact(br, ui.id().with(("ptz", i)), Sense::click_and_drag());
                let down = resp.is_pointer_button_down_on();
                let col = if down { THEME } else if resp.hovered() { ui::THEME_DARK } else { TEXT3 };
                let a = dy.atan2(*dx);
                let tipp = c + egui::vec2(a.cos(), a.sin()) * 9.0;
                let l = c + egui::vec2((a + 2.4).cos(), (a + 2.4).sin()) * 8.0;
                let rr = c + egui::vec2((a - 2.4).cos(), (a - 2.4).sin()) * 8.0;
                p.add(egui::Shape::convex_polygon(vec![tipp, l, rr], col, Stroke::NONE));
                if down && d.is_demo() {
                    view.pan = (view.pan + dx / n * PTZ_SPEED * dt / view.zoom).clamp(-PAN_LIMIT, PAN_LIMIT);
                    view.tilt = (view.tilt + dy / n * PTZ_SPEED * 0.5 * dt / view.zoom).clamp(-TILT_LIMIT, TILT_LIMIT);
                }
                if down {
                    pressed = Some(PTZ_CMDS[i]);
                }
                if !tip.is_empty() {
                    resp.on_hover_text(*tip);
                }
            }
            let cr = Rect::from_center_size(r.center(), egui::vec2(46.0, 46.0));
            let cresp = ui.interact(cr, ui.id().with("ptz-center"), Sense::click());
            p.circle_filled(r.center(), 23.0, if cresp.hovered() { ui::THEME_SOFT } else { Color32::WHITE });
            p.circle_stroke(r.center(), 8.0, Stroke::new(2.0_f32, THEME));
            p.circle_filled(r.center(), 3.0, THEME);
            if cresp.on_hover_text("Voltar ao centro").clicked() {
                view = View::default();
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Zoom").color(TEXT2));
            if ui.button(" - ").clicked() {
                view.zoom = (view.zoom / 1.25).max(1.0);
            }
            ui.add(egui::Slider::new(&mut view.zoom, 1.0..=ZOOM_MAX).show_value(false));
            if ui.button(" + ").clicked() {
                view.zoom = (view.zoom * 1.25).min(ZOOM_MAX);
            }
        });
        if !d.is_demo() {
            // real camera: start the move when an arrow is pressed, stop it when released
            let now_cmd = pressed.map(|c| (d.id.clone(), c.to_string()));
            if now_cmd != self.live.ptz_active {
                if let Some((id, cmd)) = self.live.ptz_active.take() {
                    self.send_ptz(&id, &cmd, true, view.zoom);
                }
                if let Some((id, cmd)) = &now_cmd {
                    self.send_ptz(id, cmd, false, view.zoom);
                }
                self.live.ptz_active = now_cmd;
            }
            ui.label(
                egui::RichText::new("Mantenha uma seta pressionada para girar a câmera (se ela tiver motor). O zoom aproxima a imagem; role o mouse sobre o vídeo para aproximar e arraste para mover.")
                    .small()
                    .color(TEXT3),
            );
        }
        self.live.views.insert(d.id.clone(), view);
    }

    fn pick_device(&mut self, ctx: &egui::Context, slot: usize) {
        let mut open = true;
        let mut chosen: Option<String> = None;
        egui::Window::new("Selecionar dispositivo")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(320.0);
                for d in &self.store.devices {
                    if ui.add(egui::Button::new(&d.name).min_size(egui::vec2(300.0, 32.0))).clicked() {
                        chosen = Some(d.id.clone());
                    }
                }
                if self.store.devices.is_empty() {
                    ui.label("Nenhum dispositivo");
                }
            });
        if let Some(id) = chosen {
            if slot < self.live.slots.len() {
                self.live.slots[slot] = Some(id);
            }
            self.live.pick_slot = None;
        } else if !open {
            self.live.pick_slot = None;
        }
    }

    // ---------------------------------------------------------------- playback

    pub fn playback_page(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.store.device(&self.pb.dev).cloned() else {
            self.page = Page::Main;
            return;
        };
        let now = model::now_i();
        let mut live = false;
        let back = ui::top_bar(ui, &format!("Reprodução · {}", d.name), true, |ui| {
            if ui::primary(ui, "Ao vivo").clicked() {
                live = true;
            }
            if ui::secondary(ui, "Selecionar Data").clicked() {
                self.pb.cal_open = !self.pb.cal_open;
                let dt = model::local(self.pb.day0);
                self.pb.cal_month = (dt.year(), dt.month());
            }
            ui.label(egui::RichText::new(model::fmt_date(self.pb.day0)).strong().color(TEXT));
        });
        if back {
            self.page = Page::Main;
            return;
        }
        if live {
            self.open_live(std::slice::from_ref(&d.id));
            return;
        }

        let segs: Vec<model::Segment> = d
            .segments(self.pb.day0, now)
            .into_iter()
            .filter(|s| match self.pb.filter {
                1 => s.alarm,
                2 => !s.alarm,
                _ => true,
            })
            .collect();

        // advance time
        let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
        if self.pb.playing {
            self.pb.t += dt * self.pb.speed as f64;
            if !segs.iter().any(|s| self.pb.t >= s.start as f64 && self.pb.t < s.end as f64) {
                match segs.iter().find(|s| s.start as f64 > self.pb.t) {
                    Some(s) => self.pb.t = s.start as f64,
                    None => {
                        self.pb.playing = false;
                        self.pb.t = self.pb.t.min(now as f64);
                    }
                }
            }
            // keep the playhead visible
            let half = self.pb.span_h as f64 * 1800.0;
            if self.pb.t > self.pb.center + half * 0.8 || self.pb.t < self.pb.center - half {
                self.pb.center = self.pb.t;
            }
        }

        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let list_w = 300.0;
        let main = Rect::from_min_max(full.min + egui::vec2(12.0, 12.0), egui::pos2(full.right() - list_w - 12.0, full.bottom() - 12.0));
        let list = Rect::from_min_max(egui::pos2(full.right() - list_w, full.top() + 12.0), egui::pos2(full.right() - 10.0, full.bottom() - 12.0));
        let timeline_h = 150.0;
        let bar_h = 54.0;
        let vid_area = Rect::from_min_max(main.min, egui::pos2(main.right(), main.bottom() - timeline_h - bar_h - 8.0));
        let vr = ui::fit_16_9(vid_area);
        let p = ui.painter().clone();
        p.rect_filled(Rect::from_min_max(vid_area.min, egui::pos2(vid_area.right(), vid_area.bottom())), CornerRadius::same(10), Color32::from_rgb(0x12, 0x14, 0x1A));
        let has = segs.iter().any(|s| self.pb.t >= s.start as f64 && self.pb.t < s.end as f64);
        if has {
            let tex = self.feeds.get(ui.ctx(), &format!("pb:{}", d.id), d.seed, d.scene, self.pb.t, if d.settings.record_hd { [640, 360] } else { [320, 180] }, d.look(), View::default(), 12.0, !self.pb.playing);
            p.image(tex, vr, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
            ui::osd_text(&p, vr.left_top() + egui::vec2(10.0, 8.0), egui::Align2::LEFT_TOP, &model::fmt_datetime(self.pb.t as i64), 15.0);
            ui::osd_text(&p, vr.right_bottom() - egui::vec2(10.0, 8.0), egui::Align2::RIGHT_BOTTOM, &d.name, 15.0);
            if self.pb.speed_level != 0 {
                ui::osd_text(&p, vr.right_top() + egui::vec2(-10.0, 8.0), egui::Align2::RIGHT_TOP, &speed_label(self.pb.speed_level), 14.0);
            }
        } else {
            let msg = if !d.is_demo() { "As gravações do cartão deste dispositivo não podem ser lidas nesta versão." } else if segs.is_empty() { "Não há vídeo" } else { "Não há vídeo neste horário — escolha um trecho na linha do tempo" };
            p.text(vr.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(15.0), Color32::from_gray(200));
        }

        // control bar
        let bar = Rect::from_min_size(egui::pos2(main.left(), vid_area.bottom() + 4.0), egui::vec2(main.width(), bar_h));
        p.rect_filled(bar, CornerRadius::same(10), VIDEO_BAR);
        {
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(egui::vec2(12.0, 7.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let w = Color32::WHITE;
            if ui::bar_button(&mut c, if self.pb.playing { Icon::Pause } else { Icon::Play }, if self.pb.playing { "Pausar" } else { "Reproduzir" }, false, w).clicked() {
                self.pb.playing = !self.pb.playing;
                if self.pb.playing && !has {
                    if let Some(s) = segs.iter().find(|s| s.end as f64 > self.pb.t).or(segs.first()) {
                        self.pb.t = self.pb.t.max(s.start as f64);
                        if self.pb.t >= s.end as f64 {
                            self.pb.t = s.start as f64;
                        }
                    }
                }
            }
            let lvl = self.pb.speed_level;
            let slow_txt = if lvl < 0 { format!("{}x", lvl * 2) } else { "Lento".to_string() };
            if ui::text_button(&mut c, &slow_txt, "Avanço lento (-2x, -4x)", lvl < 0).clicked() {
                self.pb.speed_level = next_speed_level(lvl, 2);
            }
            if ui::text_button(&mut c, "1X", "Velocidade normal", lvl == 0).clicked() {
                self.pb.speed_level = next_speed_level(lvl, 0);
            }
            let fast_txt = if lvl > 0 { format!("{}x", lvl * 2) } else { "Rápido".to_string() };
            if ui::text_button(&mut c, &fast_txt, "Avanço rápido (2x, 4x)", lvl > 0).clicked() {
                self.pb.speed_level = next_speed_level(lvl, 1);
            }
            self.pb.speed = speed_of_level(self.pb.speed_level);
            if ui::bar_button(&mut c, Icon::Snapshot, "Instantâneo", false, w).clicked() {
                if has {
                    let t = self.pb.t;
                    self.snapshot(&d.id, t, View::default());
                } else {
                    self.toast("Não há vídeo neste horário");
                }
            }
            c.with_layout(egui::Layout::right_to_left(egui::Align::Center), |c| {
                c.label(egui::RichText::new(model::fmt_time(self.pb.t as i64)).monospace().size(16.0).color(Color32::WHITE));
            });
        }

        // timeline
        let tl = Rect::from_min_max(egui::pos2(main.left(), bar.bottom() + 8.0), main.max);
        self.timeline(ui, tl, &d, &segs);

        // file list
        let mut lc = ui.new_child(egui::UiBuilder::new().max_rect(list));
        self.playback_list(&mut lc, &d, &segs);

        if self.pb.cal_open {
            self.calendar(ui.ctx(), &d);
        }
    }

    fn timeline(&mut self, ui: &mut egui::Ui, tl: Rect, d: &Device, segs: &[model::Segment]) {
        let p = ui.painter().clone();
        p.rect_filled(tl, CornerRadius::same(10), Color32::WHITE);
        p.rect_stroke(tl, CornerRadius::same(10), Stroke::new(1.0_f32, ui::DIVIDER), egui::StrokeKind::Inside);
        // day chips and filters
        let mut top = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(tl.min + egui::vec2(10.0, 8.0), egui::vec2(tl.width() - 20.0, 30.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let today = model::day_start(model::now_i());
        for i in (0..7).rev() {
            let day = model::day_start(today - i * 86400 + 3600);
            let label = if i == 0 { "Hoje".to_string() } else { model::local(day).format("%d/%m").to_string() };
            if ui::chip(&mut top, self.pb.day0 == day, &label).clicked() {
                self.pb.day0 = day;
                self.pb.center = day as f64 + 43200.0;
                self.pb.span_h = 24.0;
                self.pb.t = day as f64;
                self.pb.playing = false;
            }
        }

        let bar = Rect::from_min_max(egui::pos2(tl.left() + 16.0, tl.top() + 56.0), egui::pos2(tl.right() - 16.0, tl.top() + 96.0));
        let resp = ui.interact(bar.expand2(egui::vec2(0.0, 24.0)), ui.id().with("timeline"), Sense::click_and_drag());
        let span = (self.pb.span_h.clamp(0.25, 24.0) as f64) * 3600.0;
        let day0 = self.pb.day0 as f64;
        self.pb.center = self.pb.center.clamp(day0 + span / 2.0, day0 + 86400.0 - span / 2.0);
        let t0 = self.pb.center - span / 2.0;
        let x_of = |t: f64| bar.left() + ((t - t0) / span) as f32 * bar.width();
        let t_of = |x: f32| t0 + ((x - bar.left()) / bar.width()) as f64 * span;
        if resp.hovered() {
            let dz = ui.input(|i| i.smooth_scroll_delta.y);
            if dz != 0.0 {
                self.pb.span_h = (self.pb.span_h * (1.0 - dz * 0.002)).clamp(0.25, 24.0);
            }
        }
        if resp.dragged() {
            self.pb.center -= resp.drag_delta().x as f64 / bar.width() as f64 * span;
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                self.pb.t = t_of(pos.x).clamp(day0, day0 + 86399.0);
                self.pb.playing = true;
            }
        }
        p.rect_filled(bar, CornerRadius::same(4), Color32::from_rgb(0xF5, 0xF6, 0xF8));
        let clip = p.with_clip_rect(bar);
        for s in segs {
            let r = Rect::from_min_max(egui::pos2(x_of(s.start as f64), bar.top() + 6.0), egui::pos2(x_of(s.end as f64).max(x_of(s.start as f64) + 1.5), bar.bottom() - 6.0));
            clip.rect_filled(r, CornerRadius::ZERO, if s.alarm && d.settings.record_mode == model::RecordMode::Alarm { ui::RED } else { Color32::from_rgb(0x7F, 0xDB, 0xCB) });
        }
        if d.settings.record_mode == model::RecordMode::Always {
            let (wa, wb) = d.record_window(model::now_i());
            for (a, b) in d.alarm_marks((t0 as i64).max(wa), ((t0 + span) as i64).min(wb)) {
                let r = Rect::from_min_max(egui::pos2(x_of(a as f64), bar.top() + 6.0), egui::pos2(x_of(b as f64).max(x_of(a as f64) + 2.0), bar.bottom() - 6.0));
                clip.rect_filled(r, CornerRadius::ZERO, ui::RED);
            }
        }
        // hour ticks
        let step = if span <= 3600.0 { 600.0 } else if span <= 4.0 * 3600.0 { 1800.0 } else if span <= 12.0 * 3600.0 { 3600.0 } else { 7200.0 };
        let mut t = ((t0 - day0) / step).ceil() * step + day0;
        while t <= t0 + span {
            let x = x_of(t);
            p.vline(x, bar.bottom()..=bar.bottom() + 6.0, Stroke::new(1.0_f32, TEXT3));
            p.text(egui::pos2(x, bar.bottom() + 8.0), egui::Align2::CENTER_TOP, model::local(t as i64).format("%H:%M").to_string(), egui::FontId::proportional(11.5), TEXT2);
            t += step;
        }
        // now marker
        let nowx = x_of(model::now_f());
        if nowx > bar.left() && nowx < bar.right() {
            p.vline(nowx, bar.y_range(), Stroke::new(1.0_f32, ui::ORANGE));
        }
        // playhead
        let px = x_of(self.pb.t);
        if px >= bar.left() && px <= bar.right() {
            p.vline(px, bar.top() - 8.0..=bar.bottom() + 2.0, Stroke::new(2.0_f32, THEME));
            p.circle_filled(egui::pos2(px, bar.top() - 8.0), 5.0, THEME);
        }
        p.text(egui::pos2(tl.left() + 16.0, tl.bottom() - 10.0), egui::Align2::LEFT_BOTTOM, "Role para aproximar · arraste para mover · clique para reproduzir", egui::FontId::proportional(11.5), TEXT3);
        let legend_x = tl.right() - 16.0;
        p.text(egui::pos2(legend_x, tl.bottom() - 10.0), egui::Align2::RIGHT_BOTTOM, "■ Alarme", egui::FontId::proportional(11.5), ui::RED);
        p.text(egui::pos2(legend_x - 70.0, tl.bottom() - 10.0), egui::Align2::RIGHT_BOTTOM, "■ Gravação", egui::FontId::proportional(11.5), Color32::from_rgb(0x3F, 0xC1, 0xAA));
    }

    fn playback_list(&mut self, ui: &mut egui::Ui, d: &Device, segs: &[model::Segment]) {
        ui.label(egui::RichText::new(format!("Arquivos de vídeo · {}", model::fmt_date(self.pb.day0))).strong().color(TEXT));
        ui.label(egui::RichText::new(match d.settings.record_mode {
            model::RecordMode::Always => "Modo: regular (sempre a gravar)",
            model::RecordMode::Alarm => "Modo: alarme (gravação somente quando o alarme é acionado)",
            model::RecordMode::Off => "Modo: não registrar",
        }).small().color(TEXT3));
        ui.horizontal_wrapped(|ui| {
            for (i, label) in [(0u8, "Todas as gravações"), (1, "Registro de alarme"), (2, "Gravação geral")] {
                if ui::chip(ui, self.pb.filter == i, label).clicked() {
                    self.pb.filter = i;
                }
            }
        });
        ui.add_space(4.0);
        if segs.is_empty() {
            ui.label(egui::RichText::new("Não há vídeo").color(TEXT2));
            return;
        }
        let rows: Vec<model::Segment> = segs.iter().rev().copied().collect();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, 52.0, rows.len(), |ui, range| {
            for s in &rows[range] {
                let cur = self.pb.t >= s.start as f64 && self.pb.t < s.end as f64;
                let resp = egui::Frame::new()
                    .fill(if cur { ui::THEME_SOFT } else { Color32::WHITE })
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(6.0, 30.0), Sense::hover());
                            ui.painter().rect_filled(r, CornerRadius::same(3), if s.alarm { ui::RED } else { THEME });
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(format!("{} - {}", model::fmt_time(s.start), model::fmt_time(s.end))).color(TEXT));
                                ui.label(egui::RichText::new(format!("{} · {}", if s.alarm { "Registro de alarme" } else { "Gravação geral" }, model::fmt_dur(s.end - s.start))).small().color(TEXT3));
                            });
                        });
                    })
                    .response
                    .interact(Sense::click());
                if resp.clicked() {
                    self.pb.t = s.start as f64;
                    self.pb.center = s.start as f64;
                    self.pb.playing = true;
                }
                ui.add_space(2.0);
            }
        });
    }

    fn calendar(&mut self, ctx: &egui::Context, d: &Device) {
        let mut open = true;
        let (y, m) = self.pb.cal_month;
        let mut pick: Option<i64> = None;
        let now = model::now_i();
        egui::Window::new("Selecionar Data")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("‹").clicked() {
                        self.pb.cal_month = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
                    }
                    const MONTHS: [&str; 12] = ["Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho", "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro"];
                    ui.label(egui::RichText::new(format!("{} {}", MONTHS[(m as usize + 11) % 12], y)).strong());
                    if ui.button("›").clicked() {
                        self.pb.cal_month = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                    }
                });
                let Some(first) = NaiveDate::from_ymd_opt(y, m, 1) else { return };
                egui::Grid::new("cal").spacing([4.0, 4.0]).show(ui, |ui| {
                    for wd in ["D", "S", "T", "Q", "Q", "S", "S"] {
                        ui.label(egui::RichText::new(wd).color(TEXT3));
                    }
                    ui.end_row();
                    let offset = first.weekday().num_days_from_sunday();
                    for _ in 0..offset {
                        ui.label("");
                    }
                    let mut col = offset;
                    let mut day = first;
                    while day.month() == m {
                        let ds = model::date_start(day).unwrap_or(0);
                        let has = !d.segments(ds, now).is_empty();
                        let future = ds > now;
                        let sel = ds == self.pb.day0;
                        let txt = egui::RichText::new(day.day().to_string()).color(if future { TEXT3 } else if sel { Color32::WHITE } else { TEXT });
                        let mut b = egui::Button::new(txt).min_size(egui::vec2(34.0, 30.0));
                        if sel {
                            b = b.fill(THEME);
                        } else if has {
                            b = b.fill(ui::THEME_SOFT);
                        }
                        if ui.add_enabled(!future, b).clicked() {
                            pick = Some(ds);
                        }
                        col += 1;
                        if col % 7 == 0 {
                            ui.end_row();
                        }
                        day = day.succ_opt().unwrap_or(day);
                        if day.day() == 1 {
                            break;
                        }
                    }
                });
                ui.label(egui::RichText::new("Dias com gravação aparecem destacados.").small().color(TEXT3));
            });
        if let Some(ds) = pick {
            self.pb.day0 = ds;
            self.pb.t = ds as f64;
            self.pb.center = ds as f64 + 43200.0;
            self.pb.span_h = 24.0;
            self.pb.playing = false;
            self.pb.cal_open = false;
            if let Some(s) = d.segments(ds, now).first() {
                self.pb.t = s.start as f64;
            }
        } else if !open {
            self.pb.cal_open = false;
        }
    }
}
