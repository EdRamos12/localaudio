use crate::protocol::TCP_PORT;
use crate::tray::{Action, Tray};
use crate::{Engine, EngineState};
use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Margin, Rect,
    RichText, Sense, Stroke, TextStyle,
};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BG: Color32 = Color32::from_rgb(0x0D, 0x11, 0x17);
const SURFACE: Color32 = Color32::from_rgb(0x16, 0x1E, 0x26);
const SURFACE_HIGH: Color32 = Color32::from_rgb(0x1C, 0x25, 0x2E);
const SURFACE_HOVER: Color32 = Color32::from_rgb(0x24, 0x30, 0x3A);
const SURFACE_ACTIVE: Color32 = Color32::from_rgb(0x2C, 0x3A, 0x46);
const BORDER: Color32 = Color32::from_rgb(0x26, 0x31, 0x3B);
const TEXT: Color32 = Color32::from_rgb(0xE6, 0xED, 0xF3);
const MUTED: Color32 = Color32::from_rgb(0x9A, 0xA7, 0xB4);
const ACCENT: Color32 = Color32::from_rgb(0x5E, 0xEA, 0xD4);
const ACCENT_DIM: Color32 = Color32::from_rgb(0x2D, 0x9C, 0x8C);
const ACCENT_CONTAINER: Color32 = Color32::from_rgb(0x13, 0x4E, 0x48);
const ON_ACCENT: Color32 = Color32::from_rgb(0x00, 0x38, 0x2F);
const WARN: Color32 = Color32::from_rgb(0xFB, 0xBF, 0x24);
const DANGER: Color32 = Color32::from_rgb(0xFC, 0xA5, 0xA5);
const TRACK: Color32 = Color32::from_rgb(0x23, 0x2E, 0x38);

pub struct DesktopUi {
    engine: Arc<Engine>,
    tray: Option<Rc<Tray>>,
    passphrase_edit: String,
    show_passphrase: bool,
    volume: u8,
    level: f32,
    copied_at: Option<Instant>,
}

impl DesktopUi {
    pub fn new(cc: &eframe::CreationContext<'_>, engine: Arc<Engine>, tray: Option<Rc<Tray>>) -> Self {
        apply_theme(&cc.egui_ctx);
        let passphrase_edit = engine
            .passphrase
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let volume = engine.server.volume();
        Self {
            engine,
            tray,
            passphrase_edit,
            show_passphrase: false,
            volume,
            level: 0.0,
            copied_at: None,
        }
    }

    fn stream_card(&mut self, ui: &mut egui::Ui) {
        let running = self.engine.running.load(Ordering::Relaxed);
        let state = self.engine.state();
        let listeners = self.engine.server.listener_count();
        let name = self.engine.server.name.clone();

        let (pill, pill_color, headline, detail, detail_color) = match (running, &state) {
            (false, _) => (
                "Stopped",
                MUTED,
                "Streaming is off".to_string(),
                "Phones can't hear this computer right now.".to_string(),
                MUTED,
            ),
            (true, EngineState::Error(message)) => (
                "Problem",
                DANGER,
                "Can't capture audio".to_string(),
                message.clone(),
                DANGER,
            ),
            (true, EngineState::Capturing) if listeners > 0 => (
                "Live",
                ACCENT,
                format!(
                    "Streaming to {listeners} phone{}",
                    if listeners == 1 { "" } else { "s" }
                ),
                "Opus · 48 kHz stereo · 128 kbps".to_string(),
                MUTED,
            ),
            (true, EngineState::Capturing) => (
                "Ready",
                ACCENT,
                "Waiting for a phone".to_string(),
                format!("Open LocalAudio on your phone and tap “{name}”."),
                MUTED,
            ),
            (true, _) => (
                "Starting",
                WARN,
                "Opening the audio source…".to_string(),
                String::new(),
                MUTED,
            ),
        };

        card(ui, None, |ui| {
            ui.horizontal(|ui| {
                let pulse = pill == "Live";
                status_pill(ui, pill, pill_color, pulse);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(&name).color(MUTED));
                });
            });
            ui.add_space(6.0);
            ui.label(RichText::new(headline).size(19.0).family(bold()).color(TEXT));
            if !detail.is_empty() {
                ui.label(RichText::new(detail).color(detail_color));
            }
            ui.add_space(10.0);
            level_meter(ui, self.level);
            ui.add_space(12.0);

            let address = format!("{}:{}", self.engine.server.address, TCP_PORT);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Address").color(MUTED));
                ui.label(RichText::new(&address).monospace().color(TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let copied = self
                        .copied_at
                        .is_some_and(|at| at.elapsed() < Duration::from_millis(1500));
                    if ui
                        .add(egui::Button::new(if copied { "Copied" } else { "Copy" }).small())
                        .on_hover_text("For phones that don't find this computer automatically")
                        .clicked()
                    {
                        ui.ctx().copy_text(address.clone());
                        self.copied_at = Some(Instant::now());
                    }
                });
            });
            ui.add_space(10.0);

            let clicked = if running {
                big_button(ui, "Stop streaming", SURFACE_HIGH, TEXT)
            } else {
                big_button(ui, "Start streaming", ACCENT, ON_ACCENT)
            };
            if clicked {
                self.engine.running.store(!running, Ordering::Relaxed);
            }
        });
    }

    fn source_card(&mut self, ui: &mut egui::Ui) {
        let engine = Arc::clone(&self.engine);
        card(ui, Some("SOUND SOURCE"), |ui| {
            let sources = engine.sources();
            let selected = engine
                .source
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone();
            let selected_label = sources
                .iter()
                .find(|source| source.id == selected)
                .map(|source| source.label.clone())
                .unwrap_or_else(|| selected.clone());
            let mut chosen = None;
            ui.horizontal(|ui| {
                let width = (ui.available_width() - button_width(ui, "Refresh")).max(120.0);
                egui::ComboBox::from_id_salt("source")
                    .width(width)
                    .height(320.0)
                    .selected_text(RichText::new(selected_label).color(TEXT))
                    .show_ui(ui, |ui| {
                        for source in &sources {
                            if ui
                                .selectable_label(source.id == selected, &source.label)
                                .clicked()
                            {
                                chosen = Some(source.id.clone());
                            }
                        }
                    });
                if ui
                    .button("Refresh")
                    .on_hover_text("Look for new outputs")
                    .clicked()
                {
                    engine.refresh_sources();
                }
            });
            if let Some(id) = chosen {
                *engine.source.lock().unwrap_or_else(|err| err.into_inner()) = id;
            }
            ui.label(
                RichText::new("Everything this output plays is sent to your phone.")
                    .small()
                    .color(MUTED),
            );

            #[cfg(target_os = "linux")]
            {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                let on = engine.phone_only.load(Ordering::Relaxed);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Play only on the phone").color(TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if toggle(ui, on).clicked() {
                            engine.toggle_phone_only();
                        }
                    });
                });
                ui.label(
                    RichText::new(
                        "Switches apps to a silent output so this computer stays quiet. \
                         Turned back off when you quit LocalAudio.",
                    )
                    .small()
                    .color(MUTED),
                );
            }
        });
    }

    fn phones_card(&mut self, ui: &mut egui::Ui) {
        card(ui, Some("PHONES"), |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Phone volume").color(TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{}%", self.volume)).family(bold()).color(ACCENT));
                });
            });
            ui.spacing_mut().slider_width = ui.available_width();
            if ui
                .add(egui::Slider::new(&mut self.volume, 0..=100).show_value(false))
                .changed()
            {
                self.engine.server.set_volume(self.volume);
            }

            ui.add_space(10.0);
            ui.label(RichText::new("Passphrase").color(TEXT));
            ui.horizontal(|ui| {
                let margin = vec2(10.0, 7.0);
                let width =
                    (ui.available_width() - button_width(ui, "Show") - margin.x * 2.0 - 4.0)
                        .max(120.0);
                let edit = egui::TextEdit::singleline(&mut self.passphrase_edit)
                    .password(!self.show_passphrase)
                    .hint_text("Optional")
                    .margin(margin)
                    .desired_width(width);
                if ui.add(edit).changed() {
                    *self
                        .engine
                        .passphrase
                        .lock()
                        .unwrap_or_else(|err| err.into_inner()) = self.passphrase_edit.clone();
                }
                if ui
                    .button(if self.show_passphrase { "Hide" } else { "Show" })
                    .clicked()
                {
                    self.show_passphrase = !self.show_passphrase;
                }
            });
            let hint = if self.passphrase_edit.is_empty() {
                "Any phone on this network can listen."
            } else {
                "Phones need to type this passphrase to listen."
            };
            ui.label(RichText::new(hint).small().color(MUTED));
        });
    }
}

impl eframe::App for DesktopUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(50));
        match self.tray.as_ref().and_then(|tray| tray.poll()) {
            Some(Action::Open) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            Some(Action::Quit) => {
                if let Some(tray) = &self.tray {
                    tray.quitting.set(true);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            None => {}
        }
        let target = level_of(self.engine.rms.load(Ordering::Relaxed));
        self.level = target.max(self.level - 0.04);

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(Margin::same(20)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        header(ui);
                        ui.add_space(12.0);
                        self.stream_card(ui);
                        ui.add_space(4.0);
                        self.source_card(ui);
                        ui.add_space(4.0);
                        self.phones_card(ui);
                        ui.add_space(10.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("No ads · No account · Audio stays on your network")
                                    .small()
                                    .color(MUTED),
                            );
                        });
                    });
            });
    }
}

const REGULAR_FONTS: &[&str] = &[
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/TTF/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
];

const BOLD_FONTS: &[&str] = &[
    "/usr/share/fonts/noto/NotoSans-SemiBold.ttf",
    "/usr/share/fonts/noto/NotoSans-Bold.ttf",
    "/usr/share/fonts/TTF/NotoSans-SemiBold.ttf",
    "/usr/share/fonts/TTF/NotoSans-Bold.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
    "/usr/share/fonts/google-noto/NotoSans-SemiBold.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
    "C:\\Windows\\Fonts\\seguisb.ttf",
    "C:\\Windows\\Fonts\\segoeuib.ttf",
];

fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

fn first_font(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| std::fs::read(path).ok())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(data) = first_font(REGULAR_FONTS) {
        fonts
            .font_data
            .insert("ui-regular".into(), Arc::new(egui::FontData::from_owned(data)));
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, "ui-regular".into());
        }
    }
    let mut bold_family = Vec::new();
    if let Some(data) = first_font(BOLD_FONTS) {
        fonts
            .font_data
            .insert("ui-bold".into(), Arc::new(egui::FontData::from_owned(data)));
        bold_family.push("ui-bold".to_string());
    }
    bold_family.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts.families.insert(bold(), bold_family);
    ctx.set_fonts(fonts);
}

pub fn show_startup_error(message: String) -> anyhow::Result<()> {
    let busy = message.contains("Address already in use");
    let headline = if busy {
        "LocalAudio is already running"
    } else {
        "LocalAudio couldn't start"
    };
    let detail = if busy {
        "Close the other LocalAudio window, then open it again.".to_string()
    } else {
        message
    };
    let mut themed = false;
    eframe::run_simple_native(
        "LocalAudio",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("LocalAudio")
                .with_app_id("localaudio")
                .with_inner_size([420.0, 190.0])
                .with_resizable(false)
                .with_icon(app_icon()),
            ..Default::default()
        },
        move |ctx, _frame| {
            if !themed {
                apply_theme(ctx);
                themed = true;
                ctx.request_repaint();
                return;
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(BG).inner_margin(Margin::same(24)))
                .show(ctx, |ui| {
                    ui.label(RichText::new(headline).size(18.0).family(bold()).color(TEXT));
                    ui.add_space(2.0);
                    ui.label(RichText::new(&detail).color(MUTED));
                    ui.add_space(16.0);
                    if big_button(ui, "Close", SURFACE_HIGH, TEXT) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
        },
    )
    .map_err(|err| anyhow::anyhow!(err.to_string()))
}

fn apply_theme(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.style_mut(|style| {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = SURFACE_HIGH;
        visuals.extreme_bg_color = Color32::from_rgb(0x0A, 0x0E, 0x13);
        visuals.faint_bg_color = SURFACE;
        visuals.selection.bg_fill = ACCENT_DIM;
        visuals.selection.stroke = line(1.0, ACCENT);
        visuals.hyperlink_color = ACCENT;
        visuals.slider_trailing_fill = true;
        visuals.window_stroke = line(1.0, BORDER);
        visuals.window_corner_radius = CornerRadius::same(12);
        visuals.menu_corner_radius = CornerRadius::same(10);
        visuals.popup_shadow = egui::epaint::Shadow::NONE;
        visuals.window_shadow = egui::epaint::Shadow::NONE;

        let radius = CornerRadius::same(8);
        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_fill = SURFACE;
        widgets.noninteractive.weak_bg_fill = SURFACE;
        widgets.noninteractive.bg_stroke = line(1.0, BORDER);
        widgets.noninteractive.fg_stroke = line(1.0, TEXT);
        widgets.noninteractive.corner_radius = radius;

        widgets.inactive.bg_fill = TRACK;
        widgets.inactive.weak_bg_fill = SURFACE_HIGH;
        widgets.inactive.bg_stroke = line(1.0, BORDER);
        widgets.inactive.fg_stroke = line(1.0, TEXT);
        widgets.inactive.corner_radius = radius;

        widgets.hovered.bg_fill = SURFACE_HOVER;
        widgets.hovered.weak_bg_fill = SURFACE_HOVER;
        widgets.hovered.bg_stroke = line(1.0, ACCENT_DIM);
        widgets.hovered.fg_stroke = line(1.5, TEXT);
        widgets.hovered.corner_radius = radius;
        widgets.hovered.expansion = 0.0;

        widgets.active.bg_fill = SURFACE_ACTIVE;
        widgets.active.weak_bg_fill = SURFACE_ACTIVE;
        widgets.active.bg_stroke = line(1.0, ACCENT);
        widgets.active.fg_stroke = line(1.5, TEXT);
        widgets.active.corner_radius = radius;
        widgets.active.expansion = 0.0;

        widgets.open.bg_fill = SURFACE_HOVER;
        widgets.open.weak_bg_fill = SURFACE_HOVER;
        widgets.open.bg_stroke = line(1.0, ACCENT_DIM);
        widgets.open.fg_stroke = line(1.0, TEXT);
        widgets.open.corner_radius = radius;

        style.visuals = visuals;
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.button_padding = vec2(12.0, 6.0);
        style.spacing.interact_size.y = 30.0;
        style.spacing.combo_height = 320.0;
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(22.0)),
            (TextStyle::Body, FontId::proportional(14.5)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Monospace, FontId::monospace(13.5)),
        ]
        .into();
    });
}

fn header(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
        paint_mark(ui.painter(), rect);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add_space(2.0);
            ui.label(RichText::new("LocalAudio").size(22.0).family(bold()).color(TEXT));
            ui.label(RichText::new("Send this computer's sound to your phone").color(MUTED));
        });
    });
}

fn card(ui: &mut egui::Ui, title: Option<&str>, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default()
        .fill(SURFACE)
        .stroke(line(1.0, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(title) = title {
                ui.label(RichText::new(title).size(11.5).family(bold()).color(MUTED));
                ui.add_space(2.0);
            }
            add_contents(ui);
        });
}

fn big_button(ui: &mut egui::Ui, text: &str, fill: Color32, text_color: Color32) -> bool {
    let button = egui::Button::new(RichText::new(text).size(15.0).family(bold()).color(text_color))
        .fill(fill)
        .stroke(if fill == ACCENT {
            Stroke::NONE
        } else {
            line(1.0, BORDER)
        })
        .corner_radius(CornerRadius::same(10));
    ui.add_sized([ui.available_width(), 44.0], button)
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
}

fn status_pill(ui: &mut egui::Ui, text: &str, color: Color32, pulse: bool) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(13.0), color);
    let size = vec2(galley.size().x + 34.0, 26.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(13), color.gamma_multiply(0.14));
    let dot = if pulse {
        let time = ui.input(|input| input.time);
        color.gamma_multiply(0.6 + 0.4 * ((time * 3.0).sin() as f32 * 0.5 + 0.5))
    } else {
        color
    };
    painter.circle_filled(pos2(rect.left() + 14.0, rect.center().y), 4.0, dot);
    painter.galley(
        pos2(rect.left() + 25.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}

fn level_meter(ui: &mut egui::Ui, level: f32) {
    let segments = 32;
    let height = 10.0;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let gap = 3.0;
    let segment_width = (width - gap * (segments as f32 - 1.0)) / segments as f32;
    let lit = (level * segments as f32).round() as usize;
    let painter = ui.painter();
    for index in 0..segments {
        let left = rect.left() + index as f32 * (segment_width + gap);
        let segment = Rect::from_min_size(pos2(left, rect.top()), vec2(segment_width, height));
        let color = if index >= lit {
            TRACK
        } else if index as f32 >= segments as f32 * 0.85 {
            WARN
        } else {
            ACCENT
        };
        painter.rect_filled(segment, CornerRadius::same(2), color);
    }
}

fn toggle(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    let t = ui.ctx().animate_bool(response.id, on);
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(11), mix(TRACK, ACCENT, t));
    let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t);
    painter.circle_filled(pos2(x, rect.center().y), 8.0, Color32::WHITE);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

fn paint_mark(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, CornerRadius::same(12), ACCENT_CONTAINER);
    let heights = [0.34, 0.62, 0.46, 0.24];
    let stroke = rect.width() * 0.11;
    for (index, height) in heights.iter().enumerate() {
        let x = rect.left() + rect.width() * (index as f32 + 1.0) / 5.0;
        let half = rect.height() * height / 2.0;
        let top = pos2(x, rect.center().y - half);
        let bottom = pos2(x, rect.center().y + half);
        painter.line_segment([top, bottom], Stroke::new(stroke, ACCENT));
        painter.circle_filled(top, stroke / 2.0, ACCENT);
        painter.circle_filled(bottom, stroke / 2.0, ACCENT);
    }
}

pub fn app_icon() -> egui::IconData {
    let size = 64u32;
    let full = size as f32;
    let radius = 14.0f32;
    let background = [0x13u8, 0x4E, 0x48];
    let foreground = [0x5Eu8, 0xEA, 0xD4];
    let heights = [0.34f32, 0.62, 0.46, 0.24];
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let cx = fx.clamp(radius, full - radius);
            let cy = fy.clamp(radius, full - radius);
            if (fx - cx).powi(2) + (fy - cy).powi(2) > radius * radius {
                continue;
            }
            let mut color = background;
            for (index, height) in heights.iter().enumerate() {
                let bar_x = full * (index as f32 + 1.0) / 5.0;
                let half = full * height / 2.0;
                let over = (fy - full / 2.0).abs() - half;
                let distance = if over > 0.0 {
                    ((fx - bar_x).powi(2) + over.powi(2)).sqrt()
                } else {
                    (fx - bar_x).abs()
                };
                if distance <= 3.5 {
                    color = foreground;
                }
            }
            let offset = ((y * size + x) * 4) as usize;
            rgba[offset..offset + 3].copy_from_slice(&color);
            rgba[offset + 3] = 255;
        }
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

fn button_width(ui: &egui::Ui, text: &str) -> f32 {
    let font = TextStyle::Button.resolve(ui.style());
    let text_width = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, TEXT)
        .size()
        .x;
    text_width + ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x + 2.0
}

fn line(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let channel = |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * t).round() as u8;
    Color32::from_rgb(channel(a.r(), b.r()), channel(a.g(), b.g()), channel(a.b(), b.b()))
}

fn level_of(rms: u64) -> f32 {
    if rms == 0 {
        return 0.0;
    }
    let db = 20.0 * (rms as f32 / 32768.0).log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}
