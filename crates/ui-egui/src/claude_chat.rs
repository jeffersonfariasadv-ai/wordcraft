//! The Claude chat window: type what to do to the document and Claude does it.
//!
//! Claude runs as the person's own installed Claude Code (`claude -p`, signed in with their
//! Claude account, so a subscription works and no API key is stored here). It is given one MCP
//! server, `wordcraft-cli mcp --connect 127.0.0.1:<port>`, which drives this very window through
//! the control channel (`docs/mcp.md`), so every edit goes through the same commands, and the same
//! undo, as the keyboard. The control channel is started the first time the window is used.
//!
//! Desktop only: a browser cannot start a process.

use std::collections::BTreeMap;
use std::sync::mpsc::Receiver;

use egui::{Align, Context, Key, Rect, RichText, ScrollArea, Sense, Stroke, TextEdit, vec2};

use crate::theme::{Tokens, regular, semibold};
use crate::{ControlRequest, WordApp, icons};

/// What the host hands over to start the control channel on demand: the requests it will receive
/// and the loopback port it listens on.
pub type Bridge = (Receiver<ControlRequest>, u16);

/// One line of the conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Line {
    You(String),
    Claude(String),
    /// A tool Claude used ("execute", "get_text"…), shown small.
    Tool(String),
    /// Something the window says itself: an error, a hint.
    Note(String),
}

/// What the worker thread reports.
#[derive(Debug)]
pub(crate) enum Event {
    Session(String),
    /// The model Claude Code is using, as it reports it (`claude-opus-5-5`).
    Model(String),
    Text(String),
    Tool(String),
    Failed(String),
    Done,
}

/// The window's state.
#[derive(Default)]
pub struct Chat {
    pub(crate) open: bool,
    pub(crate) input: String,
    pub(crate) lines: Vec<Line>,
    pub(crate) rx: Option<Receiver<Event>>,
    /// Claude's session id, so the next message continues the same conversation.
    pub(crate) session: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) child: Option<native::Handle>,
    /// The model the last conversation used, whatever the choice was.
    pub(crate) last_model: Option<String>,
    /// The models were asked for this run.
    probed: bool,
    #[cfg(not(target_arch = "wasm32"))]
    probe: Option<Receiver<(String, String)>>,
    focus_input: bool,
}

impl Chat {
    pub(crate) fn running(&self) -> bool {
        self.rx.is_some()
    }

    /// Apply what the worker has reported since the last frame.
    pub(crate) fn apply(&mut self, ev: Event) {
        match ev {
            Event::Session(id) => self.session = Some(id),
            // Needs the app's settings: handled in `poll`.
            Event::Model(_) => {}
            Event::Text(t) => {
                if !t.trim().is_empty() {
                    self.lines.push(Line::Claude(t.trim().to_string()));
                }
            }
            Event::Tool(name) => {
                if self.lines.last() != Some(&Line::Tool(name.clone())) {
                    self.lines.push(Line::Tool(name));
                }
            }
            Event::Failed(e) => {
                self.lines.push(Line::Note(e));
                self.finish();
            }
            Event::Done => self.finish(),
        }
    }

    fn finish(&mut self) {
        self.rx = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.child = None;
        }
        self.focus_input = true;
    }

    fn new_chat(&mut self) {
        self.stop();
        self.lines.clear();
        self.session = None;
    }

    fn stop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(c) = &self.child {
                c.kill();
            }
        }
    }
}

/// The models the panel offers: the value for `claude --model` (an alias for the latest model of
/// that family; empty leaves it to Claude Code) and its name.
pub(crate) const MODELS: [(&str, &str); 5] = [("", "Padrão"), ("fable", "Fable"), ("opus", "Opus"), ("sonnet", "Sonnet"), ("haiku", "Haiku")];

/// The efforts: the value for `claude --effort` and its name.
pub(crate) const EFFORTS: [(&str, &str); 6] =
    [("", "Padrão"), ("low", "Baixo"), ("medium", "Médio"), ("high", "Alto"), ("xhigh", "Muito alto"), ("max", "Máximo")];

/// What the choices were when this was written: the 5.5 family and Fable 5.2. Only a first guess
/// for the names in the list; every run asks Claude Code which model each choice really is.
pub(crate) fn default_models() -> BTreeMap<String, String> {
    [("fable", "claude-fable-5-2"), ("opus", "claude-opus-5-5"), ("sonnet", "claude-sonnet-5-5"), ("haiku", "claude-haiku-5-5")]
        .into_iter()
        .map(|(a, m)| (a.to_string(), m.to_string()))
        .collect()
}

/// A model id such as `claude-opus-5-5` (or `claude-haiku-4-5-20251001`) as its family and version:
/// `("Opus", "5.5")`. Anything else, which includes whatever else a program might report, is `None`.
fn parse_model(id: &str) -> Option<(String, String)> {
    if id.len() > 48 || !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
        return None;
    }
    let mut parts = id.strip_prefix("claude-")?.split('-');
    let family = parts.next().filter(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_lowercase()))?;
    let version: Vec<&str> = parts.take_while(|p| !p.is_empty() && p.len() <= 2 && p.bytes().all(|b| b.is_ascii_digit())).collect();
    let mut letters = family.chars();
    let name = letters.next().map(|first| first.to_uppercase().collect::<String>() + letters.as_str())?;
    Some((name, version.join(".")))
}

/// `Opus 5.5` for `claude-opus-5-5`.
fn model_label(id: &str) -> Option<String> {
    let (name, version) = parse_model(id)?;
    Some(if version.is_empty() { name } else { format!("{name} {version}") })
}

/// The name of a choice in the list: its model and version as last seen, or just the family.
fn choice_name(models: &BTreeMap<String, String>, alias: &str, family: &str) -> String {
    models.get(alias).and_then(|id| model_label(id)).unwrap_or_else(|| family.to_string())
}

/// A choice turned out to be this model: remember it, so the list names the version in use.
fn set_resolved(app: &mut WordApp, alias: &str, id: &str) {
    if MODELS.iter().any(|(a, _)| !a.is_empty() && *a == alias) && parse_model(id).is_some() {
        app.ui.claude_models.insert(alias.to_string(), id.to_string());
    }
}

/// Claude Code says which model answers: remember it for the header, and for its family's choice.
fn note_model(app: &mut WordApp, id: &str) {
    let Some((family, _)) = parse_model(id) else { return };
    app.claude.last_model = Some(id.to_string());
    set_resolved(app, &family.to_lowercase(), id);
}

/// What the header says the next message uses.
fn current_model_label(app: &WordApp) -> String {
    match known(&MODELS, &app.ui.claude_model) {
        "" => match app.claude.last_model.as_deref().and_then(model_label) {
            Some(m) => format!("Padrão ({m})"),
            None => "Modelo padrão".to_string(),
        },
        alias => choice_name(&app.ui.claude_models, alias, alias),
    }
}

/// `value` when it is one of `options` (what a saved setting or a command may hold is not
/// trusted to be), else empty: Claude Code's default.
fn known(options: &[(&'static str, &'static str)], value: &str) -> &'static str {
    options.iter().find(|(v, _)| *v == value).map_or("", |(v, _)| *v)
}

/// The command line options that carry the choices to `claude`.
pub(crate) fn option_args(model: &str, effort: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (flag, options, value) in [("--model", &MODELS[..], model), ("--effort", &EFFORTS[..], effort)] {
        let v = known(options, value);
        if !v.is_empty() {
            out.push(flag.to_string());
            out.push(v.to_string());
        }
    }
    out
}

/// `ui.claude` with `model` and/or `effort`: choose them. A value that is not offered is an error.
pub(crate) fn set_options(app: &mut WordApp, model: Option<&str>, effort: Option<&str>) -> Result<(), String> {
    for (value, options, name) in [(model, &MODELS[..], "model"), (effort, &EFFORTS[..], "effort")] {
        if let Some(v) = value
            && known(options, v) != v
        {
            let all: Vec<&str> = options.iter().map(|(v, _)| *v).collect();
            return Err(format!("`{name}` must be one of {all:?} (empty is the default)"));
        }
    }
    if let Some(m) = model {
        app.ui.claude_model = m.to_string();
    }
    if let Some(e) = effort {
        app.ui.claude_effort = e.to_string();
    }
    Ok(())
}

/// `ui.claude`: show the chat window, or hide it when it is showing (`value` true or false says
/// which).
pub(crate) fn toggle(app: &mut WordApp, value: Option<bool>) -> bool {
    let open = value.unwrap_or(!app.claude.open);
    app.claude.open = open;
    app.claude.focus_input = open;
    open
}

/// Take in what the worker has said since the last frame. Runs every frame, shown or not, so a
/// reply that arrives while the panel is closed is waiting when it opens.
pub(crate) fn poll(app: &mut WordApp, ctx: &Context) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rx) = &app.claude.probe {
        let found: Vec<(String, String)> = rx.try_iter().collect();
        for (alias, id) in found {
            set_resolved(app, &alias, &id);
        }
    }
    let Some(rx) = &app.claude.rx else { return };
    let mut events = Vec::new();
    let mut gone = false;
    loop {
        match rx.try_recv() {
            Ok(ev) => events.push(ev),
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                gone = true;
                break;
            }
        }
    }
    for ev in events {
        if let Event::Model(id) = &ev {
            note_model(app, id);
        }
        app.claude.apply(ev);
    }
    if gone {
        app.claude.finish();
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

/// What a frame of the panel asks for.
#[derive(Default)]
struct Act {
    send: Option<String>,
    stop: bool,
    new_chat: bool,
    close: bool,
}

/// A small round-cornered icon button: `kind` is `close` or `new`.
fn icon_button(ui: &mut egui::Ui, t: &Tokens, kind: &str, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    let c = r.center();
    let st = Stroke::new(1.5, t.icon);
    match kind {
        "close" => {
            ui.painter().line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], st);
            ui.painter().line_segment([c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)], st);
        }
        _ => {
            ui.painter().line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], st);
            ui.painter().line_segment([c + vec2(0.0, -5.0), c + vec2(0.0, 5.0)], st);
        }
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
    resp.on_hover_text(tip).clicked()
}

/// Claude's reply as plain text: the markdown marks it likes to add are dropped.
fn plain(text: &str) -> String {
    text.replace("**", "")
        .replace('`', "")
        .lines()
        .map(|l| l.strip_prefix("- ").map_or_else(|| l.to_string(), |r| format!("• {r}")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_label(name: &str) -> String {
    match name {
        "type_text" | "execute" | "batch" | "select_text" | "new_document" | "open_document" | "save_document" => "Editando o documento".to_string(),
        "get_text" | "inspect_document" | "list_documents" => "Lendo o documento".to_string(),
        "render_page" => "Olhando a página".to_string(),
        "list_commands" => "Procurando comandos".to_string(),
        other => other.to_string(),
    }
}

fn bubble(ui: &mut egui::Ui, t: &Tokens, line: &Line) {
    let width = ui.available_width();
    match line {
        Line::You(text) => {
            ui.with_layout(egui::Layout::right_to_left(Align::TOP), |ui| {
                egui::Frame::NONE.fill(t.accent).corner_radius(12).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_max_width(width * 0.86);
                    ui.add(egui::Label::new(RichText::new(text).color(t.on_accent).font(regular(13.0))).selectable(true));
                });
            });
        }
        Line::Claude(text) => {
            ui.with_layout(egui::Layout::left_to_right(Align::TOP), |ui| {
                egui::Frame::NONE
                    .fill(t.ribbon)
                    .stroke(Stroke::new(1.0, t.border))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(width * 0.92);
                        ui.add(egui::Label::new(RichText::new(plain(text)).color(t.text).font(regular(13.0))).selectable(true));
                    });
            });
        }
        Line::Tool(name) => {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 2.5, t.accent);
                ui.label(RichText::new(tool_label(name)).color(t.text_dim).font(regular(11.5)));
            });
        }
        Line::Note(text) => {
            egui::Frame::NONE
                .fill(t.red.gamma_multiply(0.12))
                .stroke(Stroke::new(1.0, t.red))
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.add(egui::Label::new(RichText::new(text).color(t.text).font(regular(12.5))).selectable(true));
                });
        }
    }
}

/// What to try, shown while the conversation is empty.
fn suggestions() -> [&'static str; 4] {
    ["Corrigir a ortografia e a gramática", "Deixar o tom mais formal", "Resumir em três tópicos", "Padronizar a formatação dos títulos"]
}

fn empty_state(ui: &mut egui::Ui, t: &Tokens, act: &mut Act, running: bool) {
    ui.add_space(28.0);
    ui.vertical_centered(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 22.0, t.accent);
        icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(24.0, 24.0)), "claude", t.on_accent, t.on_accent);
        ui.add_space(10.0);
        ui.label(RichText::new("Como posso ajudar com este documento?").font(semibold(15.0)).color(t.text));
        ui.add_space(2.0);
        ui.label(
            RichText::new("Posso escrever, reescrever, formatar e corrigir o texto aberto. Você pode desfazer qualquer mudança com Ctrl+Z.")
                .font(regular(12.0))
                .color(t.text_dim),
        );
    });
    ui.add_space(14.0);
    for s in suggestions() {
        let btn = egui::Button::new(RichText::new(s).font(regular(12.5)).color(t.text))
            .fill(t.input)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(10)
            .min_size(vec2(ui.available_width(), 32.0));
        if ui.add_enabled(!running, btn).clicked() {
            act.send = Some(s.to_string());
        }
        ui.add_space(2.0);
    }
}

/// One drop-down of the choices row: a small label and the list.
fn choice(ui: &mut egui::Ui, t: &Tokens, id: &str, label: &str, tip: &str, current: &mut String, options: &[(&'static str, String)]) {
    let value = options.iter().find(|(v, _)| *v == current.as_str()).map_or("", |(v, _)| *v);
    let shown = options.iter().find(|(v, _)| *v == value).map_or("Padrão", |(_, n)| n.as_str());
    ui.label(RichText::new(label).font(regular(11.5)).color(t.text_dim));
    let r = craft_ui::ComboBox::from_id_salt(id).selected_text(shown).width(112.0).show_ui(ui, |ui| {
        for (v, name) in options {
            if ui.selectable_label(value == *v, name.as_str()).clicked() {
                *current = (*v).to_string();
            }
        }
    });
    r.response.on_hover_text(tip);
}

/// The model and the effort Claude uses for the next message.
fn choices(ui: &mut egui::Ui, app: &mut WordApp, t: &Tokens) {
    let models: Vec<(&'static str, String)> =
        MODELS.iter().map(|(v, name)| (*v, if v.is_empty() { name.to_string() } else { choice_name(&app.ui.claude_models, v, name) })).collect();
    let efforts: Vec<(&'static str, String)> = EFFORTS.iter().map(|(v, name)| (*v, name.to_string())).collect();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        choice(
            ui,
            t,
            "claude-model",
            "Modelo",
            "O modelo do Claude que responde. A lista mostra sempre a versão mais nova que o Claude Code usa.",
            &mut app.ui.claude_model,
            &models,
        );
        ui.add_space(6.0);
        choice(
            ui,
            t,
            "claude-effort",
            "Esforço",
            "Quanto o Claude pensa antes de responder. Mais esforço dá respostas melhores, mais lentas, e gasta mais do seu plano.",
            &mut app.ui.claude_effort,
            &efforts,
        );
    });
    ui.add_space(6.0);
}

/// The text box with its send (or stop) button.
fn composer(ui: &mut egui::Ui, app: &mut WordApp, t: &Tokens, act: &mut Act) {
    choices(ui, app, t);
    let running = app.claude.running();
    let id = egui::Id::unique("claude-input");
    let focused = ui.memory(|m| m.has_focus(id));
    let enter = focused && !running && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
    if enter {
        // Enter sends; keep it out of the text. Shift+Enter still makes a new line.
        ui.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, .. })));
    }
    let stroke = Stroke::new(1.0, if focused { t.accent } else { t.input_border });
    egui::Frame::NONE.fill(t.input).stroke(stroke).corner_radius(12).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        let edit = ui.add(
            TextEdit::multiline(&mut app.claude.input)
                .id(id)
                .frame(egui::Frame::NONE)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .font(regular(13.0))
                .hint_text("Peça ao Claude para alterar o documento…"),
        );
        if app.claude.focus_input {
            edit.request_focus();
            app.claude.focus_input = false;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Enter envia · Shift+Enter quebra a linha").font(regular(10.5)).color(t.text_dim));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let can = running || !app.claude.input.trim().is_empty();
                let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                let fill = if !can {
                    t.border
                } else if resp.hovered() {
                    t.accent_text
                } else {
                    t.accent
                };
                ui.painter().circle_filled(r.center(), 14.0, fill);
                let c = r.center();
                if running {
                    ui.painter().rect_filled(Rect::from_center_size(c, vec2(9.0, 9.0)), 2.0, t.on_accent);
                } else {
                    let st = Stroke::new(1.6, t.on_accent);
                    ui.painter().line_segment([c + vec2(0.0, 5.0), c + vec2(0.0, -5.0)], st);
                    ui.painter().line_segment([c + vec2(-4.5, -0.5), c + vec2(0.0, -5.0)], st);
                    ui.painter().line_segment([c + vec2(4.5, -0.5), c + vec2(0.0, -5.0)], st);
                }
                let tip = if running { "Parar" } else { "Enviar" };
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, can, tip));
                if resp.on_hover_text(tip).clicked() && can {
                    if running {
                        act.stop = true;
                    } else {
                        act.send = Some(app.claude.input.trim().to_string());
                    }
                }
            });
        });
        if enter && !app.claude.input.trim().is_empty() {
            act.send = Some(app.claude.input.trim().to_string());
        }
    });
}

/// The Claude panel, docked on the right of the document (so the page narrows instead of being
/// covered), resizable, and out of the way until the title bar's Claude button opens it.
pub(crate) fn dock(app: &mut WordApp, ui: &mut egui::Ui, t: Tokens) {
    if !app.claude.open {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if !app.claude.probed {
        app.claude.probed = true;
        app.claude.probe = Some(native::probe(ui.ctx().clone()));
    }
    let mut act = Act::default();
    let title = format!("{} · {}", current_model_label(app), app.title_stem());
    egui::Panel::right("claude-dock")
        .resizable(true)
        .default_size(360.0)
        .size_range(300.0..=640.0)
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            // Header.
            egui::Frame::NONE.inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 14.0, t.accent);
                    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), "claude", t.on_accent, t.on_accent);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new("Claude").font(semibold(14.0)).color(t.text));
                        ui.add(egui::Label::new(RichText::new(title).font(regular(11.0)).color(t.text_dim)).truncate());
                    });
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, &t, "close", "Fechar") {
                            act.close = true;
                        }
                        if icon_button(ui, &t, "new", "Nova conversa") {
                            act.new_chat = true;
                        }
                    });
                });
            });
            ui.separator();

            // Message box at the bottom, conversation above it.
            egui::Panel::bottom("claude-composer")
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(12, 10)))
                .show(ui, |ui| composer(ui, app, &t, &mut act));
            let running = app.claude.running();
            ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                egui::Frame::NONE.inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    if app.claude.lines.is_empty() {
                        empty_state(ui, &t, &mut act, running);
                    }
                    for line in &app.claude.lines {
                        bubble(ui, &t, line);
                    }
                    if running {
                        ui.horizontal(|ui| {
                            ui.add_space(6.0);
                            ui.spinner();
                            ui.label(RichText::new("Claude está trabalhando…").font(regular(12.0)).color(t.text_dim));
                        });
                    }
                });
            });
        });
    if act.close {
        app.claude.open = false;
    }
    if act.stop {
        app.claude.stop();
    }
    if act.new_chat {
        app.claude.new_chat();
    }
    if let Some(prompt) = act.send {
        app.claude.input.clear();
        if !prompt.is_empty() && !app.claude.running() {
            send_prompt(app, ui.ctx(), prompt);
        }
    }
}

/// The text added to Claude's instructions: where it is and how to work.
const SYSTEM: &str = "You are inside WordCraft, a word processor, helping the person with the document open in its window. \
Use the wordcraft tools: get_text or inspect_document to read the document, execute or batch to change it \
(list_commands finds command ids), type_text to write at the caret, render_page to look at a page. Make the \
change the person asks for, then say in a sentence or two what you did. Always answer in Brazilian Portuguese (pt-BR), whatever language the document is in, unless the person asks for another language. \
Do not save, close or open documents unless asked.";

fn send_prompt(app: &mut WordApp, ctx: &Context, prompt: String) {
    app.claude.lines.push(Line::You(prompt.clone()));
    match start(app, ctx, prompt) {
        Ok(()) => {}
        Err(e) => app.claude.lines.push(Line::Note(e)),
    }
}

#[cfg(target_arch = "wasm32")]
fn start(_app: &mut WordApp, _ctx: &Context, _prompt: String) -> Result<(), String> {
    Err("O Claude só funciona no aplicativo para computador: o navegador não consegue iniciá-lo.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn start(app: &mut WordApp, ctx: &Context, prompt: String) -> Result<(), String> {
    let port = match app.control_port {
        Some(p) => p,
        None => {
            let hook = app
                .services
                .claude_bridge
                .as_ref()
                .ok_or_else(|| "Esta versão não consegue abrir o canal de controle de que o Claude precisa.".to_string())?;
            let (rx, port) = hook(ctx)?;
            app.control_rx = Some(rx);
            app.control_port = Some(port);
            port
        }
    };
    let (rx, handle) =
        native::spawn(port, prompt, app.claude.session.clone(), option_args(&app.ui.claude_model, &app.ui.claude_effort), ctx.clone())?;
    app.claude.rx = Some(rx);
    app.claude.child = Some(handle);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod native {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};

    use super::{Event, SYSTEM};

    /// A running `claude` process, to stop it.
    pub(crate) struct Handle(Arc<Mutex<Child>>);

    impl Handle {
        pub(crate) fn kill(&self) {
            if let Ok(mut c) = self.0.lock() {
                let _ = c.kill();
            }
        }
    }

    fn exe_names(base: &str) -> Vec<String> {
        if cfg!(windows) { vec![format!("{base}.exe"), format!("{base}.cmd")] } else { vec![base.to_string()] }
    }

    fn on_path(base: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        for dir in std::env::split_paths(&path) {
            for name in exe_names(base) {
                let p = dir.join(&name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }

    fn newest(paths: Vec<PathBuf>) -> Option<PathBuf> {
        paths.into_iter().filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p))).max().map(|(_, p)| p)
    }

    fn subdirs(p: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(p).map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default()
    }

    /// Where Claude Code is installed: `WORDCRAFT_CLAUDE`, the PATH, the native installer's folder,
    /// or the copy the Claude desktop app keeps.
    pub(crate) fn find_claude() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("WORDCRAFT_CLAUDE").map(PathBuf::from).filter(|p| p.is_file()) {
            return Some(p);
        }
        if let Some(p) = on_path("claude") {
            return Some(p);
        }
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        if let Some(home) = &home {
            for name in exe_names("claude") {
                let p = home.join(".local").join("bin").join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        // The desktop app's own copy: <data>/<app>/claude-code/<version>/<build>/claude(.exe).
        let data = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Claude-Data"))
        } else if cfg!(target_os = "macos") {
            home.as_ref().map(|h| h.join("Library").join("Application Support").join("Claude"))
        } else {
            home.as_ref().map(|h| h.join(".config").join("Claude"))
        }?;
        let mut found = Vec::new();
        let roots = if cfg!(windows) { subdirs(&data) } else { vec![data] };
        for root in roots {
            for version in subdirs(&root.join("claude-code")) {
                for build in subdirs(&version) {
                    for name in exe_names("claude") {
                        let p = build.join(name);
                        if p.is_file() {
                            found.push(p);
                        }
                    }
                }
            }
        }
        newest(found)
    }

    /// `wordcraft-cli`, next to this program or on the PATH.
    pub(crate) fn find_cli() -> Option<PathBuf> {
        if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
            for name in exe_names("wordcraft-cli") {
                let p = dir.join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        on_path("wordcraft-cli")
    }

    /// The MCP configuration that points Claude at this window.
    pub(crate) fn mcp_config(cli: &Path, port: u16) -> Value {
        json!({"mcpServers": {"wordcraft": {
            "command": cli.to_string_lossy(),
            "args": ["mcp", "--connect", format!("127.0.0.1:{port}")],
        }}})
    }

    /// What one line of `claude --output-format stream-json` says.
    pub(crate) fn events_of(line: &str) -> Vec<Event> {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
        let mut out = Vec::new();
        if let Some(id) = v.get("session_id").and_then(Value::as_str) {
            out.push(Event::Session(id.to_string()));
        }
        match v.get("type").and_then(Value::as_str) {
            Some("system") if v.get("subtype").and_then(Value::as_str) == Some("init") => {
                out.extend(v.get("model").and_then(Value::as_str).map(|m| Event::Model(m.to_string())));
            }
            Some("assistant") => {
                let parts = v.pointer("/message/content").and_then(Value::as_array).cloned().unwrap_or_default();
                for p in parts {
                    match p.get("type").and_then(Value::as_str) {
                        Some("text") => out.extend(p.get("text").and_then(Value::as_str).map(|t| Event::Text(t.to_string()))),
                        Some("tool_use") => {
                            let name = p.get("name").and_then(Value::as_str).unwrap_or("tool");
                            out.push(Event::Tool(name.rsplit("__").next().unwrap_or(name).to_string()));
                        }
                        _ => {}
                    }
                }
            }
            Some("result") if v.get("is_error").and_then(Value::as_bool) == Some(true) => {
                let why = v.get("result").and_then(Value::as_str).unwrap_or("Claude stopped with an error.");
                out.push(Event::Failed(why.to_string()));
            }
            _ => {}
        }
        out
    }

    /// Start Claude on `prompt`, wired to the control channel on `port`.
    pub(crate) fn spawn(
        port: u16,
        prompt: String,
        session: Option<String>,
        options: Vec<String>,
        ctx: egui::Context,
    ) -> Result<(Receiver<Event>, Handle), String> {
        let claude = find_claude().ok_or_else(|| {
            "Claude Code não encontrado. Instale-o (claude.com/code) e entre na sua conta, ou defina WORDCRAFT_CLAUDE com o caminho dele.".to_string()
        })?;
        let cli = find_cli().ok_or_else(|| "wordcraft-cli não foi encontrado ao lado do WordCraft nem no PATH.".to_string())?;
        let dir = std::env::temp_dir();
        let cfg = dir.join(format!("wordcraft-claude-{}.json", std::process::id()));
        std::fs::write(&cfg, mcp_config(&cli, port).to_string()).map_err(|e| format!("{}: {e}", cfg.display()))?;

        let mut cmd = Command::new(&claude);
        cmd.current_dir(&dir)
            .args(["-p", "--output-format", "stream-json", "--verbose"])
            .arg("--mcp-config")
            .arg(&cfg)
            .arg("--strict-mcp-config")
            // The person's own skills and plugins are not for this panel, and loading them adds
            // hundreds of thousands of tokens to every conversation (their plan's limit).
            .arg("--disable-slash-commands")
            .args(["--allowedTools", "mcp__wordcraft"])
            .args(["--tools", ""])
            .args(["--append-system-prompt", SYSTEM])
            .args(&options);
        if let Some(id) = session {
            cmd.args(["--resume", &id]);
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
        let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", claude.display()))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes());
        }
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child = Arc::new(Mutex::new(child));
        let (tx, rx) = channel::<Event>();
        let worker = child.clone();
        std::thread::spawn(move || run(worker, stdout, stderr, tx, ctx));
        Ok((rx, Handle(child)))
    }

    /// Ask Claude Code which model each choice is today (`opus` is whatever Opus is newest), so
    /// the list can name the version. It reports its model as it starts, before any request is made,
    /// so this costs nothing from the plan. Answers arrive on the returned channel.
    pub(crate) fn probe(ctx: egui::Context) -> Receiver<(String, String)> {
        let (tx, rx) = channel();
        if let Some(claude) = find_claude() {
            for &(alias, _) in super::MODELS.iter().filter(|(a, _)| !a.is_empty()) {
                let (tx, claude, ctx) = (tx.clone(), claude.clone(), ctx.clone());
                std::thread::spawn(move || {
                    if let Some(id) = resolve(&claude, alias) {
                        let _ = tx.send((alias.to_string(), id));
                        ctx.request_repaint();
                    }
                });
            }
        }
        rx
    }

    /// Start Claude Code on `alias`, read the model it announces, and stop it.
    fn resolve(claude: &Path, alias: &str) -> Option<String> {
        let mut cmd = Command::new(claude);
        cmd.current_dir(std::env::temp_dir())
            .args(["-p", "--output-format", "stream-json", "--verbose", "--no-session-persistence"])
            .args(["--tools", "", "--disable-slash-commands", "--strict-mcp-config", "--model", alias])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
        let mut child = cmd.spawn().ok()?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(b".");
        }
        let mut found = None;
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok).take(40) {
                if let Some(Event::Model(m)) = events_of(&line).into_iter().find(|e| matches!(e, Event::Model(_))) {
                    found = Some(m);
                    break;
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        found
    }

    fn run(child: Arc<Mutex<Child>>, stdout: Option<impl Read>, stderr: Option<impl Read + Send + 'static>, tx: Sender<Event>, ctx: egui::Context) {
        let err_text = stderr.map(|mut s| {
            std::thread::spawn(move || {
                let mut t = String::new();
                let _ = s.read_to_string(&mut t);
                t
            })
        });
        let mut failed = false;
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                for ev in events_of(&line) {
                    failed |= matches!(ev, Event::Failed(_));
                    if tx.send(ev).is_err() {
                        return;
                    }
                }
                ctx.request_repaint();
            }
        }
        let status = child.lock().ok().and_then(|mut c| c.wait().ok());
        let stderr = err_text.and_then(|h| h.join().ok()).unwrap_or_default();
        if !failed && status.is_some_and(|s| !s.success()) {
            let why = stderr.trim();
            let _ = tx.send(Event::Failed(if why.is_empty() { "O Claude parou antes de responder.".to_string() } else { why.to_string() }));
        } else if !failed {
            let _ = tx.send(Event::Done);
        }
        ctx.request_repaint();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::native::{events_of, mcp_config};
    use super::*;

    #[test]
    fn an_assistant_line_gives_its_text_and_the_tools_it_used() {
        let line = r#"{"type":"assistant","session_id":"s1","message":{"content":[
            {"type":"text","text":"Done."},
            {"type":"tool_use","name":"mcp__wordcraft__execute","input":{}}]}}"#
            .replace('\n', "");
        let evs = events_of(&line);
        assert!(matches!(&evs[0], Event::Session(s) if s == "s1"));
        assert!(matches!(&evs[1], Event::Text(t) if t == "Done."));
        assert!(matches!(&evs[2], Event::Tool(t) if t == "execute"));
    }

    #[test]
    fn an_error_result_fails_the_turn_and_junk_is_ignored() {
        let evs = events_of(r#"{"type":"result","is_error":true,"result":"Not logged in"}"#);
        assert!(matches!(&evs[..], [Event::Failed(m)] if m == "Not logged in"));
        assert!(events_of("not json").is_empty());
        assert!(events_of("{}").is_empty());
        assert!(events_of(r#"{"type":"assistant","message":{"content":7}}"#).is_empty());
    }

    #[test]
    fn only_offered_models_and_efforts_reach_the_command_line() {
        assert!(option_args("", "").is_empty(), "the default adds nothing");
        assert_eq!(option_args("opus", "high"), ["--model", "opus", "--effort", "high"]);
        assert_eq!(option_args("sonnet", ""), ["--model", "sonnet"]);
        // A saved setting or a command can hold anything: nothing unknown is passed on.
        assert!(option_args("--dangerously-skip-permissions", "x; rm").is_empty());
        assert_eq!(option_args("OPUS", "max"), ["--effort", "max"], "names are exact");
    }

    #[test]
    fn the_command_sets_the_choices_and_refuses_unknown_ones() {
        let mut app = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
        let r = app.run("ui.claude", serde_json::json!({"value": false, "model": "haiku", "effort": "low"})).unwrap();
        assert_eq!((r["model"].as_str(), r["effort"].as_str()), (Some("haiku"), Some("low")));
        assert_eq!(app.ui.claude_model, "haiku");
        assert!(app.run("ui.claude", serde_json::json!({"model": "gpt"})).is_err());
        assert!(app.run("ui.claude", serde_json::json!({"effort": "extreme"})).is_err());
        assert_eq!(app.ui.claude_model, "haiku", "a refused value changes nothing");
        // Back to the default.
        app.run("ui.claude", serde_json::json!({"value": false, "model": "", "effort": ""})).unwrap();
        assert!(option_args(&app.ui.claude_model, &app.ui.claude_effort).is_empty());
    }

    #[test]
    fn a_model_id_reads_as_its_family_and_version() {
        assert_eq!(model_label("claude-opus-5-5").as_deref(), Some("Opus 5.5"));
        assert_eq!(model_label("claude-fable-5-2").as_deref(), Some("Fable 5.2"));
        assert_eq!(model_label("claude-haiku-4-5-20251001").as_deref(), Some("Haiku 4.5"));
        assert_eq!(model_label("claude-sonnet-4").as_deref(), Some("Sonnet 4"));
        assert_eq!(model_label("claude-opus").as_deref(), Some("Opus"));
        for junk in ["", "gpt-5", "claude-", "claude-3-5-sonnet", "claude-Opus-5", "claude-opus-5-5; rm", &"claude-opus-5-5".repeat(5)] {
            assert_eq!(model_label(junk), None, "{junk:?}");
        }
    }

    #[test]
    fn the_list_names_the_version_claude_code_reports() {
        let mut app = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
        // First guess: the 5.5 family.
        assert_eq!(choice_name(&app.ui.claude_models, "opus", "Opus"), "Opus 5.5");
        assert_eq!(choice_name(&app.ui.claude_models, "fable", "Fable"), "Fable 5.2");
        // A newer Opus appears: the next probe, or the next conversation, renames the choice.
        set_resolved(&mut app, "opus", "claude-opus-6");
        assert_eq!(choice_name(&app.ui.claude_models, "opus", "Opus"), "Opus 6");
        note_model(&mut app, "claude-sonnet-5-6");
        assert_eq!(choice_name(&app.ui.claude_models, "sonnet", "Sonnet"), "Sonnet 5.6");
        assert_eq!(app.claude.last_model.as_deref(), Some("claude-sonnet-5-6"));
        // What cannot be a model, or is not one of the choices, is ignored.
        set_resolved(&mut app, "opus", "not a model");
        set_resolved(&mut app, "--x", "claude-opus-7");
        assert_eq!(choice_name(&app.ui.claude_models, "opus", "Opus"), "Opus 6");
        assert!(!app.ui.claude_models.contains_key("--x"));
        // The header says what the next message uses.
        app.ui.claude_model = "opus".into();
        assert_eq!(current_model_label(&app), "Opus 6");
        app.ui.claude_model = String::new();
        assert_eq!(current_model_label(&app), "Padrão (Sonnet 5.6)");
    }

    #[test]
    fn the_start_of_a_conversation_names_the_model() {
        let line = r#"{"type":"system","subtype":"init","session_id":"s","model":"claude-opus-5-5"}"#;
        let evs = events_of(line);
        assert!(evs.iter().any(|e| matches!(e, Event::Model(m) if m == "claude-opus-5-5")));
    }

    #[test]
    fn the_mcp_config_connects_to_the_given_port() {
        let c = mcp_config(std::path::Path::new("wordcraft-cli"), 4242);
        assert_eq!(c["mcpServers"]["wordcraft"]["args"], serde_json::json!(["mcp", "--connect", "127.0.0.1:4242"]));
    }

    #[test]
    fn events_update_the_conversation_and_end_the_run() {
        let mut chat = Chat::default();
        let (_tx, rx) = std::sync::mpsc::channel();
        chat.rx = Some(rx);
        chat.apply(Event::Session("abc".into()));
        chat.apply(Event::Text("  Hello  ".into()));
        chat.apply(Event::Text("   ".into()));
        chat.apply(Event::Done);
        assert_eq!(chat.session.as_deref(), Some("abc"));
        assert_eq!(chat.lines, vec![Line::Claude("Hello".into())]);
        assert!(!chat.running());
    }
}
