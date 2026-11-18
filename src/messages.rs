//! Message centre: alarm messages and system messages.

use crate::app::{App, Confirm};
use crate::model::{self, Msg};
use crate::scene::{EvKind, View};
use crate::ui::{self, TEXT, TEXT2, TEXT3, THEME};
use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Stroke};

impl App {
    pub fn messages_page(&mut self, ui: &mut egui::Ui) {
        let mut clear = false;
        let editing = self.msg_ui.editing;
        let mut toggle_edit = false;
        let system = self.msg_ui.system;
        ui::top_bar(ui, "Central de mensagens", false, |ui| {
            if !system {
                if ui::secondary(ui, if editing { "Concluir" } else { "Editar" }).clicked() {
                    toggle_edit = true;
                }
                if ui::secondary(ui, "Limpar não lidas").clicked() {
                    clear = true;
                }
            }
        });
        if toggle_edit {
            self.msg_ui.editing = !editing;
            self.msg_ui.checked.clear();
        }
        if clear {
            self.store.mark_all_read(model::now_i());
            self.dirty = true;
            self.toast("Todas as mensagens foram marcadas como lidas");
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            if ui::chip(ui, !self.msg_ui.system, "Mensagens de alarme").clicked() {
                self.msg_ui.system = false;
            }
            if ui::chip(ui, self.msg_ui.system, "Mensagens do sistema").clicked() {
                self.msg_ui.system = true;
            }
        });
        ui.add_space(6.0);
        if self.msg_ui.system {
            self.system_messages(ui);
        } else {
            self.alarm_messages(ui);
        }
    }

    fn system_messages(&mut self, ui: &mut egui::Ui) {
        let items: Vec<model::SysMsg> = self.store.syslog.iter().rev().cloned().collect();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            if items.is_empty() {
                ui.vertical_centered(|ui| ui.label(egui::RichText::new("Nenhuma mensagem").color(TEXT3)));
            }
            for m in items {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui::card_frame().inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                        ui.set_width((ui.available_width() - 32.0).max(200.0));
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&m.text).color(TEXT));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(format!("{} {}", model::fmt_date(m.ts), model::fmt_time(m.ts))).small().color(TEXT3));
                            });
                        });
                    });
                });
            }
        });
    }

    fn alarm_messages(&mut self, ui: &mut egui::Ui) {
        let all = self.messages();
        let now = model::now_i();
        // filters
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let dev_name = self.msg_ui.dev.as_ref().and_then(|id| self.store.device(id)).map(|d| d.name.clone()).unwrap_or_else(|| "Todos os dispositivos".into());
            egui::ComboBox::from_id_salt("msgdev").selected_text(dev_name).width(190.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.msg_ui.dev, None, "Todos os dispositivos");
                for d in &self.store.devices {
                    if d.is_demo() {
                        ui.selectable_value(&mut self.msg_ui.dev, Some(d.id.clone()), &d.name);
                    }
                }
            });
            let today = model::day_start(now);
            let day_txt = match self.msg_ui.day {
                None => "Todos os dias".to_string(),
                Some(d) if d == today => "Hoje".to_string(),
                Some(d) => model::fmt_date(d),
            };
            egui::ComboBox::from_id_salt("msgday").selected_text(day_txt).width(140.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.msg_ui.day, None, "Todos os dias");
                for i in 0..model::MESSAGE_DAYS {
                    let d = model::day_start(today - i * 86400 + 3600);
                    let label = if i == 0 { "Hoje".to_string() } else { model::fmt_date(d) };
                    ui.selectable_value(&mut self.msg_ui.day, Some(d), label);
                }
            });
            let kinds = ["Todos os tipos", "Detecção de corpo humano", "Detecção de movimento"];
            egui::ComboBox::from_id_salt("msgkind").selected_text(kinds[self.msg_ui.kind as usize]).width(200.0).show_ui(ui, |ui| {
                for (i, k) in kinds.iter().enumerate() {
                    ui.selectable_value(&mut self.msg_ui.kind, i as u8, *k);
                }
            });
        });
        let list: Vec<Msg> = all
            .into_iter()
            .filter(|m| self.msg_ui.dev.as_ref().map(|d| *d == m.dev_id).unwrap_or(true))
            .filter(|m| self.msg_ui.day.map(|d| m.ts >= d && m.ts < d + 86400).unwrap_or(true))
            .filter(|m| match self.msg_ui.kind {
                1 => m.kind == EvKind::Human,
                2 => m.kind == EvKind::Motion,
                _ => true,
            })
            .collect();
        ui.add_space(6.0);
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let detail_w = (full.width() * 0.42).clamp(340.0, 560.0);
        let left = Rect::from_min_max(full.min + egui::vec2(16.0, 0.0), egui::pos2(full.right() - detail_w - 16.0, full.bottom() - 10.0));
        let right = Rect::from_min_max(egui::pos2(full.right() - detail_w - 4.0, full.top()), egui::pos2(full.right() - 16.0, full.bottom() - 10.0));
        if self.msg_ui.selected.is_none() {
            self.msg_ui.selected = list.first().map(|m| m.key.clone());
        }

        let mut lc = ui.new_child(egui::UiBuilder::new().max_rect(left));
        if self.msg_ui.editing {
            lc.horizontal(|ui| {
                let all_checked = !list.is_empty() && list.iter().all(|m| self.msg_ui.checked.contains(&m.key));
                if ui.button(if all_checked { "Desmarcar todos" } else { "Selecionar todos" }).clicked() {
                    if all_checked {
                        self.msg_ui.checked.clear();
                    } else {
                        self.msg_ui.checked = list.iter().map(|m| m.key.clone()).collect();
                    }
                }
                let n = self.msg_ui.checked.len();
                if ui.add_enabled(n > 0, egui::Button::new(egui::RichText::new(format!("Excluir ({n})")).color(if n > 0 { ui::RED } else { TEXT3 }))).clicked() {
                    self.confirm = Some(Confirm::DeleteMessages(self.msg_ui.checked.iter().cloned().collect()));
                }
            });
        }
        if list.is_empty() {
            lc.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(egui::RichText::new("Nenhuma mensagem").size(16.0).color(TEXT2));
                ui.label(egui::RichText::new("As mensagens aparecem quando a detecção de movimento está ativada.").small().color(TEXT3));
            });
        } else {
            lc.label(egui::RichText::new(format!("{} mensagens", list.len())).small().color(TEXT3));
            let mut open: Option<Msg> = None;
            egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(&mut lc, 86.0, list.len(), |ui, range| {
                for m in &list[range] {
                    if self.message_row(ui, m) {
                        open = Some(m.clone());
                    }
                }
            });
            if let Some(m) = open {
                if self.msg_ui.editing {
                    if !self.msg_ui.checked.remove(&m.key) {
                        self.msg_ui.checked.insert(m.key.clone());
                    }
                } else {
                    self.msg_ui.selected = Some(m.key.clone());
                    if !m.read {
                        self.store.read.insert(m.key.clone());
                        self.dirty = true;
                    }
                }
            }
        }

        let sel = self.msg_ui.selected.clone().and_then(|k| list.iter().find(|m| m.key == k).cloned());
        let mut rc = ui.new_child(egui::UiBuilder::new().max_rect(right));
        ui::card_frame().show(&mut rc, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(right.height() - 30.0);
            match sel {
                None => {
                    ui.label(egui::RichText::new("Selecione uma mensagem").color(TEXT3));
                }
                Some(m) => self.message_detail(ui, &m),
            }
        });
    }

    fn message_row(&mut self, ui: &mut egui::Ui, m: &Msg) -> bool {
        let sel = self.msg_ui.selected.as_deref() == Some(m.key.as_str());
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 80.0), Sense::click());
        let p = ui.painter().clone();
        p.rect_filled(rect, CornerRadius::same(10), if sel && !self.msg_ui.editing { ui::THEME_SOFT } else if resp.hovered() { Color32::from_rgb(0xFA, 0xFB, 0xFB) } else { Color32::WHITE });
        let mut x = rect.left() + 10.0;
        if self.msg_ui.editing {
            let c = self.msg_ui.checked.contains(&m.key);
            let cb = Rect::from_center_size(egui::pos2(x + 9.0, rect.center().y), egui::vec2(18.0, 18.0));
            p.rect_filled(cb, CornerRadius::same(9), if c { THEME } else { Color32::WHITE });
            p.rect_stroke(cb, CornerRadius::same(9), Stroke::new(1.0_f32, if c { THEME } else { TEXT3 }), egui::StrokeKind::Inside);
            x += 28.0;
        }
        let thumb = Rect::from_min_size(egui::pos2(x, rect.top() + 8.0), egui::vec2(114.0, 64.0));
        if let Some(d) = self.store.device(&m.dev_id).cloned() {
            let tex = self.feeds.get(ui.ctx(), &format!("msg:{}", m.key), d.seed, d.scene, m.ts as f64 + 4.0, [192, 108], d.look(), View::default(), 0.2, true);
            ui::paint_image(&p, tex, thumb, 6);
        }
        let tx = thumb.right() + 14.0;
        if !m.read {
            p.circle_filled(egui::pos2(tx + 4.0, rect.top() + 22.0), 4.0, ui::RED);
        }
        let ox = if m.read { 0.0 } else { 14.0 };
        p.text(egui::pos2(tx + ox, rect.top() + 22.0), egui::Align2::LEFT_CENTER, m.title(), egui::FontId::proportional(15.0), TEXT);
        p.text(egui::pos2(tx, rect.top() + 46.0), egui::Align2::LEFT_CENTER, &m.dev_name, egui::FontId::proportional(13.0), TEXT2);
        p.text(egui::pos2(tx, rect.top() + 66.0), egui::Align2::LEFT_CENTER, format!("{}  {}", model::fmt_date(m.ts), model::fmt_time(m.ts)), egui::FontId::proportional(12.5), TEXT3);
        ui.add_space(6.0);
        resp.clicked()
    }

    fn message_detail(&mut self, ui: &mut egui::Ui, m: &Msg) {
        let Some(d) = self.store.device(&m.dev_id).cloned() else { return };
        ui.label(egui::RichText::new(m.title()).size(17.0).strong().color(TEXT));
        ui.label(egui::RichText::new(format!("{} · {} {}", d.name, model::fmt_date(m.ts), model::fmt_time(m.ts))).color(TEXT2));
        ui.add_space(6.0);
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, w * 9.0 / 16.0), Sense::hover());
        let tex = self.feeds.get(ui.ctx(), &format!("msgbig:{}", m.key), d.seed, d.scene, m.ts as f64 + 4.0, [640, 360], d.look(), View::default(), 0.2, true);
        ui::paint_image(ui.painter(), tex, r, 8);
        ui::osd_text(ui.painter(), r.left_top() + egui::vec2(8.0, 6.0), egui::Align2::LEFT_TOP, &model::fmt_datetime(m.ts + 4), 12.0);
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui::primary(ui, "▶ Reproduzir gravação").clicked() {
                self.open_playback(&d.id, m.ts as f64 - 3.0);
            }
            if ui::secondary(ui, "Salvar imagem").clicked() {
                self.snapshot(&d.id, m.ts as f64 + 4.0, View::default());
            }
            if ui.add(egui::Button::new(egui::RichText::new("Excluir").color(ui::RED)).frame(false)).clicked() {
                self.confirm = Some(Confirm::DeleteMessages(vec![m.key.clone()]));
            }
        });
        let segs = d.segments(model::day_start(m.ts), model::now_i());
        if !segs.iter().any(|s| s.start <= m.ts + 4 && s.end > m.ts) {
            ui.label(egui::RichText::new("Não há gravação deste momento no armazenamento (verifique as configurações de gravação).").small().color(TEXT3));
        }
    }
}
