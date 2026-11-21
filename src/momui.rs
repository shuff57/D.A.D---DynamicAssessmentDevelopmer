//! The Write screen, for teachers who never see code: three steps.
//!
//! 1. Describe it: course, topic, answer type per part, difficulty, rounding and a few options, all
//!    dropdowns and checkboxes (src/mom/form.rs turns them into the AI's request). While the AI
//!    writes, progress is in plain words.
//! 2. Try it: the question exactly as students see it (the engine's MyOpenMath page: full-window in
//!    the browser build, the system browser on the desktop), then Looks Good, or a "change it"
//!    dropdown that rewrites it.
//! 3. Add to MyOpenMath: numbered steps with one copy button per editor box, and the bookmarklet.
//!
//! My Questions keeps approved questions. The code, the raw log and the checker's address sit behind
//! "Show advanced tools", off by default. Errors are plain sentences with the details folded away.

use crate::mom::author::{author_async_logged, post_engine, HttpEngine, ModelChat, AuthorEngine};
use crate::mom::engine::{SelftestResponse, DEFAULT_SEEDS};
use crate::mom::form::{self, Answer, Change, Difficulty, Form, Rounding, Topic, AI_CHOOSES, COURSES, MAX_PARTS};
use crate::mom::MomQuestion;
use crate::pageshell;
use crate::theme;
use egui::{RichText, Ui};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

/// The browser build runs the engine in the page itself; the desktop talks to engine/run.sh.
#[cfg(target_arch = "wasm32")]
pub const DEFAULT_ENGINE: &str = crate::mom::author::BUNDLED_ENGINE;
#[cfg(not(target_arch = "wasm32"))]
pub const DEFAULT_ENGINE: &str = "http://127.0.0.1:8791";
const LIBRARY: &str = "mom/library.json";
const FORM: &str = "mom/form.json";
const ADVANCED: &str = "mom/advanced.json";
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
    /// The helper browser is signed in to MyOpenMath: the teacher's courses.
    #[cfg(not(target_arch = "wasm32"))]
    Courses(Result<Vec<crate::mom::poster::Course>, String>),
    #[cfg(not(target_arch = "wasm32"))]
    Assessments(u64, Result<Vec<crate::mom::poster::Assessment>, String>),
    #[cfg(not(target_arch = "wasm32"))]
    Posted(Result<crate::mom::poster::Added, String>),
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
enum Step { #[default] Describe, Try, Copy }

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
    step: Step,
    form: Form,
    change: Change,
    change_other: String,
    /// Show the code, the raw log and the checker's address.
    advanced: bool,
    preview_html: Option<String>,
    preview_for: String,
    library: Library,
    /// What happened to the saved questions (saved, tested), shown in their card.
    lib_note: Option<String>,
    confirm_delete: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    poster: autoadd::Poster,
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
            if st.engine == crate::mom::author::BUNDLED_ENGINE { crate::mom::author::warm_bundled_engine(); }
            st.library = crate::store::load_json(LIBRARY).unwrap_or_default();
            st.form = crate::store::load_json(FORM).unwrap_or_default();
            st.form.fix();
            st.advanced = crate::store::load_json(ADVANCED).unwrap_or(false);
            // Screenshot/test hook: DAD_MOM_REQUEST pre-fills the request box.
            if let Ok(r) = crate::env_var("DAD_MOM_REQUEST") { st.request = r; }
            // Screenshot/test hook: DAD_MOM_OPEN=<file.php> opens a question and self-tests it;
            // DAD_MOM_TAB=code|copy|copy-approved picks the step (code: Try it with advanced tools open).
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(src) = crate::env_var("DAD_MOM_OPEN").ok().and_then(|p| std::fs::read_to_string(p).ok()) {
                st.current = Some(Current { q: MomQuestion::parse(&src), request: st.request.clone(), ..Default::default() });
                st.step = match crate::env_var("DAD_MOM_TAB").as_deref() { Ok("copy" | "copy-approved") => Step::Copy, _ => Step::Try };
                if crate::env_var("DAD_MOM_TAB").as_deref() == Ok("code") { st.advanced = true; }
                if crate::env_var("DAD_MOM_TAB").as_deref() == Ok("copy-approved") {
                    if let Some(c) = st.current.as_mut() { c.reviewed = true; c.sandbox_seen = true; }
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
        #[cfg(not(target_arch = "wasm32"))]
        Outcome::Courses(r) => autoadd::courses_in(st, r),
        #[cfg(not(target_arch = "wasm32"))]
        Outcome::Assessments(cid, r) => autoadd::assessments_in(st, cid, r),
        #[cfg(not(target_arch = "wasm32"))]
        Outcome::Posted(r) => autoadd::posted(st, r),
        Outcome::Authored { q, passed, problems, untested, error } => {
            st.error = error;
            if let Some(mut q) = q {
                // A question needs a name (MyOpenMath's Description); a model sometimes leaves it out.
                if q.name.trim().is_empty() {
                    q.name = st.form.short_name();
                }
                // A rewrite keeps the question's place in My Questions.
                let saved_id = st.current.as_ref().and_then(|c| c.saved_id.clone());
                st.current = Some(Current { q, request: st.request.clone(), passed: Some(passed), problems, untested, saved_id, ..Default::default() });
                st.step = Step::Try;
                st.preview_html = None;
                st.note = None;
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
            let n = st.library.questions.len();
            st.lib_note = Some(format!("Tested {n} saved question{}: {failing} need{} fixes{}.", if n == 1 { "" } else { "s" }, if failing == 1 { "s" } else { "" },
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

/// Ask the AI to write `request` (the form's request, or a "change it" request that carries the
/// current question); the teacher's own request stays in `st.request`.
fn start_write(st: &mut State, ctx: &egui::Context, cfg: &crate::ai::AiCfg, request: String) {
    st.log.clear();
    let job = start(st, "Writing");
    let (request, engine, cfg, ctx) = (request.trim().to_string(), st.engine.clone(), cfg.clone(), ctx.clone());
    run_bg(async move {
        let mut chat = ModelChat { cfg: &cfg, tools: false };
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
    let name = if c.q.name.trim().is_empty() { "Practice question".to_string() } else { c.q.name.clone() };
    st.preview_for = src.clone();
    let job = start(st, "Drawing the preview");
    let (engine, ctx) = (st.engine.clone(), ctx.clone());
    run_bg(async move {
        // The page's own Check Answers / New Version calls go to the same engine: "bundled" makes the
        // page ask the app's window.dadEngine instead of fetching a server.
        // MyOpenMath's own student player (assess2), answered by the engine: the same page, score
        // box, "Get a similar question" and "Show Detailed Solution" students see.
        let body = serde_json::json!({"src": src, "name": name, "seed": DEFAULT_SEEDS[0], "engine": engine, "static": sandbox_static(&engine)});
        let r = post_engine(&engine, "/api/assess", &body).await
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
    st.lib_note = Some(match crate::store::save_json(LIBRARY, &st.library) {
        Ok(()) => if c.passed == Some(true) { "Saved to My Questions.".into() } else { "Saved to My Questions, marked as needing fixing.".into() },
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

/// Field label above its control.
fn field_label(ui: &mut Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(theme::strong(text).size(14.0));
    ui.add_space(2.0);
}

/// A dropdown of `labels`; returns true when the choice changed.
fn dropdown(ui: &mut Ui, id: &str, labels: &[&str], cur: &mut usize, width: f32) -> bool {
    let before = *cur;
    let shown = labels.get(*cur).copied().unwrap_or("");
    // Tall enough that a course's whole topic list (and "Other" at its end) shows without scrolling.
    egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).height(520.0).show_ui(ui, |ui| {
        for (i, l) in labels.iter().enumerate() {
            ui.selectable_value(cur, i, *l);
        }
    });
    *cur != before
}

fn page(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    pageshell::title(ui, "", "Write a Question",
        "Pick what you need. The AI writes it and checks it, you try it as a student, then it goes into MyOpenMath.");
    ui.add_space(8.0);
    steps(ui, st);
    ui.add_space(12.0);
    let wide = ui.available_width() >= 1040.0;
    if wide {
        let side_w = 320.0;
        ui.horizontal_top(|ui| {
            let main_w = ui.available_width() - side_w - 16.0;
            ui.allocate_ui_with_layout(egui::vec2(main_w, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().id_salt("mom_main").show(ui, |ui| main(ui, ctx, st, cfg));
            });
            ui.add_space(16.0);
            ui.allocate_ui_with_layout(egui::vec2(side_w, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().id_salt("mom_side").show(ui, |ui| side(ui, ctx, st));
            });
        });
    } else {
        egui::ScrollArea::vertical().id_salt("mom_stack").show(ui, |ui| {
            main(ui, ctx, st, cfg);
            ui.add_space(12.0);
            side(ui, ctx, st);
        });
    }
}

/// The three steps across the top; a step opens once the one before it is done.
fn steps(ui: &mut Ui, st: &mut State) {
    let has_q = st.current.is_some();
    let reviewed = st.current.as_ref().is_some_and(|c| c.reviewed);
    ui.horizontal_wrapped(|ui| {
        for (i, (step, name, open)) in [(Step::Describe, "Describe it", true), (Step::Try, "Try it", has_q), (Step::Copy, "Add to MyOpenMath", reviewed)].into_iter().enumerate() {
            if i > 0 { ui.label(theme::muted("›").size(18.0)); }
            let label = format!("{}   {name}", i + 1);
            let r = ui.add_enabled_ui(open && !busy(st), |ui| step_pill(ui, &label, st.step == step)).inner;
            if r.clicked() { st.step = step; }
        }
        if has_q && !busy(st) {
            ui.add_space(12.0);
            if pageshell::outline(ui, "Start a New Question").on_hover_text("Your choices below stay picked").clicked() {
                st.step = Step::Describe;
                st.current = None;
                st.error = None;
                st.preview_html = None;
            }
        }
    });
}

/// One step in the row across the top: the current one filled, the others outlined.
fn step_pill(ui: &mut Ui, label: &str, on: bool) -> egui::Response {
    let fill = if on { theme::accent_faded() } else { theme::card() };
    let stroke = egui::Stroke::new(if on { 1.5 } else { 1.0 }, if on { theme::accent_col() } else { theme::border() });
    let fg = if on { theme::accent_col() } else { theme::text_col() };
    ui.add(egui::Button::new(RichText::new(label).family(theme::bold_family()).size(15.0).color(fg))
        .fill(fill).stroke(stroke).corner_radius(egui::CornerRadius::same(255)).min_size(egui::vec2(0.0, 36.0)))
}

fn main(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    if busy(st) && st.job.lock().map(|j| j.what == "Writing").unwrap_or(false) {
        leave();
        writing(ui, st);
    } else {
        match st.step {
            Step::Describe => { leave(); describe(ui, ctx, st, cfg) }
            Step::Try => try_it(ui, ctx, st, cfg),
            Step::Copy => { leave(); copy_step(ui, ctx, st) }
        }
    }
    ui.add_space(12.0);
    advanced(ui, ctx, st, cfg);
}

/// A dropdown whose first entry is "Let the AI choose" (None), then `labels` (Some(index)).
fn ai_dropdown(ui: &mut Ui, id: &str, labels: &[&str], cur: &mut Option<usize>, width: f32) -> bool {
    let mut all = vec![AI_CHOOSES];
    all.extend_from_slice(labels);
    let mut k = cur.map(|i| i + 1).unwrap_or(0);
    let changed = dropdown(ui, id, &all, &mut k, width);
    if changed { *cur = k.checked_sub(1); }
    changed
}

/// [`ai_dropdown`] over a list of values.
fn ai_pick<T: Copy + PartialEq>(ui: &mut Ui, id: &str, all: &[T], label: impl Fn(T) -> &'static str, cur: &mut Option<T>, width: f32) {
    let labels: Vec<&str> = all.iter().map(|v| label(*v)).collect();
    let mut k = cur.and_then(|v| all.iter().position(|x| *x == v));
    if ai_dropdown(ui, id, &labels, &mut k, width) { *cur = k.map(|i| all[i]); }
}

fn describe(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, "What kind of question do you need?");
        ui.label(theme::muted("Pick what matters to you. Anything left on \"Let the AI choose\" is up to the AI.").size(13.0));
        let f = &mut st.form;

        field_label(ui, "Course");
        let names: Vec<&str> = COURSES.iter().map(|c| c.0).collect();
        if ai_dropdown(ui, "mom_course", &names, &mut f.course, 320.0) { f.topic = Topic::Ai; f.fix(); }

        field_label(ui, "Topic");
        let mut topics: Vec<&str> = vec![AI_CHOOSES];
        topics.extend_from_slice(f.topics());
        topics.push("Other (type it)");
        let other = topics.len() - 1;
        let mut ti = match f.topic { Topic::Ai => 0, Topic::Listed(i) => i + 1, Topic::Typed => other };
        if dropdown(ui, "mom_topic", &topics, &mut ti, 320.0) {
            f.topic = if ti == 0 { Topic::Ai } else if ti == other { Topic::Typed } else { Topic::Listed(ti - 1) };
        }
        if f.topic == Topic::Typed {
            ui.add_space(4.0);
            let r = ui.add(egui::TextEdit::singleline(&mut f.topic_text).desired_width(320.0).hint_text("For example: Venn diagrams"));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Your topic"));
            if f.topic_text.trim().is_empty() { crate::kit::attention_ring(ui, &r); }
            if ui.link(theme::muted("Pick from the list instead").size(13.0)).clicked() { f.topic = Topic::Ai; }
        } else if ui.link(theme::muted("My topic isn't in the list").size(13.0)).clicked() {
            f.topic = Topic::Typed;
        }

        field_label(ui, "How many parts?");
        let counts: Vec<String> = (1..=MAX_PARTS).map(|n| if n == 1 { "1 (a single question)".into() } else { format!("{n} parts") }).collect();
        let counts: Vec<&str> = counts.iter().map(String::as_str).collect();
        let mut n = f.part_count.map(|n| n - 1);
        if ai_dropdown(ui, "mom_parts", &counts, &mut n, 220.0) { f.part_count = n.map(|n| n + 1); f.fix(); }

        let per_part = f.part_count.is_some_and(|n| n > 1);
        field_label(ui, match f.part_count {
            None => "What does the student answer? (every part)",
            Some(1) => "What does the student answer?",
            Some(_) => "What does the student answer in each part?",
        });
        for i in 0..f.answers.len() {
            ui.horizontal(|ui| {
                if per_part { ui.add_sized([62.0, 22.0], egui::Label::new(theme::text(format!("Part ({})", (b'a' + i as u8) as char)))); }
                ai_pick(ui, &format!("mom_ans_{i}"), &Answer::ALL, Answer::label, &mut f.answers[i], 300.0);
            });
        }
        if f.answers.contains(&Some(Answer::Written)) {
            ui.label(theme::muted("Written answers aren't checked automatically: you grade them in MyOpenMath.").size(12.0));
        }

        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                field_label(ui, "Difficulty");
                ai_pick(ui, "mom_diff", &Difficulty::ALL, Difficulty::label, &mut f.difficulty, 170.0);
            });
            if f.uses_rounding() {
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    field_label(ui, "Round answers to");
                    ai_pick(ui, "mom_round", &Rounding::ALL, Rounding::label, &mut f.rounding, 190.0);
                });
            }
            if f.uses_options() {
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    field_label(ui, "Answer choices");
                    let labels = ["2 choices", "3 choices", "4 choices", "5 choices", "6 choices"];
                    let mut k = f.options.map(|n| n - 2);
                    if ai_dropdown(ui, "mom_opts", &labels, &mut k, 170.0) { f.options = k.map(|k| k + 2); }
                });
            }
            ui.add_space(16.0);
            ui.vertical(|ui| {
                field_label(ui, "Real-world situation");
                let mut k = f.story.map(|b| if b { 0 } else { 1 });
                if ai_dropdown(ui, "mom_story", &["Yes, use a story", "No, just the math"], &mut k, 190.0) { f.story = k.map(|k| k == 0); }
            });
        });

        field_label(ui, "Options");
        ui.checkbox(&mut f.random, "Give every student different numbers");
        ui.horizontal(|ui| {
            ui.label(RichText::new("✔").color(theme::green()));
            ui.label(theme::text("Every question comes with a worked solution, shown to students after they answer.").size(13.0));
        });

        field_label(ui, "Anything else? (optional)");
        let r = ui.add(egui::TextEdit::multiline(&mut f.extra).desired_rows(2).desired_width(f32::INFINITY)
            .hint_text("For example: use money amounts, or ask for the y-intercept too"));
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Anything else (optional)"));

        ui.add_space(16.0);
        let missing = f.missing();
        if pageshell::primary_on(ui, "Write My Question", missing.is_none()).clicked() && missing.is_none() {
            st.request = st.form.request();
            let _ = crate::store::save_json(FORM, &st.form);
            let request = st.request.clone();
            start_write(st, ctx, cfg, request);
        }
        if let Some(why) = missing { crate::kit::gated_reason(ui, why); }
        error_box(ui, st);
    });
}

/// A plain-language error, with the technical text one click away.
fn error_box(ui: &mut Ui, st: &State) {
    if let Some(e) = &st.error {
        ui.add_space(8.0);
        ui.add(egui::Label::new(RichText::new(form::plain_error(e)).color(theme::red()).size(14.0)).wrap());
        egui::CollapsingHeader::new(theme::muted("Show details").size(12.0)).id_salt("mom_err_details").show(ui, |ui| {
            ui.add(egui::Label::new(theme::muted(e).size(12.0)).wrap());
        });
    }
}

fn writing(ui: &mut Ui, st: &State) {
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, "Writing your question");
        ui.add_space(6.0);
        let mut said: Vec<String> = Vec::new();
        for line in &st.log {
            if let Some(p) = form::plain_progress(line, MAX_TRIES) {
                if said.last() != Some(&p) { said.push(p); }
            }
        }
        if said.is_empty() { said.push("Writing your question…".into()); }
        let last = said.len() - 1;
        for (i, p) in said.iter().enumerate().skip(said.len().saturating_sub(5)) {
            ui.horizontal(|ui| {
                if i == last { ui.spinner(); ui.label(theme::strong(p).size(15.0)); }
                else { ui.label(RichText::new("✔").color(theme::green())); ui.label(theme::muted(p).size(14.0)); }
            });
        }
        ui.add_space(8.0);
        ui.label(theme::muted("This usually takes between 10 seconds and a minute. You can leave this page open and come back.").size(13.0));
    });
}

fn try_it(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    let Some(c) = st.current.as_ref() else { st.step = Step::Describe; return };
    let working = busy(st);
    let src = c.q.emit();
    let stale = st.preview_html.is_none() || st.preview_for != src;
    if stale && !working && st.error.is_none() && c.passed == Some(true) {
        start_preview(st, ctx);
    }
    #[cfg(target_arch = "wasm32")]
    match overlay::take_action() {
        Some(overlay::Action::Approve) => approve(st),
        Some(overlay::Action::Close) | None => {}
    }
    pageshell::card(ui, |ui| {
        let c = st.current.as_ref().unwrap();
        ui.add(egui::Label::new(theme::serif(if c.q.name.is_empty() { "Your question" } else { &c.q.name }).size(20.0)).wrap());
        ui.add_space(6.0);
        match c.passed {
            Some(true) => { ui.label(RichText::new("✔ It works: we checked it with 3 different sets of numbers.").color(theme::green()).size(14.0)); }
            Some(false) => {
                ui.add(egui::Label::new(RichText::new("✖ The AI couldn't get this one fully working.").color(theme::red()).size(15.0)).wrap());
                ui.label(theme::muted("Write it again (it often works the second time), or go back and change your choices.").size(13.0));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled_ui(!working, |ui| pageshell::primary(ui, "Write It Again")).inner.clicked() {
                        let request = st.request.clone();
                        start_write(st, ctx, cfg, request);
                    }
                    if pageshell::outline(ui, "Change My Choices").clicked() { st.step = Step::Describe; }
                });
                let c = st.current.as_ref().unwrap();
                egui::CollapsingHeader::new(theme::muted("Show details").size(12.0)).id_salt("mom_problems").show(ui, |ui| {
                    for p in c.problems.iter().take(6) { ui.add(egui::Label::new(theme::muted(format!("• {p}")).size(12.0)).wrap()); }
                });
                return;
            }
            None if working => { ui.horizontal(|ui| { ui.spinner(); ui.label(theme::muted("Checking it…")); }); }
            None => { ui.label(theme::muted("Changed since the last check. Open Advanced tools and press Check It Again.")); }
        }
        let c = st.current.as_ref().unwrap();
        if !c.untested.is_empty() {
            ui.label(theme::muted(format!("Part {} is a written or upload answer: check that one yourself.",
                c.untested.iter().map(|p| ((b'a' + *p as u8) as char).to_string()).collect::<Vec<_>>().join(", "))).size(13.0));
        }

        ui.add_space(14.0);
        ui.label(theme::strong("Try it as a student").size(15.0));
        ui.add(egui::Label::new(theme::muted("It opens exactly as your students will see it in MyOpenMath. Answer it and press Submit: the worked solution appears under the question. Then try \"Try a similar question\" once or twice to see new numbers.").size(13.0)).wrap());
        if st.current.as_ref().is_some_and(|c| c.q.detailed_solution_field().is_empty()) {
            ui.add(egui::Label::new(RichText::new("This one has no worked solution. Pick \"Make the worked solution clearer\" below and press Rewrite It to add one.").color(theme::red()).size(13.0)).wrap());
        }
        ui.add_space(6.0);
        let ready = st.preview_html.is_some();
        if ui.add_enabled_ui(ready, |ui| pageshell::primary(ui, if ready { "Try It as a Student" } else { "Getting it ready…" })).inner.clicked() {
            open_sandbox(st, ctx);
        }

        ui.add_space(14.0);
        let (seen, reviewed) = st.current.as_ref().map(|c| (c.sandbox_seen, c.reviewed)).unwrap_or_default();
        if reviewed {
            ui.horizontal(|ui| {
                ui.label(RichText::new("✔ Approved and saved").color(theme::green()));
                if pageshell::primary(ui, "Add It to MyOpenMath").clicked() { st.step = Step::Copy; }
            });
        } else {
            ui.label(theme::strong("Happy with it?").size(15.0));
            ui.add_space(4.0);
            if pageshell::primary_on(ui, "Looks Good", seen).on_hover_text("Saves it to My Questions and shows how to add it to MyOpenMath").clicked() && seen {
                approve(st);
            }
            if !seen { crate::kit::gated_reason(ui, "Try it as a student first."); }
        }

        ui.add_space(14.0);
        ui.label(theme::strong("Not quite right?").size(15.0));
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let labels: Vec<&str> = Change::ALL.iter().map(|c| c.label()).collect();
            let mut k = Change::ALL.iter().position(|c| *c == st.change).unwrap_or(0);
            if dropdown(ui, "mom_change", &labels, &mut k, 240.0) { st.change = Change::ALL[k]; }
            let other_ok = st.change != Change::Other || !st.change_other.trim().is_empty();
            if ui.add_enabled_ui(!working, |ui| pageshell::primary_on(ui, "Rewrite It", other_ok)).inner.clicked() && other_ok && !working {
                let c = st.current.as_ref().unwrap();
                let request = form::change_request(&c.request, &c.q.emit(), st.change, &st.change_other);
                start_write(st, ctx, cfg, request);
            }
        });
        if st.change == Change::Other {
            ui.add_space(4.0);
            let r = ui.add(egui::TextEdit::singleline(&mut st.change_other).desired_width(f32::INFINITY).hint_text("For example: use a pizza shop instead of a bakery"));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "What to change"));
        }
        error_box(ui, st);

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(html) = &st.preview_html {
            ui.add_space(14.0);
            ui.label(theme::strong("Quick look (the full version opens in your browser)").size(13.0));
            let question = sandbox_question_html(html).unwrap_or_default();
            ui.add(egui::Label::new(RichText::new(plain(&question)).size(14.0)).wrap());
        }
    });
}

fn open_sandbox(st: &mut State, ctx: &egui::Context) {
    #[cfg(target_arch = "wasm32")]
    {
        let can_approve = st.current.as_ref().is_some_and(|c| c.passed == Some(true));
        overlay::open(st.preview_html.as_deref().unwrap_or(""), can_approve, ctx);
        if let Some(c) = st.current.as_mut() { c.sandbox_seen = true; }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ctx;
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
}

/// The teacher tried it and likes it: save it and show how to add it to MyOpenMath.
fn approve(st: &mut State) {
    let Some(c) = st.current.as_mut() else { return };
    if c.passed != Some(true) { return; }
    c.reviewed = true;
    save_current(st);
    st.step = Step::Copy;
}

fn copy_step(ui: &mut Ui, ctx: &egui::Context, st: &mut State) {
    let Some(c) = st.current.as_ref() else { st.step = Step::Describe; return };
    if !c.reviewed {
        pageshell::card(ui, |ui| {
            pageshell::heading(ui, "Try it first");
            ui.label(theme::muted("Try the question as a student and press Looks Good. Then this page shows how to add it to MyOpenMath.").size(14.0));
            ui.add_space(8.0);
            if pageshell::primary(ui, "Go to Try It").clicked() { st.step = Step::Try; }
        });
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        autoadd::card(ui, ctx, st);
        ui.add_space(12.0);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        webhelper::card(ui, st);
        ui.add_space(12.0);
    }
    let Some(c) = st.current.as_ref() else { return };
    let menu = form::type_menu_label(&c.q.qtype);
    let menu = if menu.is_empty() { c.q.qtype.trim().to_string() } else { menu.to_string() };
    let fields = [
        ("Description", c.q.name.clone(), "the Description box"),
        ("Common Control", c.q.common_control_field(), "the Common Control box"),
        ("Question Text", c.q.qtext.clone(), "the Question Text box"),
        ("Detailed Solution", c.q.detailed_solution_field(), "the Detailed Solution box"),
    ];
    let mut copied: Option<String> = None;
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, if cfg!(target_arch = "wasm32") { "Add it to MyOpenMath" } else { "Or copy it in yourself" });
        ui.add_space(8.0);
        ui.label(theme::strong("1.  In MyOpenMath, open your course, click Manage Question Set, then Add New Question.").size(14.0));
        ui.add_space(10.0);
        ui.label(theme::strong(format!("2.  Set the Question type menu to \"{menu}\".")).size(14.0));
        ui.add_space(10.0);
        ui.label(theme::strong("3.  Copy each item and paste it into the box with the same name.").size(14.0));
        ui.add_space(6.0);
        for (name, value, place) in &fields {
            if value.trim().is_empty() { continue; }
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                if pageshell::outline(ui, &format!("Copy {name}")).clicked() {
                    ui.ctx().copy_text(value.clone());
                    copied = Some(format!("Copied. Now paste it into {place} in MyOpenMath."));
                }
            });
            if *name == "Detailed Solution" {
                ui.horizontal(|ui| {
                    ui.add_space(22.0);
                    ui.add(egui::Label::new(theme::muted("Above that box, tick \"Uses random variables from the question\" so students see the solution for their own numbers.").size(13.0)).wrap());
                });
            }
            ui.add_space(4.0);
        }
        ui.add_space(10.0);
        ui.label(theme::strong("4.  Press Save in MyOpenMath.").size(14.0));
        if let Some(n) = copied.as_ref().or(st.note.as_ref()) {
            ui.add_space(8.0);
            ui.label(RichText::new(n).color(theme::green()).size(14.0));
        }
    });
    if let Some(n) = copied { st.note = Some(n); }
    ui.add_space(12.0);
    pageshell::card(ui, |ui| {
        egui::CollapsingHeader::new(theme::strong("Faster: fill every box with one click").size(15.0)).id_salt("mom_bm").show(ui, |ui| {
            ui.add(egui::Label::new(theme::muted("Set up once: press Copy the Bookmark, make a new bookmark in your browser, and paste it as the bookmark's address (URL). Name it \"Fill MyOpenMath\".").size(13.0)).wrap());
            ui.add_space(4.0);
            if pageshell::outline(ui, "Copy the Bookmark").clicked() {
                ui.ctx().copy_text(BOOKMARKLET.trim().to_string());
                st.note = Some("Copied the bookmark. Paste it as a new bookmark's address.".into());
            }
            ui.add_space(8.0);
            ui.add(egui::Label::new(theme::muted("Each time: press Copy This Question, open Add New Question in MyOpenMath, and click your Fill MyOpenMath bookmark. Check the boxes, then press Save.").size(13.0)).wrap());
            ui.add_space(4.0);
            if pageshell::primary(ui, "Copy This Question").clicked() {
                let c = st.current.as_ref().unwrap();
                ui.ctx().copy_text(bookmarklet_package(&c.q));
                st.note = Some("Copied. Now click your Fill MyOpenMath bookmark in MyOpenMath.".into());
            }
        });
    });
}

/// My Questions: everything saved, with a plain status and Open / Delete.
fn side(ui: &mut Ui, ctx: &egui::Context, st: &mut State) {
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, "My Questions");
        ui.add_space(4.0);
        if st.library.questions.is_empty() {
            ui.label(theme::muted("Questions you approve are saved here so you can add them again later.").size(13.0));
        }
        let mut open: Option<Saved> = None;
        let mut delete: Option<String> = None;
        for s in &st.library.questions {
            let q = MomQuestion::parse(&s.src);
            let name = if q.name.is_empty() { "Untitled question".to_string() } else { q.name.clone() };
            ui.add_space(6.0);
            ui.add(egui::Label::new(theme::text(&name).size(14.0)).wrap());
            ui.horizontal(|ui| {
                let (status, fill, fg) = if !s.passed {
                    ("Needs fixing", theme::error_wash(), theme::red())
                } else if s.reviewed {
                    ("Ready", theme::accent_faded(), theme::accent_col())
                } else {
                    ("Not tried yet", theme::highlighter(), theme::on_highlighter())
                };
                crate::kit::pill(ui, status, fill, fg);
                if ui.link("Open").clicked() { open = Some(s.clone()); }
                if st.confirm_delete.as_deref() == Some(s.id.as_str()) {
                    if ui.button(RichText::new("Yes, delete").color(theme::red())).clicked() { delete = Some(s.id.clone()); }
                    if ui.button("Keep it").clicked() { st.confirm_delete = None; }
                } else if ui.link(RichText::new("Delete").color(theme::muted_col())).clicked() {
                    st.confirm_delete = Some(s.id.clone());
                }
            });
        }
        if let Some(s) = open {
            let q = MomQuestion::parse(&s.src);
            st.current = Some(Current { q, request: s.request.clone(), passed: Some(s.passed), problems: s.problems.clone(), untested: vec![], saved_id: Some(s.id.clone()), sandbox_seen: s.reviewed, reviewed: s.reviewed });
            st.request = s.request;
            st.preview_html = None;
            st.error = None;
            st.note = None;
            st.step = if s.reviewed { Step::Copy } else { Step::Try };
        }
        if let Some(id) = delete {
            st.library.questions.retain(|s| s.id != id);
            st.confirm_delete = None;
            if st.current.as_ref().and_then(|c| c.saved_id.as_deref()) == Some(id.as_str()) {
                if let Some(c) = st.current.as_mut() { c.saved_id = None; }
            }
            let _ = crate::store::save_json(LIBRARY, &st.library);
        }
        if st.advanced && !st.library.questions.is_empty() {
            ui.add_space(8.0);
            if ui.add_enabled_ui(!busy(st), |ui| pageshell::outline(ui, "Check All Again")).inner
                .on_hover_text("Run every saved question through the checker again").clicked() {
                start_test_all(st, ctx);
            }
        }
        if let Some(n) = &st.lib_note {
            ui.add_space(4.0);
            ui.label(theme::muted(n).size(12.0));
        }
    });
}

/// Off by default: the code, the raw log, the checker's address and which AI is used.
fn advanced(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    if ui.checkbox(&mut st.advanced, theme::muted("Show advanced tools (question code, test log, checker)").size(13.0)).changed() {
        let _ = crate::store::save_json(ADVANCED, &st.advanced);
    }
    if !st.advanced { return; }
    let working = busy(st);
    pageshell::card(ui, |ui| {
        ui.label(theme::muted(format!("AI: {} · {}   (change it in Settings, then AI)", cfg.provider, cfg.model)).size(12.0));
        if !st.request.is_empty() {
            egui::CollapsingHeader::new(theme::muted("What the AI was told").size(13.0)).id_salt("mom_req").show(ui, |ui| {
                ui.add(egui::Label::new(theme::muted(&st.request).size(12.0)).wrap());
            });
        } else if st.step == Step::Describe {
            egui::CollapsingHeader::new(theme::muted("What the AI will be told").size(13.0)).id_salt("mom_req_next").show(ui, |ui| {
                ui.add(egui::Label::new(theme::muted(st.form.request()).size(12.0)).wrap());
            });
        }
        if !st.log.is_empty() {
            egui::CollapsingHeader::new(theme::muted("Test log").size(13.0)).id_salt("mom_log").show(ui, |ui| {
                for line in st.log.iter().rev().take(30).collect::<Vec<_>>().into_iter().rev() {
                    ui.add(egui::Label::new(RichText::new(line).size(12.0).color(theme::muted_col())).wrap());
                }
            });
        }
        if st.current.is_some() {
            egui::CollapsingHeader::new(theme::muted("Question code").size(13.0)).id_salt("mom_code_hdr").show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.add_enabled_ui(!working, |ui| pageshell::outline(ui, "Download File")).inner.clicked() {
                        let c = st.current.as_ref().unwrap();
                        let name = format!("{}.php", slug(&c.q.name));
                        if let Some(path) = crate::platform::save_path(&name, "MyOpenMath question", &["php"]) {
                            st.note = Some(match crate::platform::write(&path, c.q.emit().as_bytes()) {
                                Ok(()) => format!("Saved {}", path.display()),
                                Err(e) => format!("Could not save the file: {e}"),
                            });
                        }
                    }
                    if ui.add_enabled_ui(!working, |ui| pageshell::outline(ui, "Save to My Questions")).inner.clicked() {
                        start_test(st, ctx, true);
                    }
                });
                code_tab(ui, ctx, st, working);
            });
        }
        egui::CollapsingHeader::new(theme::muted("Question checker").size(13.0)).id_salt("mom_engine").show(ui, |ui| {
            ui.label(theme::muted(if st.engine == crate::mom::author::BUNDLED_ENGINE {
                "Built in: IMathAS runs inside this page. Type a server address to use one instead."
            } else {
                "The IMathAS engine that checks questions. Start it with engine/run.sh."
            }).size(12.0));
            let r = ui.add(egui::TextEdit::singleline(&mut st.engine).desired_width(240.0).hint_text(DEFAULT_ENGINE));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Checker address"));
            if r.lost_focus() { let _ = crate::store::save_json("mom/engine.json", &st.engine); }
        });
    });
}

/// The question's own HTML inside the sandbox page, which hands it to IMathAS's `showandinit(0, {...})`.
/// (Desktop quick look; the browser build shows the page itself.)
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn sandbox_question_html(page: &str) -> Option<String> {
    let rest = &page[page.find("showandinit(")? ..];
    let rest = &rest[rest.find(',')? + 1..];
    let v: serde_json::Value = serde_json::Deserializer::from_str(rest).into_iter().next()?.ok()?;
    v.get("html").and_then(|h| h.as_str()).map(str::to_string)
}

/// Where the sandbox page loads MyOpenMath's scripts and styles from: the web app's own copy when the
/// engine is built in, else the engine server, which serves them.
fn sandbox_static(engine: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    if engine == crate::mom::author::BUNDLED_ENGINE {
        let base = web_sys::window().and_then(|w| w.document()).and_then(|d| d.base_uri().ok().flatten()).unwrap_or_default();
        let base = base.split(['?', '#']).next().unwrap_or("").to_string();
        let dir = &base[..base.rfind('/').map(|i| i + 1).unwrap_or(base.len())];
        return format!("{dir}engine/imathas");
    }
    engine.trim_end_matches('/').to_string()
}

/// Question text without its HTML (and without style or script blocks), for the desktop's quick look.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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
        if ui.add_enabled_ui(!working, |ui| pageshell::outline(ui, if working { "Checking…" } else { "Check It Again" })).inner.clicked() {
            start_test(st, ctx, false);
        }
        ui.label(theme::muted("Edit any box, then check it again with three sets of numbers.").size(12.0));
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

/// What "Copy for Bookmarklet" puts on the clipboard: a marker and the editor's fields as JSON.
pub fn bookmarklet_package(q: &MomQuestion) -> String {
    format!("DADMOM1{}", serde_json::json!({
        "description": q.name, "qtype": q.qtype, "control": q.common_control_field(), "qtext": q.qtext, "solution": q.detailed_solution_field(),
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
    thread_local! {
        static CTX: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
        /// The open sandbox page's blob: URL, released when the next one opens.
        static BLOB: RefCell<Option<String>> = const { RefCell::new(None) };
    }

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
            "<b style=\"font-size:17px\">Try it as a student</b> &nbsp;Answer it and press Submit, then Show Detailed Solution. Try \"Try a similar question\" once or twice."
        } else {
            "<b style=\"font-size:17px\">Try it as a student</b> &nbsp;This question still has a problem, so it can't be approved yet."
        });
        let _ = bar.append_child(&title);
        if let Some(b) = button(&doc, "Back", false, 2) { let _ = bar.append_child(&b); }
        if can_approve {
            if let Some(b) = button(&doc, "Looks Good", true, 1) { let _ = bar.append_child(&b); }
        }
        let Ok(frame) = doc.create_element("iframe") else { return };
        let Ok(frame) = frame.dyn_into::<web_sys::HtmlIFrameElement>() else { return };
        let _ = frame.set_attribute("title", "The question as a student sees it");
        let _ = frame.set_attribute("sandbox", "allow-scripts allow-same-origin allow-modals");
        let _ = frame.set_attribute("style", "flex:1;border:0;width:100%;background:#fff");
        // A blob: page, not srcdoc: MyOpenMath's player routes with the URL's #hash, and in a srcdoc
        // frame that resolves against the app's own address, which loads the app inside the frame.
        let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(html));
        let bag = web_sys::BlobPropertyBag::new();
        bag.set_type("text/html");
        let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &bag) else { return };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
        frame.set_src(&url);
        BLOB.with(|b| { if let Some(old) = b.borrow_mut().replace(url) { let _ = web_sys::Url::revoke_object_url(&old); } });
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
    fn the_question_is_read_out_of_the_sandbox_page() {
        let page = r#"<script>$(function() { showandinit(0,{"html":"<p>What is 2?</p>","jsparams":{}}); });</script>"#;
        assert_eq!(sandbox_question_html(page).as_deref(), Some("<p>What is 2?</p>"));
        assert_eq!(sandbox_question_html("<p>nothing</p>"), None);
    }

    #[test]
    fn plain_drops_tags() {
        assert_eq!(plain("<p>What is <b>`$a`</b>?</p> $answerbox"), "What is `$a` ? $answerbox");
    }
}

/// "Add it to MyOpenMath for me" (desktop): D.A.D opens MyOpenMath in its own Chrome window, with a
/// profile of its own so the sign-in is remembered, and fills (and optionally saves) the question in
/// the assessment the teacher picks. Sign-in is the teacher's, in that window, unless they saved a
/// MyOpenMath password in the keychain, which D.A.D then types into the sign-in form itself.
#[cfg(not(target_arch = "wasm32"))]
mod autoadd {
    use super::*;
    use crate::mom::chrome::{Chrome, Tab};
    use crate::mom::poster::{self, Added, Assessment, Course};
    use std::time::Duration;

    const PREFS: &str = "mom/poster.json";

    /// The helper browser, kept open between adds.
    static BROWSER: Mutex<Option<Chrome>> = Mutex::new(None);

    #[derive(Serialize, Deserialize, Default, Clone)]
    #[serde(default)]
    struct Prefs { course: Option<u64>, assessment: Option<u64>, auto_save: bool }

    #[derive(Default)]
    pub struct Poster {
        loaded: bool,
        prefs: Prefs,
        courses: Vec<Course>,
        assessments: Vec<Assessment>,
        assessments_for: Option<u64>,
        result: Option<Result<String, String>>,
        user: String,
        pass: String,
        has_login: Option<bool>,
    }

    fn tab() -> Result<Tab, String> {
        let mut b = BROWSER.lock().map_err(|_| "browser lock")?;
        if !b.as_mut().is_some_and(|c| c.alive()) {
            let profile = crate::store::store_dir().join("mom").join("browser");
            *b = Some(Chrome::launch(&profile, false)?);
        }
        b.as_ref().unwrap().tab()
    }

    fn log(job: &Arc<Mutex<Job>>, ctx: &egui::Context, line: &str) {
        if let Ok(mut j) = job.lock() { j.log.push(line.to_string()); }
        ctx.request_repaint();
    }

    /// Signed in, by the saved password or by the teacher in the window (waits up to 10 minutes).
    fn ensure_signed_in(t: &mut Tab, job: &Arc<Mutex<Job>>, ctx: &egui::Context) -> Result<(), String> {
        log(job, ctx, "Opening MyOpenMath…");
        if poster::signed_in(t)? { return Ok(()); }
        if let Some((u, p)) = poster::saved_login() {
            log(job, ctx, "Signing in with your saved MyOpenMath login…");
            if poster::sign_in_with(t, &u, &p)? { return Ok(()); }
            log(job, ctx, "The saved login didn't work.");
        }
        t.show();
        log(job, ctx, "Please sign in to MyOpenMath in the Chrome window that opened. D.A.D continues as soon as you're in.");
        if poster::wait_signed_in(t, Duration::from_secs(600))? { Ok(()) } else { Err("Nobody signed in to MyOpenMath, so nothing was added.".into()) }
    }

    fn start_connect(st: &mut State, ctx: &egui::Context) {
        st.log.clear();
        st.poster.result = None;
        let job = start(st, "MyOpenMath");
        let ctx = ctx.clone();
        run_bg(async move {
            let r = tab().and_then(|mut t| { ensure_signed_in(&mut t, &job, &ctx)?; log(&job, &ctx, "Reading your courses…"); poster::courses(&mut t) });
            finish(&job, &ctx, Outcome::Courses(r));
        });
    }

    fn start_assessments(st: &mut State, ctx: &egui::Context, cid: u64) {
        st.log.clear();
        let job = start(st, "MyOpenMath");
        let ctx = ctx.clone();
        run_bg(async move {
            log(&job, &ctx, "Reading the assessments…");
            let r = tab().and_then(|mut t| poster::assessments(&mut t, cid));
            finish(&job, &ctx, Outcome::Assessments(cid, r));
        });
    }

    fn start_add(st: &mut State, ctx: &egui::Context, cid: u64, aid: u64) {
        let Some(q) = st.current.as_ref().map(|c| c.q.clone()) else { return };
        st.log.clear();
        st.poster.result = None;
        let save = st.poster.prefs.auto_save;
        let job = start(st, "MyOpenMath");
        let ctx = ctx.clone();
        run_bg(async move {
            let r = tab().and_then(|mut t| {
                ensure_signed_in(&mut t, &job, &ctx)?;
                let (j, c) = (job.clone(), ctx.clone());
                let r = poster::add_question(&mut t, cid, aid, &q, save, &mut |l| log(&j, &c, l));
                if r.is_ok() { t.show(); }
                r
            });
            finish(&job, &ctx, Outcome::Posted(r));
        });
    }

    pub fn courses_in(st: &mut State, r: Result<Vec<Course>, String>) {
        match r {
            Ok(c) => {
                st.poster.courses = c;
                if st.poster.courses.is_empty() { st.poster.result = Some(Err("No courses you teach were found on MyOpenMath.".into())); }
            }
            Err(e) => st.poster.result = Some(Err(e)),
        }
    }

    pub fn assessments_in(st: &mut State, cid: u64, r: Result<Vec<Assessment>, String>) {
        match r {
            Ok(a) => { st.poster.assessments = a; st.poster.assessments_for = Some(cid); }
            Err(e) => st.poster.result = Some(Err(e)),
        }
    }

    pub fn posted(st: &mut State, r: Result<Added, String>) {
        let name = st.poster.prefs.assessment
            .and_then(|a| st.poster.assessments.iter().find(|x| x.id == a)).map(|a| a.name.clone()).unwrap_or_default();
        st.poster.result = Some(match r {
            Ok(Added::Saved) => Ok(format!("Done. It's saved in \"{name}\" on MyOpenMath.")),
            Ok(Added::Filled) => Ok("It's filled in. Check it in the Chrome window, then press Save there.".into()),
            Err(e) => Err(e),
        });
    }

    pub fn card(ui: &mut Ui, ctx: &egui::Context, st: &mut State) {
        if !st.poster.loaded {
            st.poster.loaded = true;
            st.poster.prefs = crate::store::load_json(PREFS).unwrap_or_default();
        }
        let working = busy(st);
        let mine = st.job.lock().map(|j| j.what == "MyOpenMath").unwrap_or(false);
        pageshell::card(ui, |ui| {
            pageshell::heading(ui, "Add it to MyOpenMath for me");
            ui.add_space(4.0);
            if st.poster.courses.is_empty() {
                ui.add(egui::Label::new(theme::muted("D.A.D opens MyOpenMath in its own Chrome window and puts the question into the assessment you pick. The first time, sign in to MyOpenMath in that window; it remembers you after that.").size(13.0)).wrap());
                ui.add_space(8.0);
                if ui.add_enabled_ui(!working, |ui| pageshell::primary(ui, "Connect to MyOpenMath")).inner.clicked() {
                    start_connect(st, ctx);
                }
            } else {
                field_label(ui, "Course");
                let names: Vec<&str> = st.poster.courses.iter().map(|c| c.name.as_str()).collect();
                let mut k = st.poster.prefs.course.and_then(|id| st.poster.courses.iter().position(|c| c.id == id)).unwrap_or(usize::MAX);
                let shown = if k == usize::MAX { "Pick a course" } else { names[k] };
                let before = k;
                egui::ComboBox::from_id_salt("mom_post_course").selected_text(shown).width(340.0).height(520.0).show_ui(ui, |ui| {
                    for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut k, i, *n); }
                });
                if k != before && k != usize::MAX {
                    st.poster.prefs.course = Some(st.poster.courses[k].id);
                    st.poster.prefs.assessment = None;
                    st.poster.assessments.clear();
                    let _ = crate::store::save_json(PREFS, &st.poster.prefs);
                }
                if let Some(cid) = st.poster.prefs.course {
                    if st.poster.assessments_for != Some(cid) && !working {
                        st.poster.assessments_for = Some(cid);
                        start_assessments(st, ctx, cid);
                    }
                    field_label(ui, "Assessment");
                    if st.poster.assessments.is_empty() {
                        ui.label(theme::muted(if working { "Reading the assessments…" } else { "This course has no assessments yet. Make one in MyOpenMath first." }).size(13.0));
                    } else {
                        let names: Vec<&str> = st.poster.assessments.iter().map(|a| a.name.as_str()).collect();
                        let mut k = st.poster.prefs.assessment.and_then(|id| st.poster.assessments.iter().position(|a| a.id == id)).unwrap_or(usize::MAX);
                        let shown = if k == usize::MAX { "Pick an assessment" } else { names[k] };
                        let before = k;
                        egui::ComboBox::from_id_salt("mom_post_assess").selected_text(shown).width(340.0).height(520.0).show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut k, i, *n); }
                        });
                        if k != before && k != usize::MAX {
                            st.poster.prefs.assessment = Some(st.poster.assessments[k].id);
                            let _ = crate::store::save_json(PREFS, &st.poster.prefs);
                        }
                    }
                }
                ui.add_space(8.0);
                if ui.checkbox(&mut st.poster.prefs.auto_save, "Save it automatically (otherwise it stops so you can check it and press Save)").changed() {
                    let _ = crate::store::save_json(PREFS, &st.poster.prefs);
                }
                ui.add_space(8.0);
                let target = st.poster.prefs.course.zip(st.poster.prefs.assessment);
                if pageshell::primary_on(ui, "Add It Now", target.is_some() && !working).clicked() {
                    if let (Some((cid, aid)), false) = (target, working) { start_add(st, ctx, cid, aid); }
                }
                if target.is_none() { crate::kit::gated_reason(ui, "Pick a course and an assessment first."); }
            }
            if mine || working && !st.log.is_empty() {
                ui.add_space(8.0);
                for line in st.log.iter().rev().take(3).collect::<Vec<_>>().into_iter().rev() {
                    ui.horizontal(|ui| {
                        if working && mine { ui.spinner(); }
                        ui.add(egui::Label::new(theme::text(line).size(13.0)).wrap());
                    });
                }
            }
            match &st.poster.result {
                Some(Ok(m)) => { ui.add_space(6.0); ui.add(egui::Label::new(RichText::new(format!("✔ {m}")).color(theme::green()).size(14.0)).wrap()); }
                Some(Err(e)) => { ui.add_space(6.0); ui.add(egui::Label::new(RichText::new(e).color(theme::red()).size(14.0)).wrap()); }
                None => {}
            }
            ui.add_space(8.0);
            login_box(ui, st);
        });
    }

    /// The optional saved MyOpenMath sign-in, kept in the computer's keychain.
    fn login_box(ui: &mut Ui, st: &mut State) {
        let p = &mut st.poster;
        if p.has_login.is_none() { p.has_login = Some(poster::saved_login().is_some()); }
        egui::CollapsingHeader::new(theme::muted("Sign in automatically (optional)").size(13.0)).id_salt("mom_post_login").show(ui, |ui| {
            ui.add(egui::Label::new(theme::muted("For a plain MyOpenMath account (not Google or Clever sign-in). Saved in this computer's keychain; D.A.D types it into MyOpenMath's sign-in page and never shows it to the AI.").size(12.0)).wrap());
            if p.has_login == Some(true) {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("✔ A MyOpenMath login is saved.").color(theme::green()));
                    if ui.link("Forget it").clicked() {
                        p.result = Some(poster::store_login(None).map(|_| "Forgotten.".to_string()));
                        p.has_login = Some(false);
                    }
                });
                return;
            }
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut p.user).desired_width(180.0).hint_text("Username"));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "MyOpenMath username"));
                let r = ui.add(egui::TextEdit::singleline(&mut p.pass).password(true).desired_width(180.0).hint_text("Password"));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "MyOpenMath password"));
                let ok = !p.user.trim().is_empty() && !p.pass.is_empty();
                if pageshell::primary_on(ui, "Save Login", ok).clicked() && ok {
                    let r = poster::store_login(Some((p.user.trim(), &p.pass)));
                    p.pass.clear();
                    p.has_login = Some(r.is_ok());
                    p.result = Some(r.map(|_| "Login saved in the keychain.".to_string()));
                }
            });
        });
    }
}

/// "Add it to MyOpenMath for me" in the web app: the helper server's browser (src/mom/browserd.rs),
/// in a full-window panel the teacher watches and can take over (src/mom/browserpanel.js).
#[cfg(target_arch = "wasm32")]
mod webhelper {
    use super::*;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsValue;

    const PREFS: &str = "mom/helper.json";

    #[derive(Serialize, Deserialize, Clone)]
    #[serde(default)]
    pub struct Prefs { server: String, token: String, course: Option<u64>, assessment: Option<u64>, auto_save: bool }

    impl Default for Prefs {
        fn default() -> Prefs { Prefs { server: "ws://127.0.0.1:8793".into(), token: String::new(), course: None, assessment: None, auto_save: false } }
    }

    thread_local! { static PREFS_NOW: RefCell<Option<Prefs>> = const { RefCell::new(None) }; }

    fn prefs() -> Prefs {
        PREFS_NOW.with(|p| p.borrow_mut().get_or_insert_with(|| crate::store::load_json(PREFS).unwrap_or_default()).clone())
    }

    fn update(f: impl FnOnce(&mut Prefs)) {
        PREFS_NOW.with(|p| {
            let mut b = p.borrow_mut();
            let v = b.get_or_insert_with(|| crate::store::load_json(PREFS).unwrap_or_default());
            f(v);
            let _ = crate::store::save_json(PREFS, v);
        });
    }

    fn open(src: &str, name: &str) -> Result<(), JsValue> {
        let w = web_sys::window().ok_or("no window")?;
        if js_sys::Reflect::get(&w, &"dadBrowser".into())?.is_undefined() {
            js_sys::eval(include_str!("mom/browserpanel.js"))?;
        }
        let p = prefs();
        let o = js_sys::Object::new();
        let set = |k: &str, v: JsValue| js_sys::Reflect::set(&o, &k.into(), &v);
        set("server", p.server.clone().into())?;
        set("token", p.token.clone().into())?;
        set("question", src.into())?;
        set("name", name.into())?;
        set("save", p.auto_save.into())?;
        if let Some(c) = p.course { set("course", (c as f64).into())?; }
        if let Some(a) = p.assessment { set("assessment", (a as f64).into())?; }
        set("onSave", Closure::<dyn FnMut(bool)>::new(|b: bool| update(|p| p.auto_save = b)).into_js_value())?;
        set("onTarget", Closure::<dyn FnMut(f64, f64)>::new(|c: f64, a: f64| update(|p| {
            p.course = Some(c as u64);
            if a > 0.0 { p.assessment = Some(a as u64); }
        })).into_js_value())?;
        let helper = js_sys::Reflect::get(&w, &"dadBrowser".into())?;
        let f: js_sys::Function = js_sys::Reflect::get(&helper, &"open".into())?.into();
        f.call1(&helper, &o)?;
        Ok(())
    }

    pub fn card(ui: &mut Ui, st: &mut State) {
        let mut p = prefs();
        pageshell::card(ui, |ui| {
            pageshell::heading(ui, "Add it to MyOpenMath for me");
            ui.add_space(4.0);
            ui.add(egui::Label::new(theme::muted("The MyOpenMath helper opens MyOpenMath in a window here and puts the question into the assessment you pick. You can watch it work and take over at any time; the first time, you sign in to MyOpenMath in that window.").size(13.0)).wrap());
            ui.add_space(8.0);
            let ready = !p.server.trim().is_empty() && p.token.trim().len() >= 8;
            if pageshell::primary_on(ui, "Open the MyOpenMath Helper", ready).clicked() && ready {
                if let Some(c) = st.current.as_ref() {
                    if let Err(e) = open(&c.q.emit(), &c.q.name) {
                        st.note = Some(format!("The helper could not open: {e:?}"));
                    }
                }
            }
            if !ready { crate::kit::gated_reason(ui, "Enter the helper's access code below first."); }
            ui.add_space(6.0);
            egui::CollapsingHeader::new(theme::muted("Helper settings").size(13.0)).id_salt("mom_helper_cfg").default_open(!ready).show(ui, |ui| {
                field_label(ui, "Access code");
                let r = ui.add(egui::TextEdit::singleline(&mut p.token).password(true).desired_width(260.0).hint_text("From whoever runs your helper"));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Helper access code"));
                let changed_token = r.changed();
                field_label(ui, "Helper address");
                let r = ui.add(egui::TextEdit::singleline(&mut p.server).desired_width(260.0));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Helper address"));
                if changed_token || r.changed() {
                    let (server, token) = (p.server.clone(), p.token.clone());
                    update(|q| { q.server = server; q.token = token; });
                }
            });
        });
    }
}
