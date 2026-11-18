//! Simulated camera picture and the motion events that happen in it.
//!
//! Everything is a pure function of (camera seed, time), so the live view, the
//! recordings, the alarm messages and the album replays always agree.

use chrono::{Local, TimeZone, Timelike};
use eframe::egui::ColorImage;

pub fn hash64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn h2(a: u64, b: u64) -> u64 {
    hash64(a ^ hash64(b))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvKind {
    Human,
    Motion,
}

#[derive(Clone, Copy, Debug)]
pub struct RawEvent {
    pub ts: i64,
    pub kind: EvKind,
    pub strength: u8,
    pub dur: f64,
    pub dir_left: bool,
    pub tint: u64,
}

/// Length of one schedule slot in seconds.
pub const SLOT: i64 = 600;
/// Chance (in %) that a slot holds a movement.
pub const SLOT_CHANCE: u64 = 22;

/// Every movement that really happens in front of the camera.
pub fn raw_events(seed: u64, from: i64, to: i64) -> Vec<RawEvent> {
    let mut out = Vec::new();
    if to <= from {
        return out;
    }
    let k0 = from.div_euclid(SLOT);
    let k1 = to.div_euclid(SLOT);
    for k in k0..=k1 {
        let h = h2(seed, k as u64);
        if h % 100 < SLOT_CHANCE {
            let ts = k * SLOT + ((h >> 8) % 540) as i64;
            if ts >= from && ts < to {
                let kind = if (h >> 20) & 1 == 0 { EvKind::Human } else { EvKind::Motion };
                out.push(RawEvent {
                    ts,
                    kind,
                    strength: ((h >> 24) % 3) as u8,
                    dur: if kind == EvKind::Human { 14.0 } else { 7.0 },
                    dir_left: (h >> 30) & 1 == 1,
                    tint: h >> 32,
                });
            }
        }
    }
    out
}

#[derive(Clone, Copy)]
pub struct EventFilter {
    pub motion: bool,
    pub human: bool,
    /// 0 low, 1 medium, 2 high
    pub sensitivity: u8,
    /// minimum seconds between two alarms
    pub interval: i64,
}

/// Movements the camera turns into alarms with its current settings.
pub fn alarm_events(seed: u64, f: &EventFilter, from: i64, to: i64) -> Vec<RawEvent> {
    if !f.motion && !f.human {
        return Vec::new();
    }
    let lead = f.interval.max(0) + SLOT;
    let mut last: Option<i64> = None;
    let mut out = Vec::new();
    for e in raw_events(seed, from - lead, to) {
        if e.strength + f.sensitivity.min(2) < 2 {
            continue;
        }
        if !f.motion && e.kind != EvKind::Human {
            continue;
        }
        if let Some(l) = last {
            if e.ts - l < f.interval {
                continue;
            }
        }
        last = Some(e.ts);
        if e.ts >= from {
            out.push(e);
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
pub struct Look {
    pub flip_lr: bool,
    pub flip_ud: bool,
    /// DayNightSwitch.SwitchMode: 0 automatic, 1 day (colour), 2 night (black and white), 3 timed
    pub night_mode: u8,
    /// Automatic switch sensitivity 0..10
    pub dn_sensitivity: u8,
    /// Timed switch: night from / to (hours)
    pub night_from: u8,
    pub night_to: u8,
}

impl Look {
    pub fn simple(flip_lr: bool, flip_ud: bool, night_mode: u8) -> Look {
        Look { flip_lr, flip_ud, night_mode, dn_sensitivity: 5, night_from: 18, night_to: 6 }
    }

    /// Whether the camera shows its black-and-white night picture at time t.
    /// Automatic: switches when daylight drops under 0.10 + 0.05 × sensitivity (estimated;
    /// the original only sends the threshold DncThr = 50 − 4 × sensitivity to the camera).
    pub fn night_picture(&self, t: f64) -> bool {
        match self.night_mode {
            0 => daylight(t) < 0.10 + 0.05 * self.dn_sensitivity.min(10) as f32,
            2 => true,
            3 => {
                let h = local_hour(t) as u8;
                if self.night_from <= self.night_to {
                    h >= self.night_from && h < self.night_to
                } else {
                    h >= self.night_from || h < self.night_to
                }
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct View {
    pub pan: f32,
    pub tilt: f32,
    pub zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        View { pan: 0.0, tilt: 0.0, zoom: 1.0 }
    }
}

pub const PAN_LIMIT: f32 = 0.35;
pub const TILT_LIMIT: f32 = 0.12;
pub const ZOOM_MAX: f32 = 4.0;

pub const SCENE_NAMES: [&str; 3] = ["Quintal", "Rua", "Sala"];

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 255) as f32 / 255.0,
        ((hex >> 8) & 255) as f32 / 255.0,
        (hex & 255) as f32 / 255.0,
    ]
}

#[inline]
fn in_rect(u: f32, v: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    u >= x0 && u < x1 && v >= y0 && v < y1
}

const ASPECT: f32 = 16.0 / 9.0;

#[inline]
fn in_circle(u: f32, v: f32, cx: f32, cy: f32, r: f32) -> bool {
    let du = (u - cx) * ASPECT;
    let dv = v - cy;
    du * du + dv * dv < r * r
}

#[inline]
fn in_ellipse(u: f32, v: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> bool {
    let du = (u - cx) / rx;
    let dv = (v - cy) / ry;
    du * du + dv * dv < 1.0
}

/// Something moving in the picture right now.
struct Actor {
    kind: EvKind,
    x: f32,
    phase: f32,
    tint: u64,
    facing_left: bool,
}

const SHIRTS: [u32; 6] = [0xC0392B, 0x2E86C1, 0x27AE60, 0x8E44AD, 0xD68910, 0x34495E];
const CARS: [u32; 5] = [0xB03A2E, 0x1F618D, 0xD5D8DC, 0x212F3C, 0xF1C40F];

fn actors(seed: u64, t: f64) -> Vec<Actor> {
    let ti = t.floor() as i64;
    raw_events(seed, ti - 20, ti + 1)
        .into_iter()
        .filter_map(|e| {
            let p = (t - e.ts as f64) / e.dur;
            if !(0.0..1.0).contains(&p) {
                return None;
            }
            let p = p as f32;
            let x = if e.dir_left { 1.15 - 1.3 * p } else { -0.15 + 1.3 * p };
            Some(Actor {
                kind: e.kind,
                x,
                phase: (t - e.ts as f64) as f32 * 7.0,
                tint: e.tint,
                facing_left: e.dir_left,
            })
        })
        .collect()
}

fn person(u: f32, v: f32, x: f32, feet: f32, hgt: f32, phase: f32, tint: u64) -> Option<[f32; 3]> {
    let head_y = feet - hgt * 0.86;
    if in_circle(u, v, x, head_y, hgt * 0.1) {
        return Some(rgb(0xE0AC69));
    }
    let hw = hgt * 0.07;
    if in_rect(u, v, x - hw, feet - hgt * 0.76, x + hw, feet - hgt * 0.42) {
        return Some(rgb(SHIRTS[(tint % 6) as usize]));
    }
    let swing = phase.sin() * hgt * 0.05;
    let lw = hgt * 0.025;
    if in_rect(u, v, x - hw * 0.6 + swing - lw, feet - hgt * 0.42, x - hw * 0.6 + swing + lw, feet)
        || in_rect(u, v, x + hw * 0.6 - swing - lw, feet - hgt * 0.42, x + hw * 0.6 - swing + lw, feet)
    {
        return Some(rgb(0x2C3E50));
    }
    None
}

fn pet(u: f32, v: f32, x: f32, y: f32, facing_left: bool) -> Option<[f32; 3]> {
    let hx = if facing_left { x - 0.045 } else { x + 0.045 };
    if in_ellipse(u, v, x, y, 0.045, 0.022) || in_circle(u, v, hx, y - 0.02, 0.022) {
        return Some(rgb(0x6E4B2A));
    }
    None
}

/// (colour, emits light)
fn car(u: f32, v: f32, x: f32, y: f32, tint: u64, facing_left: bool, night: bool) -> Option<([f32; 3], bool)> {
    let len = 0.26;
    let front = if facing_left { x - len / 2.0 } else { x + len / 2.0 };
    if night && in_circle(u, v, front, y - 0.02, 0.018) {
        return Some((rgb(0xFFF6C8), true));
    }
    if in_circle(u, v, x - len * 0.3, y + 0.035, 0.032) || in_circle(u, v, x + len * 0.3, y + 0.035, 0.032) {
        return Some((rgb(0x111111), false));
    }
    if in_rect(u, v, x - len / 2.0, y - 0.05, x + len / 2.0, y + 0.035) {
        return Some((rgb(CARS[(tint % 5) as usize]), false));
    }
    if in_rect(u, v, x - len * 0.25, y - 0.11, x + len * 0.22, y - 0.05) {
        return Some((rgb(0x85C1E9), false));
    }
    None
}

struct Ctx {
    seed: u64,
    t: f32,
    day: f32,
    night: bool,
    actors: Vec<Actor>,
}

fn sky(v: f32, c: &Ctx, horizon: f32) -> [f32; 3] {
    let day_top = rgb(0x5DADE2);
    let day_bot = rgb(0xD6EAF8);
    let night_top = rgb(0x0B1026);
    let night_bot = rgb(0x1C2541);
    let top = mix(night_top, day_top, c.day);
    let bot = mix(night_bot, day_bot, c.day);
    mix(top, bot, (v / horizon).clamp(0.0, 1.0))
}

fn cloud(u: f32, v: f32, c: &Ctx) -> bool {
    for i in 0..3u64 {
        let hh = h2(c.seed, 100 + i);
        let speed = 0.002 + (hh % 100) as f32 * 0.00002;
        let cx = ((hh % 997) as f32 / 997.0 + c.t * speed).fract() * 2.2 - 0.6;
        let cy = 0.06 + (hh >> 12) as f32 % 10.0 * 0.012;
        if in_ellipse(u, v, cx, cy, 0.09, 0.03) || in_ellipse(u, v, cx + 0.06, cy - 0.015, 0.06, 0.03) {
            return true;
        }
    }
    false
}

fn scene_garden(u: f32, v: f32, c: &Ctx) -> ([f32; 3], bool) {
    // actors first (front)
    for a in &c.actors {
        match a.kind {
            EvKind::Human => {
                if let Some(col) = person(u, v, a.x, 0.86, 0.34, a.phase, a.tint) {
                    return (col, false);
                }
            }
            EvKind::Motion => {
                if let Some(col) = pet(u, v, a.x, 0.84, a.facing_left) {
                    return (col, false);
                }
            }
        }
    }
    // tree
    let sway = (c.t * 0.6).sin() * 0.006;
    if in_circle(u, v, 0.17 + sway, 0.2, 0.12) || in_circle(u, v, 0.1 + sway, 0.27, 0.08) || in_circle(u, v, 0.25 + sway, 0.27, 0.08) {
        let shade = if ((u * 90.0).sin() * (v * 70.0).cos()) > 0.3 { 0.85 } else { 1.0 };
        let g = rgb(0x2E7D32);
        return ([g[0] * shade, g[1] * shade, g[2] * shade], false);
    }
    if in_rect(u, v, 0.155, 0.25, 0.19, 0.62) {
        return (rgb(0x6D4C41), false);
    }
    // house
    if in_rect(u, v, 0.8, 0.31, 0.92, 0.45) {
        let lit = !c.day.gt(&0.5);
        return if lit { (rgb(0xFFD27F), true) } else { (rgb(0x9CC3D5), false) };
    }
    if in_rect(u, v, 1.0, 0.4, 1.1, 0.64) {
        return (rgb(0x795548), false);
    }
    if in_rect(u, v, 0.72, 0.2, 1.35, 0.64) {
        return (rgb(0xF5E6CC), false);
    }
    let roof_y = 0.2 - (0.3 - (u - 1.03).abs()) * 0.4;
    if u > 0.68 && u < 1.39 && v >= roof_y.max(0.06) && v < 0.2 {
        return (rgb(0xA0522D), false);
    }
    // fence
    if v > 0.4 && v < 0.58 && u < 0.72 {
        let post = ((u + 0.4) / 0.06).fract() < 0.18;
        let rail = (v > 0.44 && v < 0.46) || (v > 0.52 && v < 0.54);
        if post || rail {
            return (rgb(0xE8DCC0), false);
        }
    }
    let horizon = 0.5;
    if v < horizon {
        if cloud(u, v, c) {
            return (mix(rgb(0x2C3E50), rgb(0xFFFFFF), c.day), false);
        }
        return (sky(v, c, horizon), false);
    }
    // path
    let path_half = 0.04 + (v - horizon) * 0.35;
    if (u - 0.55).abs() < path_half {
        return (rgb(0xBCAAA4), false);
    }
    let stripe = if ((v * 40.0).floor() as i32) % 2 == 0 { 1.0 } else { 0.92 };
    let g = rgb(0x66BB6A);
    ([g[0] * stripe, g[1] * stripe, g[2] * stripe], false)
}

fn scene_street(u: f32, v: f32, c: &Ctx) -> ([f32; 3], bool) {
    for a in &c.actors {
        match a.kind {
            EvKind::Human => {
                if let Some(col) = person(u, v, a.x, 0.63, 0.24, a.phase, a.tint) {
                    return (col, false);
                }
            }
            EvKind::Motion => {
                if let Some((col, e)) = car(u, v, a.x, 0.8, a.tint, a.facing_left, c.night) {
                    return (col, e);
                }
            }
        }
    }
    // lamp posts
    for i in 0..4 {
        let lx = -0.2 + i as f32 * 0.5;
        if in_rect(u, v, lx - 0.006, 0.25, lx + 0.006, 0.6) {
            return (rgb(0x37474F), false);
        }
        if in_circle(u, v, lx, 0.25, 0.018) {
            return if c.night { (rgb(0xFFE9A8), true) } else { (rgb(0xB0BEC5), false) };
        }
    }
    if v >= 0.6 {
        if v < 0.66 {
            return (rgb(0xB0B0B0), false);
        }
        if v > 0.79 && v < 0.81 && ((u * 8.0).fract() < 0.5) {
            return (rgb(0xF5F5F5), false);
        }
        return (rgb(0x4A4A4A), false);
    }
    // buildings
    let bin = ((u + 1.0) / 0.13).floor() as i64;
    let hb = h2(c.seed, 500 + bin as u64);
    let top = 0.12 + (hb % 100) as f32 * 0.0025;
    if v > top {
        let lu = ((u + 1.0) / 0.13).fract();
        let lv = v - top;
        let win = lu > 0.15 && lu < 0.85 && (lu * 4.0).fract() < 0.6 && (lv / 0.06).fract() < 0.55 && lv > 0.03;
        if win {
            let wid = h2(hb, ((lu * 4.0).floor() as u64) * 31 + (lv / 0.06).floor() as u64);
            if c.night && wid % 3 == 0 {
                return (rgb(0xFFD54F), true);
            }
            return (rgb(0x90A4AE), false);
        }
        const WALLS: [u32; 4] = [0xD7CCC8, 0xBCAAA4, 0xCFD8DC, 0xFFE0B2];
        return (rgb(WALLS[(hb % 4) as usize]), false);
    }
    if cloud(u, v, c) {
        return (mix(rgb(0x2C3E50), rgb(0xFFFFFF), c.day), false);
    }
    (sky(v, c, 0.6), false)
}

fn scene_room(u: f32, v: f32, c: &Ctx) -> ([f32; 3], bool) {
    for a in &c.actors {
        match a.kind {
            EvKind::Human => {
                if let Some(col) = person(u, v, a.x, 0.92, 0.5, a.phase, a.tint) {
                    return (col, false);
                }
            }
            EvKind::Motion => {
                if let Some(col) = pet(u, v, a.x, 0.9, a.facing_left) {
                    return (col, false);
                }
            }
        }
    }
    // lamp
    if in_circle(u, v, 0.92, 0.36, 0.05) && v < 0.36 {
        return if c.night { (rgb(0xFFE7A1), true) } else { (rgb(0xF5F5DC), false) };
    }
    if in_rect(u, v, 0.915, 0.36, 0.925, 0.7) {
        return (rgb(0x5D4037), false);
    }
    // sofa
    if in_rect(u, v, 0.45, 0.52, 0.85, 0.72) || in_rect(u, v, 0.43, 0.6, 0.87, 0.74) {
        let cushion = in_rect(u, v, 0.46, 0.53, 0.84, 0.6) && ((u - 0.46) / 0.127).fract() < 0.95;
        return (if cushion { rgb(0x5C6BC0) } else { rgb(0x3F51B5) }, false);
    }
    // window
    if in_rect(u, v, 0.1, 0.12, 0.35, 0.45) {
        if (u - 0.225).abs() < 0.004 || (v - 0.285).abs() < 0.006 {
            return (rgb(0xFFFFFF), false);
        }
        return (sky(v - 0.12, c, 0.33), c.day > 0.5);
    }
    if in_rect(u, v, 0.09, 0.11, 0.36, 0.46) {
        return (rgb(0xFFFFFF), false);
    }
    // door
    if in_rect(u, v, 1.05, 0.2, 1.22, 0.78) {
        return (rgb(0x8D6E63), false);
    }
    // plant
    if in_circle(u, v, 0.03, 0.6, 0.07) {
        return (rgb(0x388E3C), false);
    }
    if in_rect(u, v, 0.0, 0.62, 0.06, 0.75) {
        return (rgb(0xD84315), false);
    }
    if v > 0.74 {
        if in_ellipse(u, v, 0.62, 0.86, 0.3, 0.07) {
            return (rgb(0xB71C1C), false);
        }
        let plank = if ((u * 12.0 + (v * 20.0).floor() * 0.5).floor() as i32) % 2 == 0 { 1.0 } else { 0.93 };
        let w = rgb(0xC69C6D);
        return ([w[0] * plank, w[1] * plank, w[2] * plank], false);
    }
    (rgb(0xEFE6DA), false)
}

/// Local hour (0..24, with fraction) of a timestamp.
pub fn local_hour(t: f64) -> f32 {
    match Local.timestamp_opt(t.floor() as i64, 0).single() {
        Some(d) => d.hour() as f32 + d.minute() as f32 / 60.0,
        None => 12.0,
    }
}

pub fn daylight(t: f64) -> f32 {
    let hf = local_hour(t);
    smoothstep(5.5, 7.0, hf) * (1.0 - smoothstep(18.0, 19.5, hf))
}

/// Draw one picture of the simulated camera.
pub fn render(seed: u64, scene: u8, t: f64, w: usize, h: usize, look: &Look, view: &View) -> ColorImage {
    let day = daylight(t);
    let ir = look.night_picture(t);
    let c = Ctx {
        seed,
        t: (t % 100_000.0) as f32,
        day,
        night: day < 0.5,
        actors: actors(seed, t),
    };
    let indoor = scene == 2;
    let ambient = if indoor { 0.55 + 0.45 * day } else { 0.22 + 0.78 * day };
    let frame = (t * 10.0).floor() as u64;
    let grain = if ir { 0.06 } else if day < 0.5 { 0.04 } else { 0.015 };
    let zoom = view.zoom.clamp(1.0, ZOOM_MAX);
    let mut bytes = vec![0u8; w * h * 3];
    for y in 0..h {
        let mut sv = (y as f32 + 0.5) / h as f32;
        if look.flip_ud {
            sv = 1.0 - sv;
        }
        for x in 0..w {
            let mut su = (x as f32 + 0.5) / w as f32;
            if look.flip_lr {
                su = 1.0 - su;
            }
            let u = 0.5 + (su - 0.5) / zoom + view.pan;
            let v = 0.5 + (sv - 0.5) / zoom + view.tilt;
            let (col, emissive) = match scene {
                1 => scene_street(u, v, &c),
                2 => scene_room(u, v, &c),
                _ => scene_garden(u, v, &c),
            };
            let light = if emissive { 1.0 } else if ir { 0.75 } else { ambient };
            let mut r = col[0] * light;
            let mut g = col[1] * light;
            let mut b = col[2] * light;
            if ir {
                let l = 0.3 * col[0] + 0.59 * col[1] + 0.11 * col[2];
                let l = (l * if emissive { 1.2 } else { 0.85 }).min(1.0);
                r = l;
                g = l;
                b = l;
            }
            let n = (h2(frame, (y * w + x) as u64) & 1023) as f32 / 1023.0 - 0.5;
            let du = su - 0.5;
            let dv = sv - 0.5;
            let vig = 1.0 - (du * du + dv * dv) * 0.5;
            let k = vig;
            let i = (y * w + x) * 3;
            bytes[i] = ((r * k + n * grain).clamp(0.0, 1.0) * 255.0) as u8;
            bytes[i + 1] = ((g * k + n * grain).clamp(0.0, 1.0) * 255.0) as u8;
            bytes[i + 2] = ((b * k + n * grain).clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    ColorImage::from_rgb([w, h], &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_stable() {
        let a = raw_events(7, 1_700_000_000, 1_700_086_400);
        let b = raw_events(7, 1_700_000_000, 1_700_086_400);
        assert_eq!(a.len(), b.len());
        assert!(a.len() > 10 && a.len() < 60);
    }

    #[test]
    fn sensitivity_orders_counts() {
        let mk = |s| EventFilter { motion: true, human: true, sensitivity: s, interval: 0 };
        let lo = alarm_events(9, &mk(0), 1_700_000_000, 1_700_600_000).len();
        let hi = alarm_events(9, &mk(2), 1_700_000_000, 1_700_600_000).len();
        assert!(lo < hi);
    }
}
