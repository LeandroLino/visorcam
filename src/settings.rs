//! Device settings (DevSettingActivity and its sub-pages).

use crate::app::{App, Confirm, Page};
use crate::model::{self, Device, FullAction, RecordMode, VIDEO_LEN_MAX, VIDEO_LEN_MIN};
use crate::net::NetState;
use crate::scene::SCENE_NAMES;
use crate::ui::{self, TEXT, TEXT2, TEXT3, THEME};
use eframe::egui::{self, Color32, CornerRadius, Sense};

const SECTIONS: [&str; 6] = [
    "Configuração básica",
    "Configurações de alarme",
    "Configurações de gravação",
    "Gerenciamento de armazenamento",
    "Gerenciamento de senha",
    "Sobre o disp.",
];

impl App {
    pub fn settings_page(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(mut d) = self.store.device(id).cloned() else {
            self.page = Page::Main;
            return;
        };
        let mut live = false;
        let back = ui::top_bar(ui, &format!("Conf. · {}", d.name), true, |ui| {
            if ui::primary(ui, "Streaming em tempo real").clicked() {
                live = true;
            }
        });
        if back {
            self.page = Page::Main;
            return;
        }
        if live {
            self.open_live(std::slice::from_ref(&d.id));
            return;
        }
        let before = serde_json::to_string(&d).unwrap_or_default();
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let menu = egui::Rect::from_min_size(full.min + egui::vec2(16.0, 16.0), egui::vec2(285.0, full.height() - 32.0));
        let content = egui::Rect::from_min_max(egui::pos2(menu.right() + 16.0, full.top() + 16.0), egui::pos2(full.right() - 16.0, full.bottom() - 16.0));

        let mut mc = ui.new_child(egui::UiBuilder::new().max_rect(menu));
        ui::card_frame().inner_margin(egui::Margin::same(8)).show(&mut mc, |ui| {
            ui.set_width(ui.available_width());
            for (i, s) in SECTIONS.iter().enumerate() {
                let sel = self.settings_section == i as u8;
                let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), Sense::click());
                if sel {
                    ui.painter().rect_filled(r, CornerRadius::same(8), ui::THEME_SOFT);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(8), Color32::from_rgb(0xF6, 0xF7, 0xF8));
                }
                ui.painter().text(r.left_center() + egui::vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, *s, egui::FontId::proportional(14.5), if sel { ui::THEME_DARK } else { TEXT });
                ui.painter().text(r.right_center() - egui::vec2(10.0, 0.0), egui::Align2::RIGHT_CENTER, "›", egui::FontId::proportional(16.0), TEXT3);
                if resp.clicked() {
                    self.settings_section = i as u8;
                }
            }
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                if ui::danger(ui, "Excluir dispositivo").clicked() {
                    self.confirm = Some(Confirm::DeleteDevice(d.id.clone()));
                }
            });
        });

        let mut cc = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut cc, |ui| {
            ui::card_frame().show(ui, |ui| {
                ui.set_width(ui.available_width().min(760.0));
                ui.label(egui::RichText::new(SECTIONS[self.settings_section as usize]).size(18.0).strong().color(TEXT));
                if !d.is_demo() && self.settings_section < 4 {
                    ui.label(egui::RichText::new("Para dispositivos adicionados por IP, estas opções ficam salvas neste computador; elas ainda não são enviadas ao dispositivo.").small().color(ui::ORANGE));
                }
                ui.add_space(6.0);
                match self.settings_section {
                    0 => self.basic(ui, &mut d),
                    1 => alarm(ui, &mut d),
                    2 => record(ui, &mut d),
                    3 => self.storage(ui, &mut d),
                    4 => self.password(ui, &mut d),
                    _ => self.about(ui, &d),
                }
            });
        });

        let after = serde_json::to_string(&d).unwrap_or_default();
        if after != before {
            if let Some(slot) = self.store.device_mut(id) {
                let reboot = slot.reboot_until;
                *slot = d;
                slot.reboot_until = reboot;
            }
            self.dirty = true;
        }
    }

    fn basic(&mut self, ui: &mut egui::Ui, d: &mut Device) {
        ui::row(ui, "Nome do dispositivo", None, |ui| {
            let changed = self.rename_buf.trim() != d.name;
            if ui.add_enabled(changed, egui::Button::new("Salvar")).clicked() {
                let n = self.rename_buf.trim().to_string();
                if n.is_empty() {
                    self.toast("Digitar nome do dispositivo");
                } else if let Some(l) = model::name_too_long(&n) {
                    self.toast(format!("Digitar nome do dispositivo (1 e {l} caracteres)"));
                } else {
                    self.store.log(&format!("Dispositivo renomeado: {} → {}", d.name, n));
                    d.name = n;
                    self.toast("Salvo");
                }
            }
            ui.add(egui::TextEdit::singleline(&mut self.rename_buf).desired_width(220.0));
        });
        ui::row(ui, "Girar a imagem para esquerda-direita", None, |ui| {
            ui::toggle(ui, &mut d.settings.flip_lr);
        });
        ui::row(ui, "Girar a imagem para cima-baixo", None, |ui| {
            ui::toggle(ui, &mut d.settings.flip_ud);
        });
        ui::row(ui, "Ativar indicador de status", None, |ui| {
            ui::toggle(ui, &mut d.settings.status_light);
        });
        ui::row(ui, "Tom de alerta", None, |ui| {
            ui::toggle(ui, &mut d.settings.alert_tone);
        });
        ui::row(ui, "Volume do alto-falante", None, |ui| {
            ui.add(egui::Slider::new(&mut d.settings.speaker_vol, 0..=100));
        });
        ui::row(ui, "Volume MIC", None, |ui| {
            ui.add(egui::Slider::new(&mut d.settings.mic_vol, 0..=100));
        });
        // DevDayNightModeActivity: SwitchMode 0 auto / 1 day / 2 night / 3 timed
        let tips = [
            "Troca automática do modo diurno/noturno de acordo com o ambiente.",
            "Sempre em cores.",
            "Sempre em preto e branco (visão noturna).",
            "Troca no horário escolhido.",
        ];
        let mode = d.settings.night_mode.min(3) as usize;
        ui::row(ui, "Mudança dia/noite", Some(tips[mode]), |ui| {
            egui::ComboBox::from_id_salt("night").selected_text(model::DAY_NIGHT_MODES[mode]).show_ui(ui, |ui| {
                for (i, n) in model::DAY_NIGHT_MODES.iter().enumerate() {
                    ui.selectable_value(&mut d.settings.night_mode, i as u8, *n);
                }
            });
        });
        if mode == 0 {
            ui::row(ui, &format!("Sensibilidade ({})", d.settings.dn_sensitivity.min(10)), Some("Sensibilidade de troca dia/noite (0 a 10)"), |ui| {
                ui.add(egui::Slider::new(&mut d.settings.dn_sensitivity, 0..=10).show_value(false));
            });
        } else if mode == 3 {
            ui::row(ui, "Horário do modo noturno", None, |ui| {
                ui.add(egui::DragValue::new(&mut d.settings.dn_night_to).range(0..=23).suffix(":00").prefix("até "));
                ui.add(egui::DragValue::new(&mut d.settings.dn_night_from).range(0..=23).suffix(":00").prefix("das "));
            });
        }
        ui::row(ui, "Idioma", None, |ui| {
            egui::ComboBox::from_id_salt("lang").selected_text(d.settings.language.clone()).show_ui(ui, |ui| {
                for l in ["Português", "English", "Español", "Français", "Deutsch", "Italiano"] {
                    ui.selectable_value(&mut d.settings.language, l.to_string(), l);
                }
            });
        });
        if d.is_demo() {
            ui::row(ui, "Cena da demonstração", None, |ui| {
                egui::ComboBox::from_id_salt("scene").selected_text(SCENE_NAMES[d.scene.min(2) as usize]).show_ui(ui, |ui| {
                    for (i, n) in SCENE_NAMES.iter().enumerate() {
                        ui.selectable_value(&mut d.scene, i as u8, *n);
                    }
                });
            });
        }
    }

    fn storage(&mut self, ui: &mut egui::Ui, d: &mut Device) {
        if !d.is_demo() {
            ui.label(egui::RichText::new("O cartão de memória deste dispositivo não pode ser lido nesta versão.").color(TEXT2));
            return;
        }
        // DevStorageSettingActivity.U1: total = sum of partitions, video partition (DirverType
        // 0/5), free = sum of RemainSpace; sizes in MB shown with E.b(size, 2).
        let now = model::now_i();
        let total_mb = model::CARD_MB;
        let used_mb = d.used_gb(now) * 1024.0;
        let free_mb = (total_mb - used_mb).max(0.0);
        ui::row(ui, "Capacidade de armazenamento:", None, |ui| {
            ui.label(egui::RichText::new(model::fmt_size_mb(total_mb)).color(TEXT2));
        });
        ui::row(ui, "Partição de gravação:", None, |ui| {
            ui.label(egui::RichText::new(model::fmt_size_mb(total_mb)).color(TEXT2));
        });
        ui::row(ui, "Espaço livre:", None, |ui| {
            ui.label(egui::RichText::new(model::fmt_size_mb(free_mb)).color(TEXT2));
        });
        // free share, min(remain * 100 / total, 100) with "#0.0"
        let free_pct = (free_mb * 100.0 / total_mb).min(100.0);
        ui.add(
            egui::ProgressBar::new(1.0 - free_pct as f32 / 100.0)
                .fill(if free_pct < 5.0 { ui::ORANGE } else { THEME })
                .text(format!("Espaço livre {:.1}%", free_pct)),
        );
        ui.add_space(4.0);
        ui::row(ui, "Armazenamento completo:", Some("O que fazer quando o cartão encher"), |ui| {
            ui.radio_value(&mut d.settings.full_action, FullAction::Stop, "Parar gravação");
            ui.radio_value(&mut d.settings.full_action, FullAction::Cyclic, "Gravação cíclica");
        });
        let (a, b) = d.record_window(now);
        if b > a {
            ui.label(egui::RichText::new(format!("Gravações disponíveis de {} {} até {} {}.", model::fmt_date(a), model::fmt_time(a), model::fmt_date(b), model::fmt_time(b))).small().color(TEXT3));
        } else {
            ui.label(egui::RichText::new("Não há gravações no cartão.").small().color(TEXT3));
        }
        ui.add_space(8.0);
        if ui::danger(ui, "Formato").clicked() {
            self.confirm = Some(Confirm::Format(d.id.clone()));
        }
    }

    fn password(&mut self, ui: &mut egui::Ui, d: &mut Device) {
        ui::row(ui, "Nome de login do dispositivo", None, |ui| {
            ui.label(egui::RichText::new(&d.user).color(TEXT2));
        });
        egui::Grid::new("pwd").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
            ui.label("Senha atual");
            ui.add(egui::TextEdit::singleline(&mut self.pwd.0).password(true).desired_width(240.0));
            ui.end_row();
            ui.label("Nova senha");
            ui.add(egui::TextEdit::singleline(&mut self.pwd.1).password(true).desired_width(240.0));
            ui.end_row();
            ui.label("Confirmar senha");
            ui.add(egui::TextEdit::singleline(&mut self.pwd.2).password(true).desired_width(240.0));
            ui.end_row();
        });
        ui.add_space(6.0);
        if ui::primary(ui, "Modificar senha").clicked() {
            if self.pwd.0 != d.password {
                self.toast("A senha atual está incorreta");
            } else if self.pwd.1 != self.pwd.2 {
                self.toast("As senhas não coincidem");
            } else if self.pwd.1.chars().count() > 64 {
                self.toast("A senha é muito longa");
            } else {
                d.password = self.pwd.1.clone();
                self.pwd = Default::default();
                self.store.log(&format!("Senha modificada: {}", d.name));
                self.toast("Senha modificada");
                if !d.is_demo() {
                    // like FunSDK.DevSetLocalPwd: the stored login is used from the next connection
                    self.net.check(&d.id, &d.host, d.port, &d.user, &d.password);
                }
            }
        }
        ui.label(egui::RichText::new("A senha é guardada neste computador e usada para entrar no dispositivo.").small().color(TEXT3));
    }

    fn about(&mut self, ui: &mut egui::Ui, d: &Device) {
        let now = model::now_i();
        let info = |ui: &mut egui::Ui, k: &str, v: String| {
            ui::row(ui, k, None, |ui| {
                ui.label(egui::RichText::new(v).color(TEXT2));
            });
        };
        // For cameras added by IP these come from the camera's SystemInfo reply (message 1020).
        let si = if d.is_demo() { None } else { self.net.info(&d.id) };
        let pick = |live: Option<String>, fallback: String| live.filter(|s| !s.is_empty()).unwrap_or(fallback);
        info(ui, "N.º de série", pick(si.as_ref().map(|i| i.serial.clone()), d.id.clone()));
        info(ui, "Versão do software", pick(si.as_ref().map(|i| i.software.clone()), d.firmware.clone()));
        info(ui, "Data da publicação", pick(si.as_ref().map(|i| i.build_time.clone()), if d.is_demo() { "2024-06-14".into() } else { "—".into() }));
        if let Some(i) = &si {
            if !i.model.is_empty() || !i.hardware.is_empty() {
                info(ui, "Modelo", pick(Some(i.model.clone()), i.hardware.clone()));
            }
        }
        info(ui, "Hora do dispositivo", model::fmt_datetime(now));
        info(ui, "Fuso horário", d.settings.timezone.clone());
        let mode = if d.is_demo() { "Wi-Fi (demonstração)".to_string() } else { format!("IP {}:{}", d.host, d.port) };
        info(ui, "Modo de rede", mode);
        if !d.is_demo() {
            let st = match self.net.state(&d.id) {
                NetState::Online(ms) => format!("On-line ({ms} ms)"),
                NetState::Offline => "Off-line".into(),
                NetState::WrongPassword => "Senha incorreta".into(),
                NetState::Refused(c) => format!("Falha no login (código {c})"),
                NetState::NotCamera => "Dispositivo incompatível".into(),
                _ => "Verificando...".into(),
            };
            ui::row(ui, "Estado da conexão", None, |ui| {
                if ui.small_button("Testar").clicked() {
                    self.net.check(&d.id, &d.host, d.port, &d.user, &d.password);
                }
                ui.label(egui::RichText::new(st).color(TEXT2));
            });
        }
        // DeviceRunTime: "0x…" hex value read as minutes (unit estimated); otherwise shown as sent.
        let run = si.as_ref().map(|i| i.run_time.clone()).filter(|s| !s.is_empty()).map(|s| {
            match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).and_then(|h| i64::from_str_radix(h, 16).ok()) {
                Some(min) => model::fmt_dur(min * 60),
                None => s,
            }
        });
        info(ui, "Tempo de operação", run.unwrap_or_else(|| model::fmt_dur(now - d.added_at)));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui::secondary(ui, "Sincronizar hora").clicked() {
                self.toast("Hora do dispositivo sincronizada com o computador");
            }
            if ui::secondary(ui, "Atualizar").clicked() {
                self.toast("O dispositivo já está na versão mais recente");
            }
            if ui::danger(ui, "Reiniciar").clicked() {
                self.confirm = Some(Confirm::Reboot(d.id.clone()));
            }
        });
    }
}

fn alarm(ui: &mut egui::Ui, d: &mut Device) {
    ui::row(ui, "Armar", Some("Envia avisos de alarme a este computador"), |ui| {
        ui::toggle(ui, &mut d.settings.armed);
    });
    ui::row(ui, "Detecção de movimento", None, |ui| {
        ui::toggle(ui, &mut d.settings.motion);
    });
    ui::row(ui, "Detecção de corpo humano", Some("Identifica pessoas nos alarmes"), |ui| {
        ui::toggle(ui, &mut d.settings.human);
    });
    // DevVideoAlarmSetActivity: 0..100 slider; label follows >75 / >25 while dragging, and on
    // release it snaps to 97 / 50 / 3 (device Level 6 / 3 / 1).
    let sid = egui::Id::new(("sens-slider", d.id.clone()));
    let mut pos: u8 = ui.data(|m| m.get_temp(sid)).unwrap_or_else(|| model::slider_from_level(d.settings.sensitivity));
    let names = ["Baixo", "Médio", "Alto"];
    let live_level = model::sensitivity_from_slider(pos).1;
    let hint = format!("Nível enviado ao dispositivo: {}", model::SENSITIVITY_DEVICE_LEVEL[live_level as usize]);
    ui::row(ui, &format!("Sensibilidade: {}", names[live_level as usize]), Some(&hint), |ui| {
        let r = ui.add(egui::Slider::new(&mut pos, 0..=100).show_value(false));
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            let (snap, level) = model::sensitivity_from_slider(pos);
            pos = snap;
            d.settings.sensitivity = level;
        }
    });
    ui.data_mut(|m| m.insert_temp(sid, pos));
    ui::row(ui, "Intervalo (mm:ss)", Some("De 30 s a 30 min"), |ui| {
        let mut s = d.settings.alarm_interval.clamp(model::ALARM_INTERVAL_MIN_S, model::ALARM_INTERVAL_MAX_S);
        ui.label(egui::RichText::new(model::fmt_dur(s)).color(TEXT2));
        if ui.add(egui::Slider::new(&mut s, model::ALARM_INTERVAL_MIN_S..=model::ALARM_INTERVAL_MAX_S).show_value(false)).changed() {
            d.settings.alarm_interval = s;
        }
    });
    ui::row(ui, "Envio da mensagem", None, |ui| {
        ui::toggle(ui, &mut d.settings.push);
    });
    ui::row(ui, "Sino do alarme", Some("O dispositivo toca um som ao detectar"), |ui| {
        ui::toggle(ui, &mut d.settings.alarm_bell);
    });
    let now = model::now_i();
    let n = crate::scene::alarm_events(d.seed, &d.filter(), now - 86400, now).len();
    if d.is_demo() {
        ui.label(egui::RichText::new(format!("Com estas opções: {n} alarmes nas últimas 24 horas.")).small().color(TEXT3));
    }
}

fn record(ui: &mut egui::Ui, d: &mut Device) {
    ui::row(ui, "Botão REC", None, |ui| {
        ui.radio_value(&mut d.settings.record_mode, RecordMode::Off, "Não registrar");
        ui.radio_value(&mut d.settings.record_mode, RecordMode::Alarm, "Alarme");
        ui.radio_value(&mut d.settings.record_mode, RecordMode::Always, "Sempre gravar");
    });
    ui.label(egui::RichText::new(match d.settings.record_mode {
        RecordMode::Always => "regular (sempre a gravar)",
        RecordMode::Alarm => "alarme (gravação de vídeo somente quando o alarme é acionado)",
        RecordMode::Off => "Não registrar",
    }).small().color(TEXT3));
    ui::row(ui, "Qualidade da gravação", None, |ui| {
        ui.radio_value(&mut d.settings.record_hd, false, "SD");
        ui.radio_value(&mut d.settings.record_hd, true, "HD");
    });
    ui::row(ui, "Duração da gravação", Some("Você pode escolher a duração dos arquivos de vídeo."), |ui| {
        ui.add(egui::Slider::new(&mut d.settings.video_length_min, VIDEO_LEN_MIN..=VIDEO_LEN_MAX).suffix(" min"));
    });
}
