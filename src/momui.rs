//! The MyOpenMath screen: describe a question, the AI writes it, the engine tests it at three
//! random versions, and the teacher previews it, edits it, saves it and copies it into MyOpenMath.
//!
//! Left: the request, live progress, and saved questions. Right: the question, with Preview
//! (the engine's interactive page: in the browser build an iframe over the canvas, on the desktop
//! a page opened in the system browser), Code (editable, re-testable) and Copy to MyOpenMath (one
//! copy button per editor field, and the bookmarklet that fills the editor in the teacher's own tab).
//! Saving always runs the self-test first, so the bank only ever records a tested verdict.

use crate::mom::author::{author_async_logged, post_engine, HttpEngine, ModelChat, AuthorEngine};
use crate::mom::engine::{SelftestResponse, DEFAULT_SEEDS};
use crate::mom::MomQuestion;
use crate::pageshell;
use crate::theme;
use egui::{RichText, Ui};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

pub const DEFAULT_ENGINE: &str = "http://127.0.0.1:8791";
const LIBRARY: &str = "mom/library.json";
const MAX_TRIES: usize = 4;

/// One saved question.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Saved {
    pub id: String,
    pub request: String,
    /// The mom file.
    pub src: String,
    /// The self-test verdict when it was saved.
    pub passed: bool,
    #[serde(default)]
    pub problems: Vec<String>,
    /// Approved after a review in the sandbox.
    #[serde(default)]
    pub reviewed: bool,
    /// Seconds since 1970.
    pub saved_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct Library {
    #[serde(default)]
    questions: Vec<Saved>,
}

/// The question on the right.
#[derive(Clone, Default)]
struct Current {
    q: MomQuestion,
    request: String,
    /// None: not tested since the last edit.
    passed: Option<bool>,
    problems: Vec<String>,
    untested: Vec<usize>,
    /// The saved entry this came from or was saved as.
    saved_id: Option<String>,
    /// The teacher has had the sandbox open for this version of the question.
    sandbox_seen: bool,
    /// The teacher tried it in the sandbox and approved it; copying into MyOpenMath is locked until then.
    reviewed: bool,
}

enum Outcome {
    Authored { q: Option<MomQuestion>, passed: bool, problems: Vec<String>, untested: Vec<usize>, error: Option<String> },
    Tested(Result<SelftestResponse, String>, /* then save */ bool),
    Preview(Result<String, String>),
    TestedAll(Vec<(String, Result<SelftestResponse, String>)>),
}

#[derive(Default)]
struct Job {
    busy: bool,
    what: &'static str,
    log: Vec<String>,
    /// Finished results not yet taken by the screen. A list, so a job starting on the frame another
    /// finished cannot drop the first one's result.
    outcomes: Vec<Outcome>,
}

#[derive(PartialEq, Clone, Copy, Default)]
enum Tab { #[default] Preview, Code, Copy }

#[derive(Default)]
struct State {
    inited: bool,
    request: String,
    engine: String,
    job: Arc<Mutex<Job>>,
    log: Vec<String>,
    error: Option<String>,
    note: Option<String>,
    current: Option<Current>,
    tab: Tab,
    preview_html: Option<String>,
    preview_for: String,
    library: Library,
    confirm_delete: Option<String>,
}

thread_local! {
    static ST: RefCell<State> = RefCell::new(State::default());
}

/// Called every frame from the screen router; hides the browser build's preview when another
/// screen is showing.
pub fn leave() {
    #[cfg(target_arch = "wasm32")]
    overlay::hide();
}

impl crate::App {
    pub fn draw_mom(&mut self, ctx: &egui::Context) {
        let mut st = ST.with(|s| std::mem::take(&mut *s.borrow_mut()));
        if !st.inited {
            st.inited = true;
            st.engine = crate::store::load_json::<String>("mom/engine.json").unwrap_or_else(|| DEFAULT_ENGINE.to_string());
            st.library = crate::store::load_json(LIBRARY).unwrap_or_default();
            // Screenshot/test hook: DAD_MOM_REQUEST pre-fills the request box.
            if let Ok(r) = crate::env_var("DAD_MOM_REQUEST") { st.request = r; }
            // Screenshot/test hook: DAD_MOM_OPEN=<file.php> opens a question and self-tests it;
            // DAD_MOM_TAB=code|copy picks the tab.
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(src) = crate::env_var("DAD_MOM_OPEN").ok().and_then(|p| std::fs::read_to_string(p).ok()) {
                st.current = Some(Current { q: MomQuestion::parse(&src), request: st.request.clone(), ..Default::default() });
                st.tab = match crate::env_var("DAD_MOM_TAB").as_deref() { Ok("code") => Tab::Code, Ok("copy" | "copy-approved") => Tab::Copy, _ => Tab::Preview };
                if crate::env_var("DAD_MOM_TAB").as_deref() == Ok("copy-approved") {
                    if let Some(c) = st.current.as_mut() { c.reviewed = true; }
                }
                start_test(&mut st, ctx, false);
            }
        }
        collect(&mut st);
        let cfg = crate::ai::AiCfg::from(&self.settings);
        crate::screens::shell_frame(self, ctx, true, |_app, ui| page(ui, ctx, &mut st, &cfg));
        ST.with(|s| *s.borrow_mut() = st);
    }
}

/// Take a finished job's result into the screen state.
fn collect(st: &mut State) {
    let (log, outcomes) = {
        let Ok(mut j) = st.job.lock() else { return };
        (std::mem::take(&mut j.log), std::mem::take(&mut j.outcomes))
    };
    st.log.extend(log);
    for outcome in outcomes { take(st, outcome); }
}

fn take(st: &mut State, outcome: Outcome) {
    match outcome {
        Outcome::Authored { q, passed, problems, untested, error } => {
            st.error = error;
            if let Some(q) = q {
                st.current = Some(Current { q, request: st.request.clone(), passed: Some(passed), problems, untested, ..Default::default() });
                st.tab = Tab::Preview;
                st.preview_html = None;
            }
        }
        Outcome::Tested(r, then_save) => match r {
            Ok(rep) => {
                if let Some(c) = st.current.as_mut() {
                    c.passed = Some(rep.pass);
                    c.problems = rep.problems();
                    c.untested = rep.untested_parts.clone();
                }
                st.error = None;
                if then_save { save_current(st); }
            }
            Err(e) => st.error = Some(e),
        },
        Outcome::Preview(r) => match r {
            Ok(html) => st.preview_html = Some(html),
            Err(e) => st.error = Some(e),
        },
        Outcome::TestedAll(rs) => {
            let mut changed = 0;
            for (id, r) in rs {
                if let (Ok(rep), Some(s)) = (r, st.library.questions.iter_mut().find(|s| s.id == id)) {
                    if s.passed != rep.pass { changed += 1; }
                    s.passed = rep.pass;
                    s.problems = rep.problems();
                }
            }
            let failing = st.library.questions.iter().filter(|s| !s.passed).count();
            st.note = Some(format!("Tested {} saved questions: {} need fixes{}.", st.library.questions.len(), failing,
                if changed > 0 { format!(", {changed} changed since they were saved") } else { String::new() }));
            let _ = crate::store::save_json(LIBRARY, &st.library);
        }
    }
}

fn busy(st: &State) -> bool {
    st.job.lock().map(|j| j.busy).unwrap_or(false)
}

/// Run `f` in the background: a thread on the desktop, a task in the browser.
#[cfg(not(target_arch = "wasm32"))]
fn run_bg(f: impl std::future::Future<Output = ()> + Send + 'static) {
    std::thread::spawn(move || crate::mom::author::block_on_ready(f));
}
#[cfg(target_arch = "wasm32")]
fn run_bg(f: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(f);
}

fn start(st: &mut State, what: &'static str) -> Arc<Mutex<Job>> {
    if let Ok(mut j) = st.job.lock() {
        j.busy = true;
        j.what = what;
        j.log.clear();
    }
    st.error = None;
    st.note = None;
    st.job.clone()
}

fn finish(job: &Arc<Mutex<Job>>, ctx: &egui::Context, outcome: Outcome) {
    if let Ok(mut j) = job.lock() {
        j.busy = false;
        j.outcomes.push(outcome);
    }
    ctx.request_repaint();
}

fn start_write(st: &mut State, ctx: &egui::Context, cfg: &crate::ai::AiCfg) {
    st.log.clear();
    let job = start(st, "Writing");
    let (request, engine, cfg, ctx) = (st.request.trim().to_string(), st.engine.clone(), cfg.clone(), ctx.clone());
    run_bg(async move {
        let mut chat = ModelChat { cfg: &cfg };
        let mut eng = HttpEngine { base: engine };
        let log_job = job.clone();
        let log_ctx = ctx.clone();
        let run = author_async_logged(&request, &mut chat, &mut eng, MAX_TRIES, move |line| {
            if let Ok(mut j) = log_job.lock() { j.log.push(line.to_string()); }
            log_ctx.request_repaint();
        }).await;
        let untested = run.report.as_ref().map(|r| r.untested_parts.clone()).unwrap_or_default();
        finish(&job, &ctx, Outcome::Authored { q: run.question, passed: run.passed, problems: run.problems, untested, error: run.error });
    });
}

fn start_test(st: &mut State, ctx: &egui::Context, then_save: bool) {
    let Some(c) = &st.current else { return };
    let src = c.q.emit();
    let job = start(st, if then_save { "Testing and saving" } else { "Testing" });
    let (engine, ctx) = (st.engine.clone(), ctx.clone());
    run_bg(async move {
        let r = HttpEngine { base: engine }.selftest(&src, &DEFAULT_SEEDS).await;
        finish(&job, &ctx, Outcome::Tested(r, then_save));
    });
}

fn start_preview(st: &mut State, ctx: &egui::Context) {
    let Some(c) = &st.current else { return };
    let src = c.q.emit();
    st.preview_for = src.clone();
    let job = start(st, "Drawing the preview");
    let (engine, ctx) = (st.engine.clone(), ctx.clone());
    run_bg(async move {
        let body = serde_json::json!({"src": src, "seed": DEFAULT_SEEDS[0]});
        let r = post_engine(&engine, "/api/preview", &body).await
            .and_then(|v| v.get("html").and_then(|h| h.as_str()).map(str::to_string).ok_or_else(|| "The engine sent no preview.".to_string()));
        finish(&job, &ctx, Outcome::Preview(r));
    });
}

fn start_test_all(st: &mut State, ctx: &egui::Context) {
    let items: Vec<(String, String)> = st.library.questions.iter().map(|s| (s.id.clone(), s.src.clone())).collect();
    let job = start(st, "Testing saved questions");
    let (engine, ctx) = (st.engine.clone(), ctx.clone());
    run_bg(async move {
        let mut out = Vec::new();
        for (id, src) in items {
            let r = HttpEngine { base: engine.clone() }.selftest(&src, &DEFAULT_SEEDS).await;
            out.push((id, r));
        }
        finish(&job, &ctx, Outcome::TestedAll(out));
    });
}

fn save_current(st: &mut State) {
    let Some(c) = st.current.as_mut() else { return };
    let now = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let entry = Saved {
        id: c.saved_id.clone().unwrap_or_else(crate::model::new_id),
        request: c.request.clone(),
        src: c.q.emit(),
        passed: c.passed == Some(true),
        problems: c.problems.clone(),
        reviewed: c.reviewed,
        saved_at: now,
    };
    c.saved_id = Some(entry.id.clone());
    match st.library.questions.iter_mut().find(|s| s.id == entry.id) {
        Some(s) => *s = entry,
        None => st.library.questions.insert(0, entry),
    }
    st.note = Some(match crate::store::save_json(LIBRARY, &st.library) {
        Ok(()) => if c.passed == Some(true) { "Saved. It passed the self-test.".into() } else { "Saved, marked as needing fixes.".into() },
        Err(e) => format!("Could not save: {e}"),
    });
}

/// A file name for the question: its description, lowercased and dashed.
fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).take(8).collect::<Vec<_>>().join("-");
    if s.is_empty() { "question".into() } else { s }
}

// ---------- drawing ----------

fn page(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    pageshell::title(ui, "", "MyOpenMath",
        "Describe a question. The AI writes it, IMathAS tests it at three random versions, and you copy it into MyOpenMath.");
    ui.add_space(12.0);
    let wide = ui.available_width() >= 980.0;
    if wide {
        let left_w = 380.0;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(left_w, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().id_salt("mom_left").show(ui, |ui| left(ui, ctx, st, cfg));
            });
            ui.add_space(16.0);
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                right(ui, ctx, st);
            });
        });
    } else {
        egui::ScrollArea::vertical().id_salt("mom_stack").show(ui, |ui| {
            left(ui, ctx, st, cfg);
            ui.add_space(12.0);
            right(ui, ctx, st);
        });
    }
}

fn left(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    let working = busy(st);
    pageshell::card(ui, |ui| {
        ui.label(theme::strong("What should the question ask?").size(15.0));
        ui.add_space(4.0);
        let edit = ui.add(
            egui::TextEdit::multiline(&mut st.request)
                .desired_rows(5)
                .desired_width(f32::INFINITY)
                .hint_text("Integrated Math 1: give two random points; part (a) the slope as a fraction, part (b) is the line increasing or decreasing…"),
        );
        edit.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "What should the question ask?"));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let label = if working { "Writing…" } else { "Write Question" };
            let can = !working && !st.request.trim().is_empty();
            let pressed = ui.add_enabled_ui(can, |ui| pageshell::primary(ui, label)).inner.clicked();
            let ctrl_enter = edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
            if (pressed || ctrl_enter) && can { start_write(st, ctx, cfg); }
            ui.label(theme::muted(format!("{} · {}", cfg.provider, cfg.model)).size(12.0))
                .on_hover_text("The AI comes from Settings → AI. Ctrl+Enter writes the question.");
        });
        if !st.log.is_empty() || working {
            ui.add_space(8.0);
            for line in st.log.iter().rev().take(14).collect::<Vec<_>>().into_iter().rev() {
                let col = if line.contains("passes") { theme::green() } else if line.starts_with("    ") { theme::red() } else { theme::muted_col() };
                ui.add(egui::Label::new(RichText::new(line).size(12.0).color(col)).wrap());
            }
            if working { ui.horizontal(|ui| { ui.spinner(); ui.label(theme::muted(st.job.lock().map(|j| j.what).unwrap_or("Working")).size(12.0)); }); }
        }
        if let Some(e) = &st.error {
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(e).color(theme::red()).size(13.0)).wrap());
        }
    });
    ui.add_space(12.0);
    library(ui, ctx, st);
    ui.add_space(12.0);
    egui::CollapsingHeader::new(theme::muted("Engine").size(13.0)).id_salt("mom_engine").show(ui, |ui| {
        ui.label(theme::muted("The IMathAS engine that tests questions. Start it with engine/run.sh.").size(12.0));
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut st.engine).desired_width(240.0).hint_text(DEFAULT_ENGINE));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Engine address"));
            if r.lost_focus() { let _ = crate::store::save_json("mom/engine.json", &st.engine); }
        });
    });
}

fn library(ui: &mut Ui, ctx: &egui::Context, st: &mut State) {
    pageshell::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::strong("Saved Questions").size(15.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !st.library.questions.is_empty() && ui.add_enabled_ui(!busy(st), |ui| pageshell::outline(ui, "Test All")).inner
                    .on_hover_text("Run every saved question through the engine again").clicked() {
                    start_test_all(st, ctx);
                }
            });
        });
        if st.library.questions.is_empty() {
            ui.label(theme::muted("Nothing saved yet. Questions you save appear here, with whether they passed.").size(13.0));
        }
        let mut open: Option<Saved> = None;
        let mut delete: Option<String> = None;
        for s in &st.library.questions {
            let q = MomQuestion::parse(&s.src);
            ui.horizontal(|ui| {
                let (mark, col, tip) = if s.passed { ("✔", theme::green(), "Passed the self-test") } else { ("✖", theme::red(), "Needs fixes") };
                ui.label(RichText::new(mark).color(col)).on_hover_text(tip);
                let name = if q.name.is_empty() { "Untitled question".to_string() } else { q.name.clone() };
                let short: String = name.chars().take(52).collect::<String>() + if name.chars().count() > 52 { "…" } else { "" };
                if ui.link(short).on_hover_text(&name).clicked() { open = Some(s.clone()); }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if st.confirm_delete.as_deref() == Some(s.id.as_str()) {
                        if ui.button("Cancel").clicked() { st.confirm_delete = None; }
                        if ui.button(RichText::new("Delete").color(theme::red())).clicked() { delete = Some(s.id.clone()); }
                    } else if ui.small_button("✕").on_hover_text("Delete this saved question").clicked() {
                        st.confirm_delete = Some(s.id.clone());
                    }
                });
            });
        }
        if let Some(s) = open {
            let q = MomQuestion::parse(&s.src);
            st.current = Some(Current { q, request: s.request.clone(), passed: Some(s.passed), problems: s.problems.clone(), untested: vec![], saved_id: Some(s.id.clone()), sandbox_seen: false, reviewed: s.reviewed });
            st.request = s.request;
            st.preview_html = None;
            st.tab = Tab::Preview;
        }
        if let Some(id) = delete {
            st.library.questions.retain(|s| s.id != id);
            st.confirm_delete = None;
            if st.current.as_ref().and_then(|c| c.saved_id.as_deref()) == Some(id.as_str()) {
                if let Some(c) = st.current.as_mut() { c.saved_id = None; }
            }
            let _ = crate::store::save_json(LIBRARY, &st.library);
        }
        if let Some(n) = &st.note {
            ui.add_space(4.0);
            ui.label(theme::muted(n).size(12.0));
        }
    });
}

fn right(ui: &mut Ui, ctx: &egui::Context, st: &mut State) {
    let Some(_) = st.current else {
        leave();
        pageshell::card(ui, |ui| {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(theme::serif("No question yet").size(22.0));
                ui.add_space(4.0);
                ui.label(theme::muted("Describe one on the left and press Write Question, or open a saved one.").size(14.0));
            });
            ui.add_space(30.0);
        });
        return;
    };
    let working = busy(st);
    pageshell::card(ui, |ui| {
        let c = st.current.as_ref().unwrap();
        ui.add(egui::Label::new(theme::serif(if c.q.name.is_empty() { "Untitled question" } else { &c.q.name }).size(18.0)).wrap());
        ui.add_space(4.0);
        let (passed, nprob) = (c.passed, c.problems.len());
        ui.horizontal(|ui| {
            match passed {
                Some(true) => { ui.label(RichText::new(format!("✔ Passed at versions {}", DEFAULT_SEEDS.map(|s| s.to_string()).join(", "))).color(theme::green())); }
                Some(false) => { ui.label(RichText::new(format!("✖ {} problem(s) left", nprob.max(1))).color(theme::red())); }
                None if working => { ui.horizontal(|ui| { ui.spinner(); ui.label(theme::muted("Testing at three random versions…")); }); }
                None => { ui.label(theme::muted("Not tested since the last edit: press Run Self-Test on the Code tab.")); }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if st.current.as_ref().and_then(|c| c.saved_id.as_ref()).is_some() { "Save Changes" } else { "Save to Bank" };
                if ui.add_enabled_ui(!working, |ui| pageshell::primary(ui, label)).inner
                    .on_hover_text("Runs the self-test, then saves the question with its verdict").clicked() {
                    start_test(st, ctx, true);
                }
                if ui.add_enabled_ui(!working, |ui| pageshell::outline(ui, "Download .php")).inner.clicked() {
                    let c = st.current.as_ref().unwrap();
                    let name = format!("{}.php", slug(&c.q.name));
                    if let Some(path) = crate::platform::save_path(&name, "MyOpenMath question", &["php"]) {
                        st.note = Some(match crate::platform::write(&path, c.q.emit().as_bytes()) {
                            Ok(()) => format!("Saved {}", path.display()),
                            Err(e) => format!("Could not save the file: {e}"),
                        });
                    }
                }
            });
        });
        let c = st.current.as_ref().unwrap();
        if !c.untested.is_empty() {
            ui.label(theme::muted(format!("Part(s) {} are essay, upload, drawing or picture parts: check them yourself.",
                c.untested.iter().map(|p| (p + 1).to_string()).collect::<Vec<_>>().join(", "))).size(12.0));
        }
        for p in c.problems.iter().take(6) {
            ui.add(egui::Label::new(RichText::new(format!("• {p}")).size(12.0).color(theme::red())).wrap());
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let reviewed = st.current.as_ref().is_some_and(|c| c.reviewed);
            let copy_name = if reviewed { "Copy to MyOpenMath" } else { "Copy to MyOpenMath (review first)" };
            for (t, name) in [(Tab::Preview, "Sandbox"), (Tab::Code, "Code"), (Tab::Copy, copy_name)] {
                if ui.selectable_label(st.tab == t, name).clicked() { st.tab = t; }
            }
        });
        ui.separator();
        match st.tab {
            Tab::Preview => preview_tab(ui, ctx, st, working),
            Tab::Code => { leave(); code_tab(ui, ctx, st, working) }
            Tab::Copy => { leave(); copy_tab(ui, st) }
        }
    });
}

fn preview_tab(ui: &mut Ui, ctx: &egui::Context, st: &mut State, working: bool) {
    let src = st.current.as_ref().map(|c| c.q.emit()).unwrap_or_default();
    let stale = st.preview_html.is_none() || st.preview_for != src;
    if stale && !working && st.error.is_none() {
        start_preview(st, ctx);
    }

    #[cfg(target_arch = "wasm32")]
    {
        // The sandbox is the whole window: a small page laid over part of the canvas is composited
        // badly by some browsers, and a review deserves the room. Its own bar approves or closes it.
        match overlay::take_action() {
            Some(overlay::Action::Approve) => {
                if let Some(c) = st.current.as_mut() { c.reviewed = true; }
                st.tab = Tab::Copy;
                st.note = Some("Approved. Copy it into MyOpenMath.".into());
            }
            Some(overlay::Action::Close) | None => {}
        }
        let ready = st.preview_html.is_some();
        if ui.add_enabled_ui(ready, |ui| pageshell::primary(ui, if ready { "Open the Sandbox" } else { "Preparing the sandbox…" })).inner.clicked() {
            let can_approve = st.current.as_ref().is_some_and(|c| c.passed == Some(true));
            overlay::open(st.preview_html.as_deref().unwrap_or(""), can_approve, ctx);
            if let Some(c) = st.current.as_mut() { c.sandbox_seen = true; }
        }
        ui.add_space(10.0);
        review_bar(ui, st);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {

        let ready = st.preview_html.is_some();
        if ui.add_enabled_ui(ready, |ui| pageshell::primary(ui, if ready { "Open the Sandbox in Your Browser" } else { "Preparing the sandbox…" })).inner.clicked() {
            let path = crate::store::store_dir().join("mom").join("preview.html");
            let html = st.preview_html.clone().unwrap_or_default();
            match crate::platform::write(&path, html.as_bytes()) {
                Ok(()) => {
                    crate::platform::open_path(&path);
                    if let Some(c) = st.current.as_mut() { c.sandbox_seen = true; }
                }
                Err(e) => st.error = Some(format!("Could not write the preview: {e}")),
            }
        }
        ui.add_space(10.0);
        review_bar(ui, st);
        ui.add_space(10.0);
        if let Some(html) = &st.preview_html {
            ui.label(theme::strong(format!("Quick look at version {} (the sandbox shows the math typeset)", DEFAULT_SEEDS[0])).size(13.0));
            let form = html.split_once("<form id=\"dadq\"").map(|(_, rest)| rest.split_once("</form>").map(|(f, _)| f).unwrap_or(rest)).unwrap_or("");
            egui::ScrollArea::vertical().id_salt("mom_qtext").max_height(ui.available_height() - 4.0).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(plain(form.split_once('>').map(|(_, f)| f).unwrap_or(form))).size(14.0)).wrap());
            });
        }
    }
}

/// What to do in the sandbox, and the approval that unlocks copying into MyOpenMath.
fn review_bar(ui: &mut Ui, st: &mut State) {
    let Some(c) = st.current.as_mut() else { return };
    if c.reviewed {
        ui.horizontal(|ui| {
            ui.label(RichText::new("✔ Approved for MyOpenMath").color(theme::green()));
            if ui.link("Copy it now").clicked() { st.tab = Tab::Copy; }
        });
        return;
    }
    ui.add(egui::Label::new(theme::muted("Try it as a student: answer it, press Check Answers, and draw a New Version or two. When it is right, approve it to unlock copying into MyOpenMath.").size(13.0)).wrap());
    ui.add_space(4.0);
    let why = if c.passed != Some(true) {
        Some("It has to pass the self-test first.")
    } else if !c.sandbox_seen {
        Some("Open the sandbox first.")
    } else {
        None
    };
    ui.horizontal(|ui| {
        let r = ui.add_enabled_ui(why.is_none(), |ui| pageshell::primary(ui, "Approve for MyOpenMath")).inner;
        if r.clicked() {
            c.reviewed = true;
            st.tab = Tab::Copy;
            st.note = Some("Approved. Copy it into MyOpenMath.".into());
        }
        if let Some(why) = why { ui.label(theme::muted(why).size(12.0)); }
    });
}

/// Question text without its HTML (and without style or script blocks), for the desktop's quick look.
fn plain(html: &str) -> String {
    let mut html = html.to_string();
    for (open, close) in [("<style", "</style>"), ("<script", "</script>")] {
        while let Some(a) = html.find(open) {
            let b = html[a..].find(close).map(|b| a + b + close.len()).unwrap_or(html.len());
            html.replace_range(a..b, " ");
        }
    }
    let html = html.as_str();
    let mut out = String::new();
    let mut tag = false;
    for ch in html.chars() {
        match ch { '<' => tag = true, '>' => { tag = false; out.push(' ') } _ if !tag => out.push(ch), _ => {} }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").replace("&nbsp;", " ")
}

fn code_tab(ui: &mut Ui, ctx: &egui::Context, st: &mut State, working: bool) {
    ui.horizontal(|ui| {
        if ui.add_enabled_ui(!working, |ui| pageshell::outline(ui, if working { "Testing…" } else { "Run Self-Test" })).inner.clicked() {
            start_test(st, ctx, false);
        }
        ui.label(theme::muted("Edit any field, then test it again at three random versions.").size(12.0));
    });
    ui.add_space(6.0);
    let mut changed = false;
    egui::ScrollArea::vertical().id_salt("mom_code").show(ui, |ui| {
        let c = st.current.as_mut().unwrap();
        let mut field = |ui: &mut Ui, label: &str, text: &mut String, rows: usize, code: bool| {
            ui.label(theme::strong(label).size(13.0));
            let mut e = egui::TextEdit::multiline(text).desired_rows(rows).desired_width(f32::INFINITY);
            if code { e = e.code_editor(); }
            let r = ui.add(e);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, label));
            if r.changed() { changed = true; }
            ui.add_space(6.0);
        };
        field(ui, "Description", &mut c.q.name, 1, false);
        field(ui, "Question type", &mut c.q.qtype, 1, false);
        field(ui, "Common Control", &mut c.q.control, 14, true);
        field(ui, "Question Text", &mut c.q.qtext, 8, true);
        field(ui, "Answer code (runs after the control, e.g. $solutionguide)", &mut c.q.answer_code, 5, true);
        if !c.q.solution.is_empty() { field(ui, "Detailed Solution", &mut c.q.solution, 4, false); }
    });
    if changed {
        if let Some(c) = st.current.as_mut() { c.passed = None; c.problems.clear(); c.reviewed = false; c.sandbox_seen = false; }
    }
}

fn copy_tab(ui: &mut Ui, st: &mut State) {
    if !st.current.as_ref().is_some_and(|c| c.reviewed) {
        ui.add_space(12.0);
        ui.label(theme::serif("Review it in the sandbox first").size(18.0));
        ui.add(egui::Label::new(theme::muted("Copying into MyOpenMath unlocks once you have tried this question as a student in the sandbox and approved it. Editing it locks it again.").size(13.0)).wrap());
        ui.add_space(8.0);
        if pageshell::primary(ui, "Go to the Sandbox").clicked() { st.tab = Tab::Preview; }
        return;
    }
    let c = st.current.as_ref().unwrap();
    ui.add(egui::Label::new(theme::muted("In MyOpenMath open Manage Question Set → Add New Question, then copy each field into the box of the same name. Or use the bookmarklet below to fill them all at once.").size(13.0)).wrap());
    ui.add_space(8.0);
    let fields = [
        ("Description", c.q.name.clone()),
        ("Question type", c.q.qtype.clone()),
        ("Common Control", c.q.common_control_field()),
        ("Question Text", c.q.qtext.clone()),
        ("Detailed Solution", c.q.solution.clone()),
    ];
    let mut copied = None;
    for (name, value) in &fields {
        if value.trim().is_empty() { continue; }
        ui.horizontal(|ui| {
            ui.add_sized([130.0, 22.0], egui::Label::new(theme::strong(*name).size(13.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Copy").on_hover_text(format!("Copy the {name} field")).clicked() {
                    ui.ctx().copy_text(value.clone());
                    copied = Some(*name);
                }
                let first = value.lines().next().unwrap_or("");
                ui.add(egui::Label::new(theme::muted(first).size(12.0).family(egui::FontFamily::Monospace)).truncate());
            });
        });
        ui.add_space(4.0);
    }
    ui.add_space(12.0);
    ui.separator();
    ui.label(theme::strong("Fill MyOpenMath in one click").size(14.0));
    ui.add(egui::Label::new(theme::muted("Once: make a bookmark and paste the bookmarklet as its address. Then: press Copy for Bookmarklet here, open the question editor in MyOpenMath, and click the bookmark. It fills every field; you check it and press Save.").size(13.0)).wrap());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if pageshell::primary(ui, "Copy for Bookmarklet").clicked() {
            ui.ctx().copy_text(bookmarklet_package(&c.q));
            copied = Some("the question for the bookmarklet");
        }
        if pageshell::outline(ui, "Copy Bookmarklet").on_hover_text("The bookmark's address. You only need this once.").clicked() {
            ui.ctx().copy_text(BOOKMARKLET.trim().to_string());
            copied = Some("the bookmarklet");
        }
    });
    if let Some(what) = copied { st.note = Some(format!("Copied {what}.")); }
    if let Some(n) = &st.note { ui.label(theme::muted(n).size(12.0)); }
}

/// What "Copy for Bookmarklet" puts on the clipboard: a marker and the editor's fields as JSON.
pub fn bookmarklet_package(q: &MomQuestion) -> String {
    format!("DADMOM1{}", serde_json::json!({
        "description": q.name, "qtype": q.qtype, "control": q.common_control_field(), "qtext": q.qtext, "solution": q.solution,
    }))
}

/// The bookmarklet: reads the package from the clipboard (or asks for it), then fills MyOpenMath's
/// question editor (course/moddataset.php): the type through its own menu, and each code box
/// through its CodeMirror editor when there is one, else the plain textarea. It never saves.
pub const BOOKMARKLET: &str = include_str!("mom/bookmarklet.js");

#[cfg(target_arch = "wasm32")]
mod overlay {
    //! The sandbox in the browser build: the engine's preview page in an iframe that fills the
    //! window, under a bar with Approve for MyOpenMath and Back to Editing. The bar's buttons are
    //! page elements; what they pressed is picked up by the screen on its next frame.
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicU8, Ordering};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast as _;

    const ID: &str = "dad_mom_sandbox";
    static ACTION: AtomicU8 = AtomicU8::new(0);
    thread_local! { static CTX: RefCell<Option<egui::Context>> = const { RefCell::new(None) }; }

    pub enum Action { Approve, Close }

    pub fn take_action() -> Option<Action> {
        match ACTION.swap(0, Ordering::Relaxed) { 1 => Some(Action::Approve), 2 => Some(Action::Close), _ => None }
    }

    fn button(doc: &web_sys::Document, text: &str, primary: bool, code: u8) -> Option<web_sys::Element> {
        let b = doc.create_element("button").ok()?;
        b.set_text_content(Some(text));
        let _ = b.set_attribute("type", "button");
        let _ = b.set_attribute("style", &format!(
            "font:600 15px 'Source Sans 3',system-ui,sans-serif;padding:8px 18px;border-radius:10px;cursor:pointer;border:1px solid {};background:{};color:{}",
            if primary { "#2d5e45" } else { "#cbbfa8" }, if primary { "#2d5e45" } else { "#fffdf8" }, if primary { "#fff" } else { "#241f16" }));
        let click = Closure::<dyn FnMut()>::new(move || {
            ACTION.store(code, Ordering::Relaxed);
            close();
            CTX.with(|c| if let Some(ctx) = c.borrow().as_ref() { ctx.request_repaint(); });
        });
        b.add_event_listener_with_callback("click", click.as_ref().unchecked_ref()).ok()?;
        click.forget();
        Some(b)
    }

    pub fn open(html: &str, can_approve: bool, ctx: &egui::Context) {
        CTX.with(|c| *c.borrow_mut() = Some(ctx.clone()));
        close();
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
        let Some(body) = doc.body() else { return };
        let Ok(panel) = doc.create_element("div") else { return };
        panel.set_id(ID);
        let _ = panel.set_attribute("role", "dialog");
        let _ = panel.set_attribute("aria-modal", "true");
        let _ = panel.set_attribute("aria-label", "Sandbox: try the question as a student");
        let _ = panel.set_attribute("style", "position:fixed;inset:0;z-index:30;display:flex;flex-direction:column;background:#f7f4ed");
        let Ok(bar) = doc.create_element("div") else { return };
        let _ = bar.set_attribute("style", "display:flex;flex-wrap:wrap;gap:10px;align-items:center;padding:12px 20px;border-bottom:1px solid #d8cfbf;background:#fffdf8;font:15px 'Source Sans 3',system-ui,sans-serif;color:#241f16");
        let Ok(title) = doc.create_element("div") else { return };
        let _ = title.set_attribute("style", "margin-right:auto;min-width:0");
        title.set_inner_html(if can_approve {
            "<b style=\"font-size:17px\">Sandbox</b> &nbsp;Answer it as a student, press Check Answers, and try a New Version or two."
        } else {
            "<b style=\"font-size:17px\">Sandbox</b> &nbsp;This question has problems left: fix them before approving it."
        });
        let _ = bar.append_child(&title);
        if let Some(b) = button(&doc, "Back to Editing", false, 2) { let _ = bar.append_child(&b); }
        if can_approve {
            if let Some(b) = button(&doc, "Approve for MyOpenMath", true, 1) { let _ = bar.append_child(&b); }
        }
        let Ok(frame) = doc.create_element("iframe") else { return };
        let Ok(frame) = frame.dyn_into::<web_sys::HtmlIFrameElement>() else { return };
        let _ = frame.set_attribute("title", "The question as a student sees it");
        let _ = frame.set_attribute("sandbox", "allow-scripts allow-same-origin allow-modals");
        let _ = frame.set_attribute("style", "flex:1;border:0;width:100%;background:#fff");
        frame.set_srcdoc(html);
        let _ = panel.append_child(&bar);
        let _ = panel.append_child(&frame);
        let _ = body.append_child(&panel);
        // Escape goes back to editing, as any dialog does.
        let key = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if e.key() == "Escape" { ACTION.store(2, Ordering::Relaxed); close(); }
        });
        let _ = panel.add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref());
        key.forget();
    }

    pub fn close() {
        if let Some(el) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(ID)) {
            el.remove();
        }
    }

    pub fn hide() { close(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_makes_a_file_name() {
        assert_eq!(slug("Slope between two points — two-part!"), "slope-between-two-points-two-part");
        assert_eq!(slug("  "), "question");
    }

    #[test]
    fn package_carries_the_editor_fields() {
        let q = MomQuestion { name: "N".into(), qtype: "number".into(), control: "$a=1".into(), qtext: "T".into(), answer_code: "$solutionguide='s'".into(), ..Default::default() };
        let p = bookmarklet_package(&q);
        assert!(p.starts_with("DADMOM1{"));
        let v: serde_json::Value = serde_json::from_str(&p[7..]).unwrap();
        assert_eq!(v["control"], "$a=1\n\n$solutionguide='s'");
        assert_eq!(v["qtype"], "number");
    }

    #[test]
    fn plain_drops_tags() {
        assert_eq!(plain("<p>What is <b>`$a`</b>?</p> $answerbox"), "What is `$a` ? $answerbox");
    }
}
