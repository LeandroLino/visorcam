//! Photo album (snapshots and recordings) and the "Eu" page.

use crate::app::{AddDlg, AddMode, App, Confirm, Tab};
use crate::model::{self, AlbumItem, MediaKind};
use crate::scene::{Look, View};
use crate::ui::{self, Icon, TEXT, TEXT2, TEXT3, THEME};
use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Stroke};

impl App {
    pub fn album_page(&mut self, ui: &mut egui::Ui) {
        let mut open_folder = false;
        let mut toggle_edit = false;
        let editing = self.album_ui.editing;
        ui::top_bar(ui, "Álbum de fotos", false, |ui| {
            if ui::secondary(ui, "Abrir pasta").clicked() {
                open_folder = true;
            }
            if ui::secondary(ui, if editing { "Concluir" } else { "Editar" }).clicked() {
                toggle_edit = true;
            }
        });
        if open_folder {
            let dir = self.album_path();
            let _ = std::fs::create_dir_all(&dir);
            if !model::open_in_files(&dir, false) {
                self.toast(format!("Pasta: {}", dir.display()));
            }
        }
        if toggle_edit {
            self.album_ui.editing = !editing;
            self.album_ui.checked.clear();
        }
        if self.album_ui.viewer.is_some() {
            self.album_viewer(ui);
            return;
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            for (i, l) in ["Todos", "Imagem", "Vídeo"].iter().enumerate() {
                if ui::chip(ui, self.album_ui.kind == i as u8, l).clicked() {
                    self.album_ui.kind = i as u8;
                }
            }
            ui.add_space(12.0);
            let name = self.album_ui.dev.clone().unwrap_or_else(|| "Todos os dispositivos".into());
            egui::ComboBox::from_id_salt("albdev").selected_text(name).width(190.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.album_ui.dev, None, "Todos os dispositivos");
                let mut names: Vec<String> = self.store.album.iter().map(|a| a.dev_name.clone()).collect();
                names.sort();
                names.dedup();
                for n in names {
                    ui.selectable_value(&mut self.album_ui.dev, Some(n.clone()), n);
                }
            });
            if self.album_ui.editing {
                let n = self.album_ui.checked.len();
                if ui.add_enabled(n > 0, egui::Button::new(egui::RichText::new(format!("Excluir ({n})")).color(if n > 0 { ui::RED } else { TEXT3 }))).clicked() {
                    self.confirm = Some(Confirm::DeleteMedia(self.album_ui.checked.iter().cloned().collect()));
                }
            }
        });
        let mut items: Vec<AlbumItem> = self
            .store
            .album
            .iter()
            .filter(|a| match self.album_ui.kind {
                1 => a.kind == MediaKind::Photo,
                2 => a.kind == MediaKind::Video,
                _ => true,
            })
            .filter(|a| self.album_ui.dev.as_ref().map(|n| *n == a.dev_name).unwrap_or(true))
            .cloned()
            .collect();
        items.sort_by(|a, b| b.ts.partial_cmp(&a.ts).unwrap_or(std::cmp::Ordering::Equal));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if items.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    let (r, _) = ui.allocate_exact_size(egui::vec2(80.0, 80.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 40.0, ui::THEME_SOFT);
                    ui::draw_icon(ui.painter(), Icon::Image, r.shrink(24.0), THEME);
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Sem fotos ou vídeos ainda.").size(16.0).color(TEXT2));
                    ui.label(egui::RichText::new("Use o Instantâneo ou a gravação na tela ao vivo.").small().color(TEXT3));
                });
                return;
            }
            let margin = 16.0;
            let gap = 12.0;
            let avail = ui.available_width() - margin * 2.0;
            let cols = ((avail + gap) / (220.0 + gap)).floor().max(1.0) as usize;
            let w = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
            let mut day = String::new();
            let mut row: Vec<AlbumItem> = Vec::new();
            let mut groups: Vec<(String, Vec<AlbumItem>)> = Vec::new();
            for it in items {
                let d = model::fmt_date(it.ts as i64);
                if d != day {
                    if !row.is_empty() {
                        groups.push((day.clone(), std::mem::take(&mut row)));
                    }
                    day = d;
                }
                row.push(it);
            }
            if !row.is_empty() {
                groups.push((day, row));
            }
            for (day, its) in groups {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(margin);
                    ui.label(egui::RichText::new(day).strong().color(TEXT));
                });
                for chunk in its.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.add_space(margin);
                        ui.spacing_mut().item_spacing.x = gap;
                        for it in chunk {
                            self.album_tile(ui, it, w);
                        }
                    });
                    ui.add_space(gap);
                }
            }
        });
    }

    fn album_tile(&mut self, ui: &mut egui::Ui, it: &AlbumItem, w: f32) {
        let h = w * 9.0 / 16.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h + 26.0), Sense::click());
        let p = ui.painter().clone();
        let img = Rect::from_min_size(rect.min, egui::vec2(w, h));
        match self.images.get(ui.ctx(), &it.path) {
            Some((tex, _)) => ui::paint_image(&p, tex, img, 8),
            None => {
                p.rect_filled(img, CornerRadius::same(8), Color32::from_rgb(0xDD, 0xDF, 0xE3));
                p.text(img.center(), egui::Align2::CENTER_CENTER, "Arquivo não encontrado", egui::FontId::proportional(12.0), TEXT2);
            }
        }
        if it.kind == MediaKind::Video {
            p.circle_filled(img.center(), 20.0, Color32::from_black_alpha(130));
            ui::draw_icon(&p, Icon::Play, Rect::from_center_size(img.center() + egui::vec2(2.0, 0.0), egui::vec2(18.0, 18.0)), Color32::WHITE);
            ui::osd_text(&p, img.right_bottom() - egui::vec2(8.0, 6.0), egui::Align2::RIGHT_BOTTOM, &model::fmt_dur(it.duration as i64), 12.0);
        }
        if resp.hovered() {
            p.rect_stroke(img, CornerRadius::same(8), Stroke::new(2.0_f32, THEME), egui::StrokeKind::Inside);
        }
        if self.album_ui.editing {
            let c = self.album_ui.checked.contains(&it.path);
            let cb = Rect::from_center_size(img.right_top() + egui::vec2(-16.0, 16.0), egui::vec2(20.0, 20.0));
            p.circle_filled(cb.center(), 10.0, if c { THEME } else { Color32::from_white_alpha(220) });
            p.circle_stroke(cb.center(), 10.0, Stroke::new(1.0_f32, if c { THEME } else { TEXT3 }));
        }
        p.text(egui::pos2(rect.left() + 2.0, rect.bottom() - 4.0), egui::Align2::LEFT_BOTTOM, format!("{} · {}", it.dev_name, model::fmt_time(it.ts as i64)), egui::FontId::proportional(12.5), TEXT2);
        if resp.clicked() {
            if self.album_ui.editing {
                if !self.album_ui.checked.remove(&it.path) {
                    self.album_ui.checked.insert(it.path.clone());
                }
            } else {
                self.album_ui.viewer = Some(it.path.clone());
                self.album_ui.play_t = 0.0;
                self.album_ui.playing = true;
            }
        }
    }

    fn album_viewer(&mut self, ui: &mut egui::Ui) {
        let path = self.album_ui.viewer.clone().unwrap_or_default();
        let mut items: Vec<AlbumItem> = self.store.album.clone();
        items.sort_by(|a, b| b.ts.partial_cmp(&a.ts).unwrap_or(std::cmp::Ordering::Equal));
        let Some(idx) = items.iter().position(|a| a.path == path) else {
            self.album_ui.viewer = None;
            return;
        };
        let it = items[idx].clone();
        ui.add_space(8.0);
        let mut close = false;
        let mut go: Option<usize> = None;
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            if ui::secondary(ui, "‹ Voltar ao álbum").clicked() {
                close = true;
            }
            ui.label(egui::RichText::new(format!("{} · {} {}", it.dev_name, model::fmt_date(it.ts as i64), model::fmt_time(it.ts as i64))).strong().color(TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);
                if ui.add(egui::Button::new(egui::RichText::new("Excluir").color(ui::RED))).clicked() {
                    self.confirm = Some(Confirm::DeleteMedia(vec![it.path.clone()]));
                }
                if ui.button("Mostrar na pasta").clicked() {
                    let _ = model::open_in_files(std::path::Path::new(&it.path), true);
                }
                if ui.add_enabled(idx + 1 < items.len(), egui::Button::new("Próximo ›")).clicked() {
                    go = Some(idx + 1);
                }
                if ui.add_enabled(idx > 0, egui::Button::new("‹ Anterior")).clicked() {
                    go = Some(idx - 1);
                }
            });
        });
        if close {
            self.album_ui.viewer = None;
            return;
        }
        if let Some(i) = go {
            self.album_ui.viewer = Some(items[i].path.clone());
            self.album_ui.play_t = 0.0;
            self.album_ui.playing = true;
            return;
        }
        let full = ui.available_rect_before_wrap().shrink(16.0);
        ui.allocate_rect(full, Sense::hover());
        let bar_h = if it.kind == MediaKind::Video { 54.0 } else { 0.0 };
        let area = Rect::from_min_max(full.min, egui::pos2(full.right(), full.bottom() - bar_h));
        let p = ui.painter().clone();
        p.rect_filled(full, CornerRadius::same(10), Color32::from_rgb(0x12, 0x14, 0x1A));
        let r = ui::fit_16_9(area.shrink(8.0));
        match it.kind {
            MediaKind::Photo => match self.images.get(ui.ctx(), &it.path) {
                Some((tex, _)) => ui::paint_image(&p, tex, r, 0),
                None => {
                    p.text(r.center(), egui::Align2::CENTER_CENTER, "Arquivo não encontrado", egui::FontId::proportional(15.0), Color32::WHITE);
                }
            },
            MediaKind::Video => {
                let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
                if self.album_ui.playing {
                    self.album_ui.play_t += dt;
                    if self.album_ui.play_t >= it.duration {
                        self.album_ui.play_t = it.duration;
                        self.album_ui.playing = false;
                    }
                }
                let t = it.ts + self.album_ui.play_t;
                let look = Look::simple(it.flip_lr, it.flip_ud, it.night_mode);
                let tex = self.feeds.get(ui.ctx(), &format!("albv:{}", it.path), it.seed, it.scene, t, [640, 360], look, View::default(), 12.0, !self.album_ui.playing);
                p.image(tex, r, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
                ui::osd_text(&p, r.left_top() + egui::vec2(10.0, 8.0), egui::Align2::LEFT_TOP, &model::fmt_datetime(t as i64), 15.0);
                let bar = Rect::from_min_max(egui::pos2(full.left() + 8.0, full.bottom() - bar_h + 4.0), egui::pos2(full.right() - 8.0, full.bottom() - 6.0));
                p.rect_filled(bar, CornerRadius::same(8), ui::VIDEO_BAR);
                let mut c = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(egui::vec2(10.0, 4.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
                let playing = self.album_ui.playing;
                if ui::bar_button(&mut c, if playing { Icon::Pause } else { Icon::Play }, if playing { "Pausar" } else { "Reproduzir" }, false, Color32::WHITE).clicked() {
                    if !playing && self.album_ui.play_t >= it.duration {
                        self.album_ui.play_t = 0.0;
                    }
                    self.album_ui.playing = !playing;
                }
                c.label(egui::RichText::new(format!("{} / {}", model::fmt_dur(self.album_ui.play_t as i64), model::fmt_dur(it.duration as i64))).monospace().color(Color32::WHITE));
                let mut pos = self.album_ui.play_t as f32;
                c.spacing_mut().slider_width = (bar.width() - 220.0).max(80.0);
                if c.add(egui::Slider::new(&mut pos, 0.0..=it.duration.max(0.1) as f32).show_value(false)).changed() {
                    self.album_ui.play_t = pos as f64;
                }
            }
        }
    }

    // ------------------------------------------------------------------ "Eu"

    pub fn me_page(&mut self, ui: &mut egui::Ui) {
        ui::top_bar(ui, "Eu", false, |_| {});
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(14.0);
            let w = (ui.available_width() - 40.0).min(760.0);
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.set_width(w);
                    self.me_content(ui);
                });
            });
        });
    }

    fn me_content(&mut self, ui: &mut egui::Ui) {
        // profile header
        egui::Frame::new()
            .fill(THEME)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 30.0, Color32::WHITE);
                    ui::draw_icon(ui.painter(), Icon::User, r.shrink(17.0), THEME);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Usuário local").size(19.0).strong().color(Color32::WHITE));
                        ui.label(egui::RichText::new("Os dados ficam neste computador. A conta na nuvem não está disponível nesta versão.").color(Color32::from_rgb(0xE0, 0xF7, 0xF3)));
                    });
                });
                ui.add_space(8.0);
                let n_dev = self.store.devices.len();
                let n_alb = self.store.album.len();
                let n_msg = self.unread_for(None);
                ui.horizontal(|ui| {
                    for (v, l) in [(n_dev, "Dispositivos"), (n_alb, "Fotos e vídeos"), (n_msg, "Não lidas")] {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(v.to_string()).size(20.0).strong().color(Color32::WHITE));
                            ui.label(egui::RichText::new(l).small().color(Color32::from_rgb(0xE0, 0xF7, 0xF3)));
                        });
                        ui.add_space(30.0);
                    }
                });
            });
        ui.add_space(12.0);

        ui::card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Notificações").strong().color(TEXT));
            let mut changed = false;
            ui::row(ui, "Notificação push do aplicativo", Some("Quando desativada, os alarmes não aparecem como aviso"), |ui| {
                changed |= ui::toggle(ui, &mut self.store.prefs.push).changed();
            });
            ui::row(ui, "Não perturbe", Some("Os alarmes chegam em silêncio no período escolhido"), |ui| {
                changed |= ui::toggle(ui, &mut self.store.prefs.dnd).changed();
            });
            if self.store.prefs.dnd {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Período: das").color(TEXT2));
                    changed |= ui.add(egui::DragValue::new(&mut self.store.prefs.dnd_from).range(0..=23).suffix(":00")).changed();
                    ui.label(egui::RichText::new("até").color(TEXT2));
                    changed |= ui.add(egui::DragValue::new(&mut self.store.prefs.dnd_to).range(0..=23).suffix(":00")).changed();
                    if self.store.prefs.dnd_active(model::now_i()) {
                        ui.label(egui::RichText::new("(ativo agora)").small().color(ui::ORANGE));
                    }
                });
            }
            if changed {
                self.dirty = true;
            }
        });
        ui.add_space(12.0);

        ui::card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Ferramentas").strong().color(TEXT));
            let mut go_album = false;
            let mut go_scan = false;
            let mut go_sys = false;
            ui::row(ui, "Álbum de fotos", None, |ui| {
                if ui.button("Abrir ›").clicked() {
                    go_album = true;
                }
            });
            ui::row(ui, "Procurar por dispositivo", Some("Procura câmeras na rede local (porta 34567)"), |ui| {
                if ui.button("Procurar ›").clicked() {
                    go_scan = true;
                }
            });
            ui::row(ui, "Mensagens do sistema", None, |ui| {
                if ui.button("Abrir ›").clicked() {
                    go_sys = true;
                }
            });
            let mut changed = false;
            ui::row(ui, "Decodificação por hardware", Some("Usa a placa de vídeo para desenhar as imagens"), |ui| {
                changed |= ui::toggle(ui, &mut self.store.prefs.hard_decode).changed();
            });
            ui::row(ui, "Abrir ao vivo em 4 telas", None, |ui| {
                changed |= ui::toggle(ui, &mut self.store.prefs.split_default).changed();
            });
            if changed {
                self.dirty = true;
            }
            if go_album {
                self.tab = Tab::Album;
            }
            if go_scan {
                self.add = Some(AddDlg::new(AddMode::Lan));
                self.net.start_scan(model::DEFAULT_PORT);
            }
            if go_sys {
                self.tab = Tab::Messages;
                self.msg_ui.system = true;
            }
        });
        ui.add_space(12.0);

        ui::card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Gerenciamento do compartilhamento").strong().color(TEXT));
            egui::CollapsingHeader::new(egui::RichText::new("Como funciona o compartilhamento?").color(ui::THEME_DARK))
                .id_salt("share-help")
                .default_open(self.store.shares.is_empty())
                .show(ui, |ui| {
                    crate::devices::share_help(ui, crate::devices::ShareHelp::All);
                });
            ui.add_space(4.0);
            if self.store.shares.is_empty() {
                ui.label(egui::RichText::new("Nenhum código compartilhado ainda. Use Compartilhar no cartão do dispositivo.").color(TEXT3));
            }
            let mut remove: Option<usize> = None;
            let shares = self.store.shares.clone();
            for (i, s) in shares.iter().enumerate().rev().take(8) {
                ui::row(ui, &s.dev_name, Some(&format!("{} {}", model::fmt_date(s.ts), model::fmt_time(s.ts))), |ui| {
                    if ui.small_button("Remover").clicked() {
                        remove = Some(i);
                    }
                    if ui.small_button("Copiar").clicked() {
                        ui.ctx().copy_text(s.code.clone());
                    }
                });
            }
            if let Some(i) = remove {
                self.store.shares.remove(i);
                self.dirty = true;
            }
        });
        ui.add_space(12.0);

        ui::card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Sobre").strong().color(TEXT));
            ui::row(ui, "Versão", None, |ui| {
                ui.label(egui::RichText::new(env!("CARGO_PKG_VERSION")).color(TEXT2));
            });
            let data = model::data_dir(self.is_check());
            let alb = self.album_path();
            ui::row(ui, "Pasta de dados", Some(&data.display().to_string()), |_| {});
            ui::row(ui, "Pasta do álbum", Some(&alb.display().to_string()), |_| {});
            ui.label(egui::RichText::new("Aplicativo para visualizar câmeras de segurança: dispositivos, ao vivo, reprodução, alarmes e álbum.").small().color(TEXT3));
        });
        ui.add_space(20.0);
    }
}
