//! QR codes: read one from a picture (a photo of the camera's label, a screenshot of a share
//! code) and draw the share code as a QR for another computer or phone.

use crate::model::{self, Device};

/// What a scanned QR code asks for.
pub enum QrAction {
    /// A VisorCam share code ("VC1-..."): add this device.
    Add(Device),
    /// A stream address (rtsp://...): open the RTSP tab with it.
    Rtsp(String),
    /// A camera serial number (the label QR of these cameras, or a "sn=" link): the camera
    /// still needs its IP on the local network.
    Serial(String),
    /// Something this app can't use, with the reason.
    Unknown(String),
}

/// SnAddDevActivity.onActivityResult: the label QR is the bare serial number (checked by
/// g3.f.j1); links carry it in "sn="; other codes are the vendor's encrypted formats
/// (FunSDK.DecDevInfo / DecQRCodeDevInfo), which need its closed library.
pub fn read_text(text: &str) -> QrAction {
    let t = text.trim();
    if t.starts_with("VC1-") {
        return match model::device_from_code(t) {
            Some(d) => QrAction::Add(d),
            None => QrAction::Unknown("O código de compartilhamento está incompleto ou danificado.".into()),
        };
    }
    if t.starts_with("rtsp://") || t.starts_with("rtsps://") {
        return QrAction::Rtsp(t.to_string());
    }
    if is_serial(t) {
        return QrAction::Serial(t.to_lowercase());
    }
    if t.contains("http") {
        if let Some(q) = t.split_once('?').map(|x| x.1) {
            for kv in q.split('&') {
                if let Some(v) = kv.strip_prefix("sn=") {
                    if is_serial(v) {
                        return QrAction::Serial(v.to_lowercase());
                    }
                }
            }
        }
        if t.contains("shareInfo=") || t.contains("sv=") || t.contains("qrLogin") {
            return QrAction::Unknown("Este QR é do app original e vem criptografado pela nuvem do fabricante; ele não pode ser lido aqui. Use o QR da etiqueta da câmera, o IP ou um código VisorCam.".into());
        }
        return QrAction::Unknown(format!("O QR traz um link que não é de câmera: {}", short(t)));
    }
    if t.is_empty() {
        return QrAction::Unknown("O QR code está vazio.".into());
    }
    // GoogleScanQRCodeActivity: a short code that FunSDK.DecDevInfo can't open goes to
    // requestCheckCode -> https://tools.xmeye.net/deviceSuperPassword?text=..&uuid=..&appkey=..
    // (g3.f.m0): the manufacturer's server, with the app's own key, turns it into the camera.
    if t.len() <= 32 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return QrAction::Unknown(format!("Este é um código de compartilhamento do app iCSee (\"{t}\"). Ele não traz a câmera dentro: o iCSee pergunta ao servidor do fabricante qual câmera é, e este app não usa a nuvem do fabricante. Para adicionar aqui: procure a câmera na rede, digite o IP dela (no iCSee: Configurações da câmera › Sobre/Rede), ou compartilhe pelo VisorCam."));
    }
    QrAction::Unknown(format!("O QR code não é de uma câmera conhecida: {}", short(t)))
}

/// g3.f.j1: a serial number is 16 hexadecimal characters or 20 letters and digits.
pub fn is_serial(s: &str) -> bool {
    (s.len() == 16 && s.chars().all(|c| c.is_ascii_hexdigit())) || (s.len() == 20 && s.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn short(t: &str) -> String {
    if t.chars().count() > 60 {
        format!("{}...", t.chars().take(60).collect::<String>())
    } else {
        t.to_string()
    }
}

/// Find and read the first QR code in a picture file.
pub fn decode_file(path: &std::path::Path) -> Result<String, String> {
    let img = image::open(path).map_err(|_| "Não foi possível abrir esta imagem (use PNG ou JPG).".to_string())?;
    decode_image(&img.to_luma8())
}

pub fn decode_image(img: &image::GrayImage) -> Result<String, String> {
    // big phone photos are scaled down first: faster, and still enough for a label QR
    let mut tries = Vec::new();
    let (w, h) = img.dimensions();
    let max = w.max(h);
    if max > 1600 {
        let k = 1600.0 / max as f32;
        tries.push(image::imageops::resize(img, (w as f32 * k) as u32, (h as f32 * k) as u32, image::imageops::FilterType::Triangle));
    }
    tries.push(img.clone());
    for g in &tries {
        let (gw, gh) = (g.width() as usize, g.height() as usize);
        let mut p = rqrr::PreparedImage::prepare_from_greyscale(gw, gh, |x, y| g.get_pixel(x as u32, y as u32).0[0]);
        for grid in p.detect_grids() {
            if let Ok((_, text)) = grid.decode() {
                return Ok(text);
            }
        }
    }
    Err("Nenhum QR code foi encontrado nesta imagem. Tente uma foto mais próxima e nítida.".into())
}

/// The modules of a QR code (true = dark), row by row, and its width.
pub fn encode(text: &str) -> Option<(Vec<bool>, usize)> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let w = code.width();
    Some((code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect(), w))
}

/// A picture of a QR code (with a quiet zone), `scale` pixels per module.
pub fn render(text: &str, scale: u32) -> Option<image::GrayImage> {
    let (mods, w) = encode(text)?;
    let quiet = 4;
    let side = (w as u32 + 2 * quiet) * scale;
    Some(image::GrayImage::from_fn(side, side, |x, y| {
        let (mx, my) = ((x / scale) as i64 - quiet as i64, (y / scale) as i64 - quiet as i64);
        let dark = mx >= 0 && my >= 0 && (mx as usize) < w && (my as usize) < w && mods[my as usize * w + mx as usize];
        image::Luma([if dark { 0 } else { 255 }])
    }))
}

/// Draw a QR code into a square on screen.
pub fn paint(p: &eframe::egui::Painter, rect: eframe::egui::Rect, text: &str) {
    use eframe::egui::{Color32, CornerRadius, Rect, Vec2};
    p.rect_filled(rect, CornerRadius::same(6), Color32::WHITE);
    let Some((mods, w)) = encode(text) else { return };
    let cell = rect.width() / (w as f32 + 8.0);
    let origin = rect.min + Vec2::splat(cell * 4.0);
    for y in 0..w {
        for x in 0..w {
            if mods[y * w + x] {
                let r = Rect::from_min_size(origin + Vec2::new(x as f32 * cell, y as f32 * cell), Vec2::splat(cell + 0.3));
                p.rect_filled(r, CornerRadius::ZERO, Color32::BLACK);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_code_round_trips_through_a_qr_picture() {
        let d = Device::rtsp("Garagem", "rtsp://admin:x@192.168.1.30:554/stream1");
        let code = model::share_code(&d);
        let img = render(&code, 4).unwrap();
        let text = decode_image(&img).unwrap();
        assert_eq!(text, code);
        match read_text(&text) {
            QrAction::Add(got) => assert_eq!(got.url, d.url),
            _ => panic!("not a share code"),
        }
    }

    #[test]
    fn reads_label_and_links() {
        assert!(matches!(read_text("ABCDEF0123456789"), QrAction::Serial(s) if s == "abcdef0123456789"));
        assert!(matches!(read_text("https://x.example/a?sn=abcdef0123456789&t=1"), QrAction::Serial(s) if s == "abcdef0123456789"));
        assert!(matches!(read_text("rtsp://cam/1"), QrAction::Rtsp(s) if s == "rtsp://cam/1"));
        assert!(matches!(read_text("https://d.jfapp.net/x?shareInfo=zz"), QrAction::Unknown(_)));
        assert!(matches!(read_text("abc"), QrAction::Unknown(_)));
        // a label code that isn't a serial (seen on a real camera)
        assert!(matches!(read_text("OHJG1302115NQ"), QrAction::Unknown(m) if m.contains("compartilhamento do app iCSee")));
        assert!(matches!(read_text("ABCDEFGHIJ0123456789"), QrAction::Serial(_)));
        assert!(matches!(read_text("ABCDEFGHIJ012345"), QrAction::Unknown(_)));
    }

    /// Read a picture given in VISORCAM_QR and print what it holds (for checking real QRs).
    #[test]
    fn read_picture_from_env() {
        if let Ok(p) = std::env::var("VISORCAM_QR") {
            println!("QR: {:?}", decode_file(std::path::Path::new(&p)));
        }
    }
}
