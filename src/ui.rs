//! Colours, small widgets and the picture caches.

use crate::scene::{self, Look, View};
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, TextureHandle, Ui, Vec2};
use std::collections::HashMap;
use std::time::Instant;

/// app_theme_color #00BFA5 (colors.xml)
pub const THEME: Color32 = Color32::from_rgb(0x00, 0xBF, 0xA5);
pub const THEME_DARK: Color32 = Color32::from_rgb(0x00, 0x9E, 0x88);
pub const THEME_SOFT: Color32 = Color32::from_rgb(0xE0, 0xF7, 0xF3);
/// bg_color #F5F5F5
pub const BG: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
pub const CARD: Color32 = Color32::WHITE;
/// algo_title_text #333333, algo_right_text #666666, algo_subtitle_text #999999
pub const TEXT: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
pub const TEXT2: Color32 = Color32::from_rgb(0x66, 0x66, 0x66);
pub const TEXT3: Color32 = Color32::from_rgb(0x99, 0x99, 0x99);
/// algo_divider #F0F0F0
pub const DIVIDER: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xE8);
/// algo_beta_mark_text #F04438, algo_exclusion_tip #FF9500
pub const RED: Color32 = Color32::from_rgb(0xF0, 0x44, 0x38);
pub const ORANGE: Color32 = Color32::from_rgb(0xFF, 0x95, 0x00);

pub const VIDEO_BAR: Color32 = Color32::from_rgb(0x26, 0x2A, 0x36);

pub fn setup_style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::light());
    ctx.style_mut(|s| {
        use egui::{FontFamily, FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(15.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Heading, FontId::new(21.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(12.0, FontFamily::Proportional));
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        s.visuals.selection.bg_fill = THEME;
        s.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        s.visuals.hyperlink_color = THEME_DARK;
        s.visuals.panel_fill = BG;
        s.visuals.window_fill = Color32::WHITE;
        s.visuals.window_corner_radius = CornerRadius::same(12);
        s.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
        s.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
        s.visuals.widgets.active.corner_radius = CornerRadius::same(6);
        s.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xF0, 0xF1, 0xF3);
        s.visuals.widgets.inactive.bg_fill = Color32::from_rgb(0xE4, 0xE6, 0xEA);
        s.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xE4, 0xF5, 0xF2);
        s.visuals.widgets.hovered.bg_fill = THEME_SOFT;
        s.visuals.widgets.active.weak_bg_fill = THEME_SOFT;
        s.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
        s.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
        s.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xDD, 0xDF, 0xE3));
        s.visuals.extreme_bg_color = Color32::from_rgb(0xF7, 0xF8, 0xF9);
        s.visuals.faint_bg_color = Color32::from_rgb(0xFA, 0xFA, 0xFA);
    });
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// Switch like the original's toggles.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(44.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let how = ui.ctx().animate_bool(resp.id, *on);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(12), lerp_color(Color32::from_rgb(0xCF, 0xD3, 0xD8), THEME, how));
    let x = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, how);
    p.circle_filled(egui::pos2(x, rect.center().y), 9.5, Color32::WHITE);
    resp
}

pub fn primary(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong())
            .fill(THEME)
            .corner_radius(CornerRadius::same(18))
            .min_size(egui::vec2(0.0, 34.0)),
    )
}

pub fn secondary(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(THEME_DARK))
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0_f32, THEME))
            .corner_radius(CornerRadius::same(18))
            .min_size(egui::vec2(0.0, 34.0)),
    )
}

pub fn danger(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong())
            .fill(RED)
            .corner_radius(CornerRadius::same(18))
            .min_size(egui::vec2(0.0, 34.0)),
    )
}

/// A chip that is either selected (theme) or not.
pub fn chip(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let (fill, col, stroke) = if selected {
        (THEME, Color32::WHITE, Stroke::NONE)
    } else {
        (Color32::WHITE, TEXT2, Stroke::new(1.0_f32, DIVIDER))
    };
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(col))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(14))
            .min_size(egui::vec2(0.0, 28.0)),
    )
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(14))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0xEC, 0xEC, 0xEC)))
}

/// Title bar of a page. Returns true when the back arrow was clicked.
pub fn top_bar(ui: &mut Ui, title: &str, back: bool, right: impl FnOnce(&mut Ui)) -> bool {
    let mut clicked = false;
    egui::Frame::new()
        .fill(Color32::WHITE)
        .inner_margin(egui::Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_min_height(36.0);
            ui.horizontal(|ui| {
                if back {
                    let r = ui.add(egui::Button::new(egui::RichText::new("‹").size(26.0).color(TEXT)).frame(false));
                    if r.on_hover_text("Voltar").clicked() {
                        clicked = true;
                    }
                }
                ui.label(egui::RichText::new(title).size(19.0).strong().color(TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| right(ui));
            });
        });
    let r = ui.min_rect();
    ui.painter().hline(r.x_range(), ui.cursor().top(), Stroke::new(1.0_f32, DIVIDER));
    clicked
}

/// One row of a settings list: title (and optional hint) on the left, controls on the right.
pub fn row(ui: &mut Ui, title: &str, hint: Option<&str>, right: impl FnOnce(&mut Ui)) -> Response {
    let resp = ui
        .horizontal(|ui| {
            ui.set_min_height(40.0);
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(egui::RichText::new(title).color(TEXT));
                if let Some(h) = hint {
                    ui.label(egui::RichText::new(h).small().color(TEXT3));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| right(ui));
        })
        .response;
    let r = resp.rect;
    ui.painter().hline(r.x_range(), r.bottom() + 3.0, Stroke::new(1.0_f32, Color32::from_rgb(0xF0, 0xF0, 0xF0)));
    resp
}

/// Simple vector icons (drawn, so they look the same everywhere).
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum Icon {
    Camera,
    Bell,
    Image,
    User,
    Plus,
    Share,
    Gear,
    Shield,
    Play,
    Pause,
    Speaker,
    Mute,
    Record,
    Snapshot,
    Mic,
    Fullscreen,
    Grid,
    Single,
    Playback,
    Live,
    Search,
    Folder,
    Trash,
}

pub fn draw_icon(p: &egui::Painter, icon: Icon, rect: Rect, col: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) / 2.0;
    let st = Stroke::new((s * 0.16).max(1.4), col);
    let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_center_size(c + egui::vec2(x * s, y * s), egui::vec2(w * s, h * s));
    let pt = |x: f32, y: f32| c + egui::vec2(x * s, y * s);
    match icon {
        Icon::Camera => {
            p.rect_stroke(r(-0.15, 0.0, 1.3, 1.0), CornerRadius::same(3), st, egui::StrokeKind::Middle);
            p.add(egui::Shape::convex_polygon(vec![pt(0.55, -0.15), pt(0.95, -0.4), pt(0.95, 0.4), pt(0.55, 0.15)], col, Stroke::NONE));
        }
        Icon::Bell => {
            p.add(egui::Shape::convex_polygon(
                vec![pt(-0.55, 0.45), pt(-0.45, -0.25), pt(-0.2, -0.6), pt(0.2, -0.6), pt(0.45, -0.25), pt(0.55, 0.45)],
                Color32::TRANSPARENT,
                st,
            ));
            p.line_segment([pt(-0.75, 0.45), pt(0.75, 0.45)], st);
            p.circle_filled(pt(0.0, 0.7), s * 0.14, col);
        }
        Icon::Image => {
            p.rect_stroke(r(0.0, 0.0, 1.7, 1.4), CornerRadius::same(2), st, egui::StrokeKind::Middle);
            p.add(egui::Shape::convex_polygon(vec![pt(-0.7, 0.55), pt(-0.2, -0.05), pt(0.2, 0.35), pt(0.4, 0.15), pt(0.75, 0.55)], col, Stroke::NONE));
            p.circle_filled(pt(0.35, -0.3), s * 0.15, col);
        }
        Icon::User => {
            p.circle_stroke(pt(0.0, -0.35), s * 0.35, st);
            p.add(egui::Shape::line(
                (0..=16).map(|i| {
                    let a = std::f32::consts::PI * (1.0 + i as f32 / 16.0);
                    pt(0.75 * a.cos(), 0.85 + 0.6 * a.sin())
                }).collect(),
                st,
            ));
        }
        Icon::Plus => {
            p.line_segment([pt(-0.7, 0.0), pt(0.7, 0.0)], st);
            p.line_segment([pt(0.0, -0.7), pt(0.0, 0.7)], st);
        }
        Icon::Share => {
            let a = pt(0.55, -0.55);
            let b = pt(-0.55, 0.0);
            let d = pt(0.55, 0.55);
            p.line_segment([a, b], st);
            p.line_segment([b, d], st);
            for q in [a, b, d] {
                p.circle_filled(q, s * 0.22, col);
            }
        }
        Icon::Gear => {
            for i in 0..8 {
                let ang = i as f32 * std::f32::consts::FRAC_PI_4;
                p.line_segment([pt(0.5 * ang.cos(), 0.5 * ang.sin()), pt(0.85 * ang.cos(), 0.85 * ang.sin())], Stroke::new(st.width * 1.6, col));
            }
            p.circle_stroke(c, s * 0.5, st);
            p.circle_stroke(c, s * 0.18, st);
        }
        Icon::Shield => {
            p.add(egui::Shape::convex_polygon(
                vec![pt(0.0, -0.8), pt(0.7, -0.5), pt(0.6, 0.25), pt(0.0, 0.8), pt(-0.6, 0.25), pt(-0.7, -0.5)],
                Color32::TRANSPARENT,
                st,
            ));
            p.line_segment([pt(-0.3, 0.0), pt(-0.05, 0.25)], st);
            p.line_segment([pt(-0.05, 0.25), pt(0.35, -0.2)], st);
        }
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(vec![pt(-0.45, -0.65), pt(0.65, 0.0), pt(-0.45, 0.65)], col, Stroke::NONE));
        }
        Icon::Pause => {
            p.rect_filled(r(-0.3, 0.0, 0.3, 1.3), CornerRadius::same(1), col);
            p.rect_filled(r(0.3, 0.0, 0.3, 1.3), CornerRadius::same(1), col);
        }
        Icon::Speaker | Icon::Mute => {
            p.add(egui::Shape::convex_polygon(vec![pt(-0.8, -0.25), pt(-0.45, -0.25), pt(0.0, -0.7), pt(0.0, 0.7), pt(-0.45, 0.25), pt(-0.8, 0.25)], col, Stroke::NONE));
            if icon == Icon::Speaker {
                p.add(egui::Shape::line((0..=10).map(|i| { let a = -0.9 + 1.8 * i as f32 / 10.0; pt(0.15 + 0.35 * a.cos(), 0.45 * a.sin()) }).collect(), st));
                p.add(egui::Shape::line((0..=10).map(|i| { let a = -0.9 + 1.8 * i as f32 / 10.0; pt(0.15 + 0.65 * a.cos(), 0.8 * a.sin()) }).collect(), st));
            } else {
                p.line_segment([pt(0.25, -0.3), pt(0.85, 0.3)], st);
                p.line_segment([pt(0.25, 0.3), pt(0.85, -0.3)], st);
            }
        }
        Icon::Record => {
            p.circle_stroke(c, s * 0.8, st);
            p.circle_filled(c, s * 0.45, col);
        }
        Icon::Snapshot => {
            p.rect_stroke(r(0.0, 0.1, 1.7, 1.2), CornerRadius::same(3), st, egui::StrokeKind::Middle);
            p.rect_filled(r(0.0, -0.6, 0.6, 0.25), CornerRadius::same(1), col);
            p.circle_stroke(pt(0.0, 0.1), s * 0.35, st);
        }
        Icon::Mic => {
            p.rect_stroke(r(0.0, -0.25, 0.55, 1.0), CornerRadius::same(6), st, egui::StrokeKind::Middle);
            p.add(egui::Shape::line((0..=12).map(|i| { let a = std::f32::consts::PI * i as f32 / 12.0; pt(0.55 * a.cos(), 0.05 + 0.5 * a.sin()) }).collect(), st));
            p.line_segment([pt(0.0, 0.55), pt(0.0, 0.85)], st);
        }
        Icon::Fullscreen => {
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = pt(0.75 * x, 0.75 * y);
                p.line_segment([corner, pt(0.75 * x - 0.45 * x, 0.75 * y)], st);
                p.line_segment([corner, pt(0.75 * x, 0.75 * y - 0.45 * y)], st);
            }
        }
        Icon::Grid => {
            for (x, y) in [(-0.4, -0.4), (0.4, -0.4), (-0.4, 0.4), (0.4, 0.4)] {
                p.rect_stroke(r(x, y, 0.6, 0.6), CornerRadius::same(1), st, egui::StrokeKind::Middle);
            }
        }
        Icon::Single => {
            p.rect_stroke(r(0.0, 0.0, 1.5, 1.2), CornerRadius::same(2), st, egui::StrokeKind::Middle);
        }
        Icon::Playback => {
            p.circle_stroke(c, s * 0.8, st);
            p.line_segment([c, pt(0.0, -0.5)], st);
            p.line_segment([c, pt(0.35, 0.2)], st);
        }
        Icon::Live => {
            p.circle_filled(c, s * 0.3, col);
            p.add(egui::Shape::line((0..=10).map(|i| { let a = -0.8 + 1.6 * i as f32 / 10.0; pt(0.7 * a.cos(), 0.7 * a.sin()) }).collect(), st));
            p.add(egui::Shape::line((0..=10).map(|i| { let a = std::f32::consts::PI - 0.8 + 1.6 * i as f32 / 10.0; pt(0.7 * a.cos(), 0.7 * a.sin()) }).collect(), st));
        }
        Icon::Search => {
            p.circle_stroke(pt(-0.15, -0.15), s * 0.5, st);
            p.line_segment([pt(0.22, 0.22), pt(0.7, 0.7)], st);
        }
        Icon::Folder => {
            p.add(egui::Shape::convex_polygon(vec![pt(-0.8, -0.55), pt(-0.25, -0.55), pt(-0.1, -0.35), pt(0.8, -0.35), pt(0.8, 0.6), pt(-0.8, 0.6)], Color32::TRANSPARENT, st));
        }
        Icon::Trash => {
            p.line_segment([pt(-0.7, -0.5), pt(0.7, -0.5)], st);
            p.line_segment([pt(-0.2, -0.7), pt(0.2, -0.7)], st);
            p.add(egui::Shape::convex_polygon(vec![pt(-0.5, -0.5), pt(0.5, -0.5), pt(0.4, 0.75), pt(-0.4, 0.75)], Color32::TRANSPARENT, st));
        }
    }
}

/// Icon above a short label, like the original's ButtonTouch.
pub fn icon_label_button(ui: &mut Ui, icon: Icon, label: &str, size: Vec2, col: Color32, badge: usize) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hover = resp.hovered();
    let p = ui.painter();
    if hover {
        p.rect_filled(rect, CornerRadius::same(8), Color32::from_rgba_unmultiplied(0, 191, 165, 22));
    }
    let col = if hover { THEME_DARK } else { col };
    let ir = Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + size.y * 0.36), egui::vec2(20.0, 20.0));
    draw_icon(p, icon, ir, col);
    p.text(
        egui::pos2(rect.center().x, rect.bottom() - 6.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::proportional(12.0),
        col,
    );
    if badge > 0 {
        let bp = ir.right_top() + egui::vec2(4.0, 0.0);
        let txt = if badge > 99 { "99+".to_string() } else { badge.to_string() };
        let w = 8.0 + 6.0 * txt.len() as f32;
        p.rect_filled(Rect::from_center_size(bp, egui::vec2(w, 15.0)), CornerRadius::same(8), RED);
        p.text(bp, egui::Align2::CENTER_CENTER, txt, egui::FontId::proportional(10.0), Color32::WHITE);
    }
    resp
}

/// Round icon button for dark video bars.
pub fn bar_button(ui: &mut Ui, icon: Icon, tip: &str, active: bool, tint: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() || active {
        p.circle_filled(rect.center(), 19.0, if active { Color32::from_rgba_unmultiplied(0, 191, 165, 90) } else { Color32::from_white_alpha(28) });
    }
    draw_icon(p, icon, rect.shrink(11.0), tint);
    resp.on_hover_text(tip)
}

pub fn text_button(ui: &mut Ui, text: &str, tip: &str, active: bool) -> Response {
    let fill = if active { THEME } else { Color32::from_rgb(0x3A, 0x3F, 0x4D) };
    ui.add(egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong()).fill(fill).stroke(Stroke::NONE).corner_radius(CornerRadius::same(12)))
        .on_hover_text(tip)
}

/// The simulated pictures, redrawn at a limited rate.
pub struct Feeds {
    map: HashMap<String, (TextureHandle, Instant, FeedKey)>,
    used: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
struct FeedKey {
    seed: u64,
    scene: u8,
    w: usize,
    h: usize,
    look: Look,
    view: View,
    t_frozen: Option<i64>,
}

impl Default for Feeds {
    fn default() -> Self {
        Feeds { map: HashMap::new(), used: Vec::new() }
    }
}

#[allow(clippy::too_many_arguments)]
impl Feeds {
    /// fps: how often the picture is redrawn; `frozen` keeps a still picture.
    pub fn get(&mut self, ctx: &egui::Context, key: &str, seed: u64, scene_kind: u8, t: f64, size: [usize; 2], look: Look, view: View, fps: f32, frozen: bool) -> egui::TextureId {
        let fk = FeedKey {
            seed,
            scene: scene_kind,
            w: size[0],
            h: size[1],
            look,
            view,
            t_frozen: if frozen { Some((t * 10.0) as i64) } else { None },
        };
        self.used.push(key.to_string());
        if let Some((tex, at, old)) = self.map.get_mut(key) {
            let stale = at.elapsed().as_secs_f32() > 1.0 / fps.max(0.1);
            if *old != fk || (stale && !frozen) {
                tex.set(scene::render(seed, scene_kind, t, size[0], size[1], &look, &view), egui::TextureOptions::LINEAR);
                *at = Instant::now();
                *old = fk;
            }
            return tex.id();
        }
        let tex = ctx.load_texture(key, scene::render(seed, scene_kind, t, size[0], size[1], &look, &view), egui::TextureOptions::LINEAR);
        let id = tex.id();
        self.map.insert(key.to_string(), (tex, Instant::now(), fk));
        id
    }

    /// Forget pictures that were not shown this frame.
    pub fn end_frame(&mut self) {
        let used: std::collections::HashSet<String> = self.used.drain(..).collect();
        self.map.retain(|k, _| used.contains(k));
    }
}

/// Pictures loaded from files (album).
#[derive(Default)]
pub struct Images {
    map: HashMap<String, Option<TextureHandle>>,
}

impl Images {
    pub fn get(&mut self, ctx: &egui::Context, path: &str) -> Option<(egui::TextureId, Vec2)> {
        if !self.map.contains_key(path) {
            let tex = image::open(path).ok().map(|img| {
                let img = img.thumbnail(960, 540).to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                ctx.load_texture(path, egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR)
            });
            if self.map.len() > 300 {
                self.map.clear();
            }
            self.map.insert(path.to_string(), tex);
        }
        self.map.get(path).and_then(|t| t.as_ref()).map(|t| (t.id(), t.size_vec2()))
    }

    pub fn forget(&mut self, path: &str) {
        self.map.remove(path);
    }
}

pub fn paint_image(p: &egui::Painter, tex: egui::TextureId, rect: Rect, radius: u8) {
    if radius == 0 {
        p.image(tex, rect, Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        p.add(egui::Shape::Rect(
            egui::epaint::RectShape::filled(rect, CornerRadius::same(radius), Color32::WHITE)
                .with_texture(tex, Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0))),
        ));
    }
}

/// Fit a 16:9 box inside `avail`.
pub fn fit_16_9(avail: Rect) -> Rect {
    let w = avail.width().min(avail.height() * 16.0 / 9.0);
    let h = w * 9.0 / 16.0;
    Rect::from_center_size(avail.center(), egui::vec2(w, h))
}

pub fn osd_text(p: &egui::Painter, pos: Pos2, align: egui::Align2, text: &str, size: f32) {
    p.text(pos + egui::vec2(1.0, 1.0), align, text, egui::FontId::monospace(size), Color32::from_black_alpha(160));
    p.text(pos, align, text, egui::FontId::monospace(size), Color32::WHITE);
}
