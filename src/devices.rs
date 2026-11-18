//! Device list, adding devices, sharing and confirmation dialogs.

use crate::app::{AddDlg, AddMode, App, Confirm};
use crate::model::{self, Device, DEFAULT_PORT, DEFAULT_USER};
use crate::net::NetState;
use crate::scene::{View, SCENE_NAMES};
use crate::ui::{self, Icon, TEXT, TEXT2, TEXT3, THEME};
use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Stroke};

impl App {
    pub fn devices_page(&mut self, ui: &mut egui::Ui) {
        let mut add = false;
        let mut all = false;
        let mut search = std::mem::take(&mut self.search);
        ui::top_bar(ui, "Dispositivo", false, |ui| {
            if ui::primary(ui, "+ Adicionar dispositivo").clicked() {
                add = true;
            }
            if ui::secondary(ui, "Visualizar em 4 telas").clicked() {
                all = true;
            }
            ui.add(egui::TextEdit::singleline(&mut search).hint_text("Pesquisar").desired_width(180.0));
        });
        self.search = search;
        if add {
            self.add = Some(AddDlg::new(AddMode::Ip));
        }
        if all {
            let ids: Vec<String> = self.store.devices.iter().take(4).map(|d| d.id.clone()).collect();
            if ids.is_empty() {
                self.toast("Adicione um dispositivo primeiro");
            } else if ids.len() == 1 {
                self.open_live(&ids);
                self.live.split4 = true;
            } else {
                self.open_live(&ids);
            }
        }

        let q = self.search.trim().to_lowercase();
        let devs: Vec<Device> = self
            .store
            .devices
            .iter()
            .filter(|d| q.is_empty() || d.name.to_lowercase().contains(&q) || d.id.to_lowercase().contains(&q))
            .cloned()
            .collect();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(16.0);
            if devs.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    let (r, _) = ui.allocate_exact_size(egui::vec2(90.0, 90.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 45.0, ui::THEME_SOFT);
                    ui::draw_icon(ui.painter(), Icon::Camera, r.shrink(26.0), THEME);
                    ui.add_space(10.0);
                    if q.is_empty() {
                        ui.label(egui::RichText::new("Nenhum dispositivo").size(17.0).color(TEXT2));
                        ui.add_space(6.0);
                        if ui::primary(ui, "+ Adicionar dispositivo").clicked() {
                            self.add = Some(AddDlg::new(AddMode::Ip));
                        }
                    } else {
                        ui.label(egui::RichText::new(format!("Não foi encontrado nenhum dispositivo relacionado a “{}”.", self.search.trim())).color(TEXT2));
                    }
                });
                return;
            }
            let margin = 20.0;
            let gap = 16.0;
            let avail = ui.available_width() - margin * 2.0;
            let cols = ((avail + gap) / (330.0 + gap)).floor().max(1.0) as usize;
            let w = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
            for chunk in devs.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.add_space(margin);
                    ui.spacing_mut().item_spacing.x = gap;
                    for d in chunk {
                        self.device_card(ui, d, w);
                    }
                });
                ui.add_space(gap);
            }
        });
    }

    fn device_card(&mut self, ui: &mut egui::Ui, d: &Device, w: f32) {
        let now = model::now_i();
        let ph = w * 9.0 / 16.0;
        let h = 46.0 + ph + 66.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(rect, CornerRadius::same(12), Color32::WHITE);
        p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0_f32, Color32::from_rgb(0xEC, 0xEC, 0xEC)), egui::StrokeKind::Inside);

        // header: name and state
        let state = if d.is_demo() {
            if d.rebooting(now) {
                NetState::Checking
            } else {
                NetState::Online(0)
            }
        } else {
            self.net.state(&d.id)
        };
        let (st_text, st_col) = match state {
            NetState::Online(_) => ("On-line", THEME),
            NetState::Offline => ("Off-line", ui::TEXT3),
            NetState::Checking => (if d.rebooting(now) { "Reiniciando..." } else { "Conectando..." }, ui::ORANGE),
            NetState::Unknown => ("Conectando...", ui::ORANGE),
            // Invalid_Password (pt_BR)
            NetState::WrongPassword => ("Senha incorreta", ui::RED),
            NetState::Refused(_) => ("Falha no login", ui::RED),
            NetState::NotCamera => ("Dispositivo incompatível", ui::RED),
        };
        let hy = rect.top() + 23.0;
        p.circle_filled(egui::pos2(rect.left() + 18.0, hy), 4.5, st_col);
        let name = p.layout_no_wrap(d.name.clone(), egui::FontId::proportional(16.0), TEXT);
        let name_w = name.size().x.min(w - 170.0);
        p.with_clip_rect(Rect::from_min_size(egui::pos2(rect.left() + 30.0, rect.top()), egui::vec2(name_w, 46.0)))
            .galley(egui::pos2(rect.left() + 30.0, hy - name.size().y / 2.0), name, TEXT);
        let tag = if d.is_demo() { "Demonstração" } else { "IP" };
        p.text(egui::pos2(rect.right() - 14.0, hy), egui::Align2::RIGHT_CENTER, format!("{st_text} · {tag}"), egui::FontId::proportional(12.5), st_col);

        // preview
        let pr = Rect::from_min_size(egui::pos2(rect.left() + 10.0, rect.top() + 46.0), egui::vec2(w - 20.0, ph - 10.0));
        let presp = ui.interact(pr, ui.id().with(("prev", &d.id)), Sense::click());
        let online = matches!(state, NetState::Online(_));
        if d.is_demo() && online {
            let tex = self.feeds.get(ui.ctx(), &format!("card:{}", d.id), d.seed, d.scene, model::now_f(), [384, 216], d.look(), View::default(), 3.0, false);
            ui::paint_image(&p, tex, pr, 10);
        } else {
            p.rect_filled(pr, CornerRadius::same(10), Color32::from_rgb(0x2B, 0x30, 0x3B));
        }
        // name pill (adapter_device_list: "Camera01" pill over the preview)
        let pill = p.layout_no_wrap("Camera01".into(), egui::FontId::proportional(11.5), Color32::WHITE);
        let pill_r = Rect::from_min_size(pr.left_top() + egui::vec2(10.0, 10.0), pill.size() + egui::vec2(20.0, 8.0));
        p.rect_filled(pill_r, CornerRadius::same(11), Color32::from_black_alpha(110));
        p.galley(pill_r.min + egui::vec2(10.0, 4.0), pill, Color32::WHITE);
        if d.settings.armed && d.is_demo() {
            let sr = Rect::from_center_size(pr.right_top() + egui::vec2(-20.0, 20.0), egui::vec2(24.0, 24.0));
            p.circle_filled(sr.center(), 13.0, Color32::from_black_alpha(110));
            ui::draw_icon(&p, Icon::Shield, sr.shrink(5.0), Color32::WHITE);
        }
        match state {
            NetState::Offline => {
                p.rect_filled(pr, CornerRadius::same(10), Color32::from_black_alpha(150));
                p.text(pr.center() - egui::vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, "Off-line", egui::FontId::proportional(15.0), Color32::WHITE);
                let hr = Rect::from_center_size(pr.center() + egui::vec2(0.0, 18.0), egui::vec2(84.0, 26.0));
                p.rect_stroke(hr, CornerRadius::same(13), Stroke::new(1.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
                p.text(hr.center(), egui::Align2::CENTER_CENTER, "Ajuda", egui::FontId::proportional(13.0), Color32::WHITE);
                if ui.interact(hr, ui.id().with(("help", &d.id)), Sense::click()).clicked() {
                    self.help = true;
                }
            }
            NetState::Checking | NetState::Unknown => {
                p.text(pr.center(), egui::Align2::CENTER_CENTER, st_text, egui::FontId::proportional(15.0), Color32::WHITE);
            }
            NetState::Online(ms) if !d.is_demo() => {
                // last picture seen in the live view, if any
                let last = self.stream_tex.iter().find(|(k, _)| k.starts_with(&format!("{}|", d.id))).map(|(_, (t, _))| t.id());
                if let Some(tex) = last {
                    ui::paint_image(&p, tex, pr, 10);
                } else {
                    p.text(pr.center() - egui::vec2(0.0, 10.0), egui::Align2::CENTER_CENTER, format!("Conectado ({} ms)", ms), egui::FontId::proportional(14.0), Color32::WHITE);
                    p.text(pr.center() + egui::vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, "Clique para ver ao vivo", egui::FontId::proportional(12.0), Color32::from_gray(190));
                }
            }
            NetState::WrongPassword | NetState::Refused(_) | NetState::NotCamera => {
                p.rect_filled(pr, CornerRadius::same(10), Color32::from_black_alpha(150));
                let detail = match state {
                    NetState::WrongPassword => "Altere a senha em Conf. › Gerenciamento de senha".to_string(),
                    NetState::Refused(code) => format!("O dispositivo respondeu com o código {code}"),
                    _ => format!("A porta {} não usa o protocolo destas câmeras", d.port),
                };
                p.text(pr.center() - egui::vec2(0.0, 10.0), egui::Align2::CENTER_CENTER, st_text, egui::FontId::proportional(15.0), Color32::WHITE);
                p.text(pr.center() + egui::vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, detail, egui::FontId::proportional(12.0), Color32::from_gray(200));
            }
            _ => {
                if presp.hovered() {
                    p.circle_filled(pr.center(), 26.0, Color32::from_black_alpha(120));
                    ui::draw_icon(&p, Icon::Play, Rect::from_center_size(pr.center() + egui::vec2(3.0, 0.0), egui::vec2(24.0, 24.0)), Color32::WHITE);
                }
            }
        }
        if presp.clicked() {
            if !d.is_demo() && !state.is_online() {
                self.net.check(&d.id, &d.host, d.port, &d.user, &d.password);
                self.toast("Verificando a conexão...");
            } else {
                self.open_live(std::slice::from_ref(&d.id));
            }
        }
        let presp = presp.on_hover_text("Abrir streaming em tempo real");
        let _ = presp;

        // function row: Compartilhar / Mensagem / Proteção / Conf.
        let unread = self.unread_for(Some(&d.id));
        let row = Rect::from_min_max(egui::pos2(rect.left() + 6.0, rect.bottom() - 62.0), egui::pos2(rect.right() - 6.0, rect.bottom() - 4.0));
        p.hline(rect.left() + 12.0..=rect.right() - 12.0, row.top() - 2.0, Stroke::new(1.0_f32, Color32::from_rgb(0xF0, 0xF0, 0xF0)));
        let bw = row.width() / 4.0;
        let items = [
            (Icon::Share, "Compartilhar", 0usize),
            (Icon::Bell, "Mensagem", unread),
            (Icon::Shield, if d.settings.armed { "Desarmar" } else { "Armar" }, 0),
            (Icon::Gear, "Conf.", 0),
        ];
        for (i, (icon, label, badge)) in items.into_iter().enumerate() {
            let r = Rect::from_min_size(egui::pos2(row.left() + bw * i as f32, row.top()), egui::vec2(bw, row.height()));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
            let col = if i == 2 && d.settings.armed { THEME } else { TEXT2 };
            if ui::icon_label_button(&mut child, icon, label, r.size(), col, badge).clicked() {
                match i {
                    0 => self.share = Some(d.id.clone()),
                    1 => {
                        self.tab = crate::app::Tab::Messages;
                        self.msg_ui.system = false;
                        self.msg_ui.dev = Some(d.id.clone());
                        self.msg_ui.selected = None;
                    }
                    2 => {
                        if let Some(dm) = self.store.device_mut(&d.id) {
                            dm.settings.armed = !dm.settings.armed;
                            let t = if dm.settings.armed { "Armar" } else { "Desarmar" };
                            let n = dm.name.clone();
                            self.store.log(&format!("{t}: {n}"));
                            self.toast(format!("{t}: {n}"));
                        }
                        self.dirty = true;
                    }
                    _ => self.open_settings(&d.id),
                }
            }
        }
    }

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        if self.add.is_some() {
            self.add_dialog(ctx);
        }
        if let Some(id) = self.share.clone() {
            self.share_dialog(ctx, &id);
        }
        if self.help {
            let mut open = true;
            egui::Window::new("Verificar ajuda")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.set_width(420.0);
                    ui.label("O dispositivo não respondeu. Verifique:");
                    for s in [
                        "1. O dispositivo está ligado e com a luz de status acesa.",
                        "2. O computador e o dispositivo estão na mesma rede.",
                        "3. O IP e a porta estão corretos (porta padrão 34567).",
                        "4. Nenhum firewall está bloqueando a porta.",
                    ] {
                        ui.label(egui::RichText::new(s).color(TEXT2));
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Clique na imagem do cartão para testar a conexão de novo.").small().color(TEXT3));
                });
            self.help = open;
        }
        if self.confirm.is_some() {
            self.confirm_dialog(ctx);
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let (title, text, ok) = match self.confirm.as_ref().unwrap() {
            Confirm::DeleteDevice(_) => ("Excluir dispositivo", "Tem certeza que deseja excluí-lo?", "Excluir"),
            Confirm::Format(_) => ("Formato", "Esta ação excluirá todos os arquivos no cartão de memória. Continuar?", "Formatar"),
            Confirm::Reboot(_) => ("Reiniciar", "Reiniciar o dispositivo agora?", "Reiniciar"),
            Confirm::DeleteMessages(_) => ("Excluir", "Tem certeza que deseja excluir as mensagens selecionadas?", "Excluir"),
            Confirm::DeleteMedia(_) => ("Excluir", "Tem certeza que deseja excluir os arquivos selecionados?", "Excluir"),
        };
        let mut result: Option<bool> = None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(340.0);
                ui.add_space(4.0);
                ui.label(text);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui::secondary(ui, "Cancelar").clicked() {
                        result = Some(false);
                    }
                    if ui::danger(ui, ok).clicked() {
                        result = Some(true);
                    }
                });
            });
        match result {
            Some(true) => {
                let c = self.confirm.take().unwrap();
                match c {
                    Confirm::DeleteDevice(id) => self.delete_device(&id),
                    Confirm::Format(id) => {
                        let now = model::now_i();
                        if let Some(d) = self.store.device_mut(&id) {
                            d.format_ts = now;
                            let n = d.name.clone();
                            self.store.log(&format!("Armazenamento formatado: {n}"));
                        }
                        self.dirty = true;
                        self.toast("Formatado com sucesso");
                    }
                    Confirm::Reboot(id) => {
                        let now = model::now_i();
                        if let Some(d) = self.store.device_mut(&id) {
                            d.reboot_until = now + model::REBOOT_SECONDS;
                            let n = d.name.clone();
                            self.store.log(&format!("Dispositivo reiniciado: {n}"));
                        }
                        self.toast("O dispositivo está reiniciando...");
                    }
                    Confirm::DeleteMessages(keys) => {
                        for k in keys {
                            self.store.deleted.insert(k);
                        }
                        self.msg_ui.checked.clear();
                        self.msg_ui.selected = None;
                        self.msg_ui.editing = false;
                        self.dirty = true;
                    }
                    Confirm::DeleteMedia(paths) => {
                        for path in paths {
                            let _ = std::fs::remove_file(&path);
                            self.images.forget(&path);
                            self.store.album.retain(|a| a.path != path);
                        }
                        self.album_ui.checked.clear();
                        self.album_ui.viewer = None;
                        self.album_ui.editing = false;
                        self.dirty = true;
                    }
                }
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    fn add_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut dlg = self.add.take().unwrap();
        let mut close = false;
        let mut to_add: Option<Device> = None;
        egui::Window::new("Adicionar dispositivo")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(520.0);
                ui.horizontal(|ui| {
                    for (m, label) in [
                        (AddMode::Ip, "Adicionar via IP"),
                        (AddMode::Lan, "Procurar por dispositivo"),
                        (AddMode::Code, "Código"),
                    ] {
                        if ui::chip(ui, dlg.mode == m, label).clicked() {
                            dlg.mode = m;
                            dlg.error = None;
                        }
                    }
                });
                ui.add_space(8.0);
                match dlg.mode {
                    AddMode::Ip => {
                        egui::Grid::new("ipgrid").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                            ui.label("Nome do dispositivo");
                            ui.add(egui::TextEdit::singleline(&mut dlg.name).hint_text("Opcional (usa o IP)").desired_width(300.0));
                            ui.end_row();
                            ui.label("IP");
                            ui.add(egui::TextEdit::singleline(&mut dlg.host).hint_text("192.168.1.10").desired_width(300.0));
                            ui.end_row();
                            ui.label("Porta");
                            ui.add(egui::TextEdit::singleline(&mut dlg.port).hint_text(DEFAULT_PORT.to_string()).desired_width(300.0));
                            ui.end_row();
                            ui.label("Nome de login do dispositivo");
                            ui.add(egui::TextEdit::singleline(&mut dlg.user).hint_text(DEFAULT_USER).desired_width(300.0));
                            ui.end_row();
                            ui.label("Senha do Dispositivo");
                            ui.add(egui::TextEdit::singleline(&mut dlg.password).password(true).desired_width(300.0));
                            ui.end_row();
                        });
                        if let Some(tid) = &dlg.test_id {
                            let (t, c) = match self.net.state(tid) {
                                NetState::Online(ms) => {
                                    let sn = self.net.info(tid).map(|i| i.serial).filter(|s| !s.is_empty());
                                    (format!("Login aceito ({ms} ms){}.", sn.map(|s| format!(" · N.º de série {s}")).unwrap_or_default()), THEME)
                                }
                                NetState::Offline => ("O dispositivo pode estar off-line. Tente mais tarde".to_string(), ui::RED),
                                NetState::WrongPassword => ("Senha incorreta".to_string(), ui::RED),
                                NetState::Refused(code) => (format!("O dispositivo recusou o login (código {code})."), ui::RED),
                                NetState::NotCamera => ("Este endereço não responde como uma câmera compatível.".to_string(), ui::RED),
                                _ => ("Testando conexão...".to_string(), ui::ORANGE),
                            };
                            ui.label(egui::RichText::new(t).color(c));
                        }
                        // OnFunSDKResult 5009: added only when the device answers, else Device_NotExist
                        if let Some(p) = dlg.pending.clone() {
                            let key = format!("add:{}", p.id);
                            match self.net.state(&key) {
                                NetState::Online(_) => {
                                    // OnFunSDKResult 5128 SystemInfo: the device id becomes its SerialNo
                                    let mut d = p;
                                    if let Some(info) = self.net.info(&key) {
                                        if !info.serial.is_empty() {
                                            d.id = info.serial.clone();
                                        }
                                        if !info.software.is_empty() {
                                            d.firmware = info.software.clone();
                                        }
                                    }
                                    dlg.pending = None;
                                    if self.store.device(&d.id).is_some() {
                                        dlg.error = Some("O dispositivo já existe".into());
                                    } else {
                                        to_add = Some(d);
                                    }
                                }
                                NetState::Offline => {
                                    dlg.pending = None;
                                    dlg.error = Some("O dispositivo pode estar off-line. Tente mais tarde".into());
                                }
                                NetState::WrongPassword => {
                                    dlg.pending = None;
                                    dlg.error = Some("Senha incorreta".into());
                                }
                                NetState::Refused(code) => {
                                    dlg.pending = None;
                                    dlg.error = Some(format!("O dispositivo recusou o login (código {code})."));
                                }
                                NetState::NotCamera => {
                                    dlg.pending = None;
                                    dlg.error = Some("Este endereço não responde como uma câmera compatível.".into());
                                }
                                _ => {
                                    ui.label(egui::RichText::new("Adicionando dispositivo...").color(ui::ORANGE));
                                }
                            }
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui::secondary(ui, "Testar conexão").clicked() {
                                match parse_ip(&mut dlg) {
                                    Ok(d) => {
                                        let tid = format!("test:{}", d.id);
                                        self.net.check(&tid, &d.host, d.port, &d.user, &d.password);
                                        dlg.test_id = Some(tid);
                                        dlg.error = None;
                                    }
                                    Err(e) => dlg.error = Some(e),
                                }
                            }
                            // the original's Add button only works once an IP is typed (TextWatcher b)
                            let can_add = !dlg.host.trim().is_empty() && dlg.pending.is_none();
                            let add_btn = ui.add_enabled(
                                can_add,
                                egui::Button::new(egui::RichText::new("Adicionar").color(Color32::WHITE).strong())
                                    .fill(THEME)
                                    .corner_radius(CornerRadius::same(18))
                                    .min_size(egui::vec2(0.0, 34.0)),
                            );
                            if add_btn.clicked() {
                                match parse_ip(&mut dlg) {
                                    Ok(d) if self.store.device(&d.id).is_some() => dlg.error = Some("O dispositivo já existe".into()),
                                    Ok(d) => {
                                        self.net.check(&format!("add:{}", d.id), &d.host, d.port, &d.user, &d.password);
                                        dlg.error = None;
                                        dlg.pending = Some(d);
                                    }
                                    Err(e) => dlg.error = Some(e),
                                }
                            }
                        });
                        ui.label(egui::RichText::new("O app entra no dispositivo com o mesmo login das câmeras (porta 34567) e mostra o vídeo ao vivo (H.264 ou MJPEG; câmeras em H.265 ainda não).").small().color(TEXT3));
                    }
                    AddMode::Lan => {
                        let st = self.net.scan_state();
                        ui.label(egui::RichText::new(format!("Procura na rede local por dispositivos que respondem na porta {DEFAULT_PORT}.")).color(TEXT2));
                        ui.horizontal(|ui| {
                            if st.running {
                                ui.add(egui::ProgressBar::new(st.done as f32 / st.total.max(1) as f32).desired_width(300.0).text(format!("Pesquisando... {}", st.subnet)));
                            } else if ui::primary(ui, "Procurar").clicked() {
                                self.net.start_scan(DEFAULT_PORT);
                            }
                        });
                        if let Some(e) = &st.error {
                            ui.colored_label(ui::RED, e);
                        }
                        if !st.running && st.total > 0 && st.found.is_empty() {
                            ui.label(egui::RichText::new(format!("Nenhum dispositivo encontrado em {}.", st.subnet)).color(TEXT2));
                        }
                        for (ip, port, ms) in &st.found {
                            ui.horizontal(|ui| {
                                ui.label(format!("{ip}:{port}"));
                                ui.label(egui::RichText::new(format!("{ms} ms")).small().color(TEXT3));
                                let exists = self.store.device(&format!("{ip}:{port}")).is_some();
                                if exists {
                                    ui.label(egui::RichText::new("O dispositivo já existe").small().color(TEXT3));
                                } else if ui.button("Adicionar").clicked() {
                                    dlg.mode = AddMode::Ip;
                                    dlg.host = ip.clone();
                                    dlg.port = port.to_string();
                                }
                            });
                        }
                    }
                    AddMode::Code => {
                        ui::card_frame().show(ui, |ui| {
                            ui.set_width(490.0);
                            share_help(ui, ShareHelp::Receive);
                        });
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Código de compartilhamento").color(TEXT2));
                        ui.add(egui::TextEdit::multiline(&mut dlg.code).desired_rows(3).desired_width(500.0).hint_text("VC1-..."));
                        if ui::primary(ui, "Adicionar").clicked() {
                            match model::device_from_code(&dlg.code) {
                                Some(d) => to_add = Some(d),
                                None => dlg.error = Some("Código inválido".into()),
                            }
                        }
                    }
                    AddMode::Demo => {
                        ui.label(egui::RichText::new("Uma câmera simulada, para conhecer o aplicativo.").color(TEXT2));
                        ui.horizontal(|ui| {
                            ui.label("Nome do dispositivo");
                            ui.add(egui::TextEdit::singleline(&mut dlg.name).hint_text("Câmera").desired_width(260.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Cena");
                            for (i, n) in SCENE_NAMES.iter().enumerate() {
                                if ui::chip(ui, dlg.scene == i as u8, n).clicked() {
                                    dlg.scene = i as u8;
                                }
                            }
                        });
                        if ui::primary(ui, "Adicionar").clicked() {
                            let name = if dlg.name.trim().is_empty() { format!("Câmera {}", SCENE_NAMES[dlg.scene as usize]) } else { dlg.name.trim().to_string() };
                            if let Some(l) = model::name_too_long(&name) {
                                dlg.error = Some(format!("Digitar nome do dispositivo (1 e {l} caracteres)"));
                            } else {
                                let seed = crate::scene::hash64(model::now_f().to_bits());
                                to_add = Some(Device::demo(&name, dlg.scene, seed));
                            }
                        }
                    }
                }
                if let Some(e) = &dlg.error {
                    ui.colored_label(ui::RED, e);
                }
            });
        if let Some(d) = to_add {
            let name = d.name.clone();
            match self.add_device(d) {
                Ok(()) => {
                    self.toast(format!("Dispositivo adicionado: {name}"));
                    close = true;
                }
                Err(e) => dlg.error = Some(e),
            }
        }
        if open && !close {
            self.add = Some(dlg);
        }
    }

    fn share_dialog(&mut self, ctx: &egui::Context, id: &str) {
        let Some(d) = self.store.device(id).cloned() else {
            self.share = None;
            return;
        };
        let code = model::share_code(&d);
        let mut open = true;
        let mut copied = false;
        egui::Window::new("Compartilhar com amigos")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.vertical_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(170.0, 170.0), Sense::hover());
                    draw_pattern(ui.painter(), r, &code);
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(&d.name).strong().color(TEXT));
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Envie este código para quem vai adicionar a câmera. A pessoa abre + Adicionar dispositivo › Código, cola o código e clica em Adicionar.").color(TEXT2));
                let mut c = code.clone();
                ui.add(egui::TextEdit::multiline(&mut c).desired_rows(3).desired_width(420.0).font(egui::TextStyle::Monospace));
                if !d.is_demo() {
                    ui.label(egui::RichText::new("A senha do dispositivo não vai no código.").small().color(TEXT3));
                }
                ui.add_space(6.0);
                if ui::primary(ui, "Copiar código").clicked() {
                    ctx.copy_text(code.clone());
                    copied = true;
                }
            });
        if copied {
            self.store.shares.push(model::ShareRec { ts: model::now_i(), dev_id: d.id.clone(), dev_name: d.name.clone(), code });
            self.store.log(&format!("Código de compartilhamento criado: {}", d.name));
            self.dirty = true;
            self.toast("Código copiado");
        }
        if !open {
            self.share = None;
        }
    }
}

/// AddDeviceByIPActivity.K1, in its order: name length, then IPv4 (g3.f.X0), then the port
/// (digits only, else 34567, written back to the field), empty name = IP, login aliases → admin.
fn parse_ip(dlg: &mut AddDlg) -> Result<Device, String> {
    let name_in = dlg.name.trim().to_string();
    if !name_in.is_empty() {
        if let Some(l) = model::name_too_long(&name_in) {
            return Err(format!("Digitar nome do dispositivo (1 e {l} caracteres)"));
        }
    }
    let host = dlg.host.trim().to_string();
    if !model::valid_host(&host) {
        return Err("Por favor, insira um IP".into());
    }
    let port = model::parse_port(&dlg.port);
    dlg.port = port.to_string();
    let name = if name_in.is_empty() {
        dlg.name = host.clone();
        host.clone()
    } else {
        name_in
    };
    let user = model::normalize_user(&dlg.user);
    if user != dlg.user.trim() {
        dlg.user = user.clone();
    }
    Ok(Device::ip(&name, &host, port, &user, dlg.password.trim()))
}

/// A square pattern drawn from the code (looks like a QR code; scan is not supported).
fn draw_pattern(p: &egui::Painter, r: Rect, code: &str) {
    p.rect_filled(r, CornerRadius::same(6), Color32::WHITE);
    p.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0_f32, ui::DIVIDER), egui::StrokeKind::Inside);
    let n = 25;
    let cell = (r.width() - 16.0) / n as f32;
    let mut h: u64 = 1469598103934665603;
    for b in code.bytes() {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    let finder = |x: usize, y: usize| -> Option<bool> {
        for (fx, fy) in [(0, 0), (n - 7, 0), (0, n - 7)] {
            if x >= fx && x < fx + 7 && y >= fy && y < fy + 7 {
                let dx = x - fx;
                let dy = y - fy;
                let ring = dx == 0 || dy == 0 || dx == 6 || dy == 6;
                let core = (2..5).contains(&dx) && (2..5).contains(&dy);
                return Some(ring || core);
            }
        }
        None
    };
    for y in 0..n {
        for x in 0..n {
            let on = finder(x, y).unwrap_or_else(|| crate::scene::h2(h, (y * n + x) as u64) % 2 == 0);
            if on {
                let c = Rect::from_min_size(r.min + egui::vec2(8.0 + x as f32 * cell, 8.0 + y as f32 * cell), egui::vec2(cell + 0.5, cell + 0.5));
                p.rect_filled(c, CornerRadius::ZERO, Color32::from_rgb(0x1E, 0x22, 0x31));
            }
        }
    }
}

/// Which part of the sharing help to show.
#[derive(Clone, Copy, PartialEq)]
pub enum ShareHelp {
    /// For whoever owns the camera and wants to share it.
    Send,
    /// For whoever received a code and wants to add the camera.
    Receive,
    /// Everything, including how to add a brand-new camera.
    All,
}

fn help_steps(ui: &mut egui::Ui, title: &str, steps: &[&str]) {
    ui.label(egui::RichText::new(title).strong().color(TEXT));
    for (i, s) in steps.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 10.0, ui::THEME_SOFT);
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, (i + 1).to_string(), egui::FontId::proportional(12.0), ui::THEME_DARK);
            ui.label(egui::RichText::new(*s).color(TEXT2));
        });
    }
}

/// How sharing works in this app: a code made from the camera's details, sent by any
/// messenger and pasted on the other computer. (The original shares through its cloud
/// account, which this version doesn't have.)
pub fn share_help(ui: &mut egui::Ui, which: ShareHelp) {
    if which != ShareHelp::Receive {
        help_steps(
            ui,
            "Compartilhar uma câmera que você já tem",
            &[
                "No cartão da câmera, clique em Compartilhar.",
                "Clique em Copiar código e envie o código para a outra pessoa (WhatsApp, e-mail...).",
                "Os códigos enviados ficam em Eu › Gerenciamento do compartilhamento.",
            ],
        );
        ui.add_space(6.0);
    }
    if which != ShareHelp::Send {
        help_steps(
            ui,
            "Adicionar uma câmera que alguém compartilhou com você",
            &[
                "Clique em + Adicionar dispositivo e abra a aba Código.",
                "Cole o código recebido (Ctrl+V) e clique em Adicionar.",
                "A câmera aparece na lista com o mesmo nome. Se for uma câmera por IP, digite a senha em Conf. › Gerenciamento de senha (a senha não vai no código) e use um computador que alcance o IP da câmera.",
            ],
        );
        ui.add_space(6.0);
    }
    if which == ShareHelp::All {
        help_steps(
            ui,
            "Adicionar uma câmera nova (sem código)",
            &[
                "Clique em + Adicionar dispositivo.",
                "Na aba Adicionar via IP, digite o IP da câmera, a porta (34567), o usuário (admin) e a senha; ou use Procurar por dispositivo para achar câmeras na sua rede.",
                "O app entra na câmera e, se o login for aceito, ela é adicionada.",
            ],
        );
        ui.add_space(6.0);
    }
    ui.label(
        egui::RichText::new(
            "No app original o compartilhamento é feito pela conta na nuvem; aqui é por código, sem conta. \
             Remover um código da lista não desativa um código já enviado.",
        )
        .small()
        .color(TEXT3),
    );
}
