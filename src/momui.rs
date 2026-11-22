//! The Write screen, for teachers who never see code: three steps.
//!
//! 1. Describe it: course, topic, answer type per part, difficulty, rounding and a few options, all
//!    dropdowns and checkboxes (src/mom/form.rs turns them into the AI's request). While the AI
//!    writes, progress is in plain words.
//! 2. Try it: the question exactly as students see it (the engine's MyOpenMath page: full-window in
//!    the browser build or in D.A.D's native workspace), then approve it or use a "change it"
//!    dropdown that rewrites it.
//! 3. Add to MyOpenMath: numbered steps with one copy button per editor box, and the bookmarklet.
//!
//! My Questions keeps approved questions. The code, the raw log and the checker's address sit behind
//! "Show advanced tools", off by default. Errors are plain sentences with the details folded away.

use crate::mom::author::{author_async_logged, post_engine, HttpEngine, ModelChat, AuthorEngine};
use crate::mom::engine::{SelftestResponse, DEFAULT_SEEDS};
use crate::mom::form::{self, Answer, Change, Difficulty, Form, Rounding, Topic, AI_CHOOSES, COURSES, MAX_PARTS};
use crate::mom::MomQuestion;
use crate::mom::worksheet;
#[cfg(not(target_arch = "wasm32"))]
use crate::mom::student_text::{StudentAction, StudentText};
use crate::pageshell;
use crate::platform::PickedFile;
use crate::theme;
use egui::{RichText, Ui};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

/// The browser build runs the engine in the page itself; the desktop runs its own (no Docker).
#[cfg(target_arch = "wasm32")]
pub const DEFAULT_ENGINE: &str = crate::mom::author::BUNDLED_ENGINE;
#[cfg(not(target_arch = "wasm32"))]
pub const DEFAULT_ENGINE: &str = crate::mom::localengine::LOCAL;
const LIBRARY: &str = "mom/library.json";
const FORM: &str = "mom/form.json";
const ADVANCED: &str = "mom/advanced.json";
const AI_PICKED: &str = "mom/ai_picked.json";
const MAX_TRIES: usize = 4;

pub fn export_my_questions() -> Result<(), String> {
    let mut library: Library = crate::store::load_json(LIBRARY).unwrap_or_default();
    library.worksheet = None;
    if let Some(path) = crate::platform::save_path("dad-my-questions.json", "D.A.D questions", &["json"]) {
        let data = serde_json::to_vec_pretty(&library).map_err(|error| error.to_string())?;
        crate::platform::write(&path, &data).map_err(|error| error.to_string())?;
    }
    Ok(())
}

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub untested: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worksheet_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worksheet_origin: Option<WorksheetOrigin>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worksheet: Option<WorksheetBatch>,
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
    worksheet_source: Option<String>,
    /// The saved entry this came from or was saved as.
    saved_id: Option<String>,
    /// The app opened the student player for this version.
    preview_opened: bool,
    /// The teacher confirmed this version in the student player.
    preview_confirmed: bool,
    /// The teacher tried it in the sandbox and approved it; copying into MyOpenMath is locked until then.
    reviewed: bool,
}

enum Outcome {
    Authored { q: Option<MomQuestion>, passed: bool, problems: Vec<String>, untested: Vec<usize>, error: Option<String> },
    WorksheetRead { questions: Vec<worksheet::SourceQuestion>, notes: Vec<String>, retry_files: Vec<PickedFile> },
    WorksheetItem(WorksheetOrigin, Result<Saved, WorksheetFailure>),
    WorksheetFinished,
    Tested(Result<SelftestResponse, String>, /* then save */ bool),
    Preview(Result<String, String>),
    #[cfg(not(target_arch = "wasm32"))]
    StudentBrowser(Result<String, String>),
    #[cfg(not(target_arch = "wasm32"))]
    StudentText(Result<StudentText, String>),
    TestedAll(Vec<(String, Result<SelftestResponse, String>)>),
    /// The helper browser is signed in to MyOpenMath: the teacher's courses.
    #[cfg(not(target_arch = "wasm32"))]
    Courses(Result<Vec<crate::mom::poster::Course>, String>),
    #[cfg(not(target_arch = "wasm32"))]
    Assessments(u64, Result<Vec<crate::mom::poster::Assessment>, String>),
    #[cfg(not(target_arch = "wasm32"))]
    Posted { result: Result<crate::mom::poster::Added, String>, preview: bool },
}

#[derive(Default)]
struct Job {
    busy: bool,
    what: &'static str,
    pause_after_current: bool,
    log: Vec<String>,
    /// Finished results not yet taken by the screen. A list, so a job starting on the frame another
    /// finished cannot drop the first one's result.
    outcomes: Vec<Outcome>,
}

#[derive(PartialEq, Clone, Copy, Default)]
enum Step { #[default] Describe, Try, Copy }

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorksheetOrigin {
    batch: String,
    number: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original: Option<worksheet::OriginalPage>,
}

#[derive(Clone, Serialize, Deserialize)]
struct WorksheetProblem {
    source: worksheet::SourceQuestion,
    origin: WorksheetOrigin,
}

#[derive(Serialize, Deserialize)]
struct WorksheetFailure {
    problem: WorksheetProblem,
    error: String,
}

#[derive(Serialize, Deserialize)]
struct WorksheetBatch {
    id: String,
    pending: Vec<WorksheetProblem>,
    notes: Vec<String>,
    failures: Vec<WorksheetFailure>,
}

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
    worksheet_mode: bool,
    worksheet_files: Vec<PickedFile>,
    worksheet_save_error: Option<String>,
    scroll_to_question: bool,
    form: Form,
    change: Change,
    change_other: String,
    /// Show the code, the raw log and the checker's address.
    advanced: bool,
    preview_html: Option<String>,
    preview_for: String,
    library: Library,
    jump_library: bool,
    /// What happened to the saved questions (saved, tested), shown in their card.
    lib_note: Option<String>,
    confirm_delete: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    poster: autoadd::Poster,
    #[cfg(not(target_arch = "wasm32"))]
    browser: crate::mom::desktop_browser::Panel,
    #[cfg(not(target_arch = "wasm32"))]
    focus_student_review: bool,
    #[cfg(not(target_arch = "wasm32"))]
    student_text_mode: bool,
    #[cfg(not(target_arch = "wasm32"))]
    student_url: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    student_text: Option<StudentText>,
    /// The "Your AI" card is open for choosing.
    ai_open: bool,
    ai_key: String,
    ai_model_pick: usize,
    /// A choice made in the card, applied to the app's settings after the frame.
    ai_pick: Option<AiPick>,
    /// "Keep my current AI" was pressed.
    ai_keep: bool,
}

/// An AI picked in the "Your AI" card.
struct AiPick { provider: &'static str, model: String, key: Option<String> }

thread_local! {
    static ST: RefCell<State> = RefCell::new(State::default());
}

pub(crate) fn browser_workspace_open() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    { ST.with(|state| state.borrow().browser.surface().is_some()) }
    #[cfg(target_arch = "wasm32")]
    { false }
}

/// Called every frame from the screen router; hides the browser build's preview when another
/// screen is showing.
pub fn leave() {
    #[cfg(target_arch = "wasm32")]
    overlay::hide();
    ST.with(|state| {
        let mut state = state.borrow_mut();
        if state.inited { collect(&mut state); }
        #[cfg(not(target_arch = "wasm32"))]
        if state.browser.surface().is_some() { state.browser.close(); }
    });
}

/// Stage a worksheet delivered by the host's open-file handler.
pub fn open_worksheet(name: &str, bytes: Vec<u8>) {
    ST.with(|state| {
        let mut state = state.borrow_mut();
        #[cfg(not(target_arch = "wasm32"))]
        state.browser.close();
        state.worksheet_files.push(PickedFile { name: name.into(), bytes });
        state.worksheet_mode = true;
        if !busy(&state) { state.step = Step::Describe; }
    });
}

impl crate::App {
    pub fn draw_mom(&mut self, ctx: &egui::Context) {
        let mut st = ST.with(|s| std::mem::take(&mut *s.borrow_mut()));
        if !st.inited {
            st.inited = true;
            st.engine = crate::store::load_json::<String>("mom/engine.json").unwrap_or_else(|| DEFAULT_ENGINE.to_string());
            if st.engine == crate::mom::author::BUNDLED_ENGINE { crate::mom::author::warm_bundled_engine(); }
            st.library = crate::store::load_json(LIBRARY).unwrap_or_default();
            st.worksheet_mode |= st.library.worksheet.is_some();
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
                    if let Some(c) = st.current.as_mut() { c.reviewed = true; c.preview_opened = true; }
                }
                start_test(&mut st, ctx, false);
            }
        }
        collect(&mut st);
        let cfg = crate::ai::AiCfg::from(&self.settings);
        // Asked once: until the teacher picks an AI (or keeps the one set up), the card stays open.
        let chosen = crate::store::load_json::<bool>(AI_PICKED).unwrap_or(false);
        #[cfg(not(target_arch = "wasm32"))]
        let browser_open = st.browser.surface().is_some();
        #[cfg(target_arch = "wasm32")]
        let browser_open = false;
        crate::screens::shell_frame(self, ctx, !browser_open, |_app, ui| {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(surface) = st.browser.surface() {
                browser_workspace(ui, &mut st, surface);
                return;
            }
            page(ui, ctx, &mut st, &cfg, chosen);
        });
        if let Some(p) = st.ai_pick.take() {
            self.settings.ai_provider = p.provider.to_string();
            self.settings.ai_model = p.model;
            let _ = crate::store::save_json("settings.json", &self.settings);
            if let Some(k) = p.key {
                st.note = Some(match crate::ai::set_key(p.provider, Some(k.trim())) { Ok(()) => "AI saved.".into(), Err(e) => format!("Could not save the key: {e}") });
            }
            st.ai_open = false;
            st.error = None;
            let _ = crate::store::save_json(AI_PICKED, &true);
        }
        if std::mem::take(&mut st.ai_keep) {
            let _ = crate::store::save_json(AI_PICKED, &true);
            st.ai_open = false;
        }
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
        Outcome::Posted { result, preview } => {
            if preview && result.is_err() { st.browser.close(); }
            autoadd::posted(st, result);
        }
        Outcome::Authored { q, passed, problems, untested, error } => {
            st.error = error;
            if let Some(mut q) = q {
                // A question needs a name (MyOpenMath's Description); a model sometimes leaves it out.
                if q.name.trim().is_empty() {
                    q.name = st.form.short_name();
                }
                // A rewrite keeps the question's place in My Questions.
                let saved_id = st.current.as_ref().and_then(|c| c.saved_id.clone());
                let worksheet_source = st.current.as_ref().and_then(|c| c.worksheet_source.clone());
                st.current = Some(Current { q, request: st.request.clone(), passed: Some(passed), problems, untested, saved_id, worksheet_source, ..Default::default() });
                st.step = Step::Try;
                st.preview_html = None;
                st.note = None;
            }
        }
        Outcome::WorksheetRead { questions, mut notes, retry_files } => {
            let id = crate::model::new_id();
            let pending: Vec<_> = questions.into_iter().enumerate().map(|(index, mut source)| WorksheetProblem {
                origin: WorksheetOrigin { batch: id.clone(), number: index + 1, original: source.original.take() },
                source,
            }).collect();
            if pending.is_empty() && notes.is_empty() {
                notes.push("No problems were found in this worksheet.".into());
            }
            st.library.worksheet = Some(WorksheetBatch { id, pending, notes, failures: Vec::new() });
            st.worksheet_files.extend(retry_files);
            save_worksheet(st);
        }
        Outcome::WorksheetItem(origin, item) => {
            if record_worksheet_item(&mut st.library, origin, item) { save_worksheet(st); }
        }
        Outcome::WorksheetFinished => {
            st.scroll_to_question = true;
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
        #[cfg(not(target_arch = "wasm32"))]
        Outcome::StudentBrowser(result) => match result {
            Ok(url) => {
                if st.browser.surface() == Some(crate::mom::desktop_browser::Surface::Student) {
                    st.student_url = Some(url);
                    if let Some(current) = st.current.as_mut() { current.preview_opened = true; }
                }
            }
            Err(error) => st.error = Some(error),
        },
        #[cfg(not(target_arch = "wasm32"))]
        Outcome::StudentText(result) => match result {
            Ok(text) if st.student_text_mode => st.student_text = Some(text),
            Ok(_) => {}
            Err(error) => st.error = Some(error),
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
                    s.untested = rep.untested_parts;
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
        j.pause_after_current = false;
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

fn save_worksheet(st: &mut State) {
    st.worksheet_save_error = crate::store::save_json(LIBRARY, &st.library).err();
}

fn record_worksheet_item(library: &mut Library, origin: WorksheetOrigin, item: Result<Saved, WorksheetFailure>) -> bool {
    let Some(batch) = library.worksheet.as_mut().filter(|batch| batch.id == origin.batch) else { return false };
    let before = batch.pending.len() + batch.failures.len();
    batch.pending.retain(|problem| problem.origin.number != origin.number);
    batch.failures.retain(|failure| failure.problem.origin.number != origin.number);
    if before == batch.pending.len() + batch.failures.len() { return false; }
    match item {
        Ok(draft) => library.questions.insert(0, draft),
        Err(failure) => {
            let index = batch.failures.partition_point(|entry| entry.problem.origin.number < origin.number);
            batch.failures.insert(index, failure);
        }
    }
    true
}

fn read_worksheet(st: &mut State, ctx: &egui::Context, cfg: &crate::ai::AiCfg) {
    let files = std::mem::take(&mut st.worksheet_files);
    st.current = None;
    st.preview_html = None;
    st.log.clear();
    let job = start(st, "Reading worksheet");
    let (cfg, ctx) = (cfg.clone(), ctx.clone());
    run_bg(async move {
        let (questions, notes) = worksheet::read(&files, &cfg, |line| {
            if let Ok(mut j) = job.lock() { j.log.push(line); }
            ctx.request_repaint();
        }).await;
        let retry_files = if questions.is_empty() { files } else { Vec::new() };
        finish(&job, &ctx, Outcome::WorksheetRead { questions, notes, retry_files });
    });
}

fn convert_worksheet(st: &mut State, ctx: &egui::Context, cfg: &crate::ai::AiCfg, questions: Vec<WorksheetProblem>) {
    st.log.clear();
    let job = start(st, "Converting worksheet");
    let (engine, cfg, ctx) = (st.engine.clone(), cfg.clone(), ctx.clone());
    run_bg(async move {
        let progress = |line| {
            if let Ok(mut j) = job.lock() { j.log.push(line); }
            ctx.request_repaint();
        };
        let total = questions.len();
        let mut chat = ModelChat { cfg: &cfg, tools: false };
        let mut engine = HttpEngine { base: engine };
        for (index, question) in questions.into_iter().enumerate() {
            if job.lock().is_ok_and(|job| job.pause_after_current) { break; }
            progress(format!("Converting and checking question {} of {total}…", index + 1));
            let request = question.source.request();
            let run = author_async_logged(&request, &mut chat, &mut engine, MAX_TRIES, |_| {}).await;
            let origin = question.origin.clone();
            let item = if let Some(mut q) = run.question {
                if q.name.trim().is_empty() { q.name = format!("Worksheet problem {}", origin.number); }
                let mut problems = run.problems;
                problems.extend(run.error);
                Ok(Saved {
                    id: crate::model::new_id(),
                    request,
                    src: q.emit(),
                    passed: run.passed,
                    problems,
                    untested: run.report.map_or_else(Vec::new, |report| report.untested_parts),
                    worksheet_source: Some(question.source.location),
                    worksheet_origin: Some(origin.clone()),
                    reviewed: false,
                    saved_at: timestamp(),
                })
            } else {
                let error = run.error.or_else(|| run.problems.into_iter().next())
                    .unwrap_or_else(|| "The AI did not produce a MyOpenMath question.".into());
                Err(WorksheetFailure { problem: question, error })
            };
            if let Ok(mut job) = job.lock() { job.outcomes.push(Outcome::WorksheetItem(origin, item)); }
            ctx.request_repaint();
        }
        finish(&job, &ctx, Outcome::WorksheetFinished);
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
    st.preview_html = None;
    st.preview_for = src.clone();
    let job = start(st, "Drawing the preview");
    let (engine, ctx) = (st.engine.clone(), ctx.clone());
    run_bg(async move {
        // The page's own Check Answers / New Version calls go to the same engine: "bundled" makes the
        // page ask the app's window.dadEngine instead of fetching a server.
        // MyOpenMath's own student player (assess2), answered by the engine: the same page, score
        // box, "Get a similar question" and "Show Detailed Solution" students see.
        // The page talks to the engine by its real address ("local" is started here if needed).
        #[cfg(not(target_arch = "wasm32"))]
        let engine = match crate::mom::author::resolve_engine(&engine) { Ok(e) => e, Err(e) => { finish(&job, &ctx, Outcome::Preview(Err(e))); return; } };
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

fn timestamp() -> u64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn save_current(st: &mut State) {
    let Some(c) = st.current.as_mut() else { return };
    let worksheet_origin = st.library.questions.iter().find(|saved| Some(&saved.id) == c.saved_id.as_ref())
        .and_then(|saved| saved.worksheet_origin.clone());
    let entry = Saved {
        id: c.saved_id.clone().unwrap_or_else(crate::model::new_id),
        request: c.request.clone(),
        src: c.q.emit(),
        passed: c.passed == Some(true),
        problems: c.problems.clone(),
        untested: c.untested.clone(),
        worksheet_source: c.worksheet_source.clone(),
        worksheet_origin,
        reviewed: c.reviewed,
        saved_at: timestamp(),
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

fn page(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg, chosen: bool) {
    pageshell::title(ui, "", "MyOpenMath Questions",
        "Create questions or convert a worksheet, then try the questions before adding them to MyOpenMath.");
    ui.add_space(8.0);
    if st.step == Step::Describe {
        ui.add_enabled_ui(!busy(st), |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut st.worksheet_mode, false, "Create a question");
                ui.selectable_value(&mut st.worksheet_mode, true, "Convert a worksheet");
            });
        });
        ui.add_space(8.0);
    }
    steps(ui, st);
    ui.add_space(12.0);
    let wide = ui.available_width() >= 900.0;
    if !wide && ui.link("My Questions").clicked() {
        st.jump_library = true;
    }
    if wide {
        let side_w = 320.0;
        ui.horizontal_top(|ui| {
            let main_w = ui.available_width() - side_w - 16.0;
            ui.allocate_ui_with_layout(egui::vec2(main_w, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().id_salt("mom_main").show(ui, |ui| main(ui, ctx, st, cfg, chosen));
            });
            ui.add_space(16.0);
            ui.allocate_ui_with_layout(egui::vec2(side_w, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().id_salt("mom_side").show(ui, |ui| side(ui, ctx, st));
            });
        });
    } else {
        egui::ScrollArea::vertical().id_salt("mom_stack").show(ui, |ui| {
            main(ui, ctx, st, cfg, chosen);
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
        for (i, (step, name, open)) in [(Step::Describe, if st.worksheet_mode { "Upload" } else { "Describe it" }, true), (Step::Try, "Try it", has_q), (Step::Copy, "Add to MyOpenMath", reviewed)].into_iter().enumerate() {
            if i > 0 { ui.label(theme::muted("›").size(18.0)); }
            let label = format!("{}   {name}", i + 1);
            let r = ui.add_enabled_ui(open && !busy(st), |ui| step_pill(ui, &label, st.step == step)).inner;
            if r.clicked() { st.step = step; }
        }
        if has_q && !busy(st) {
            ui.add_space(12.0);
            if pageshell::outline(ui, "Start a New Question").on_hover_text("Your choices below stay picked").clicked() {
                st.step = Step::Describe;
                st.worksheet_mode = false;
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

fn main(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg, chosen: bool) {
    if busy(st) && st.job.lock().map(|j| j.what == "Writing").unwrap_or(false) {
        leave();
        writing(ui, st);
    } else {
        match st.step {
            Step::Describe => {
                leave();
                if !chosen {
                    ai_card(ui, st, cfg, chosen);
                    ui.add_space(12.0);
                }
                if st.worksheet_mode { worksheet_form(ui, ctx, st, cfg); } else { describe(ui, ctx, st, cfg); }
                if chosen {
                    ui.add_space(12.0);
                    ai_card(ui, st, cfg, chosen);
                }
            }
            Step::Try => try_it(ui, ctx, st, cfg),
            Step::Copy => { leave(); copy_step(ui, ctx, st) }
        }
    }
    ui.add_space(12.0);
    advanced(ui, ctx, st, cfg);
}

fn worksheet_form(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    st.worksheet_files.extend(crate::platform::take_picked());
    let working = busy(st);
    let can_pick = st.library.worksheet.as_ref().is_none_or(|batch| batch.pending.is_empty() && batch.failures.is_empty());
    let mut save = false;
    let mut convert = None;
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, "Upload a math or science worksheet");
        ui.add(egui::Label::new(theme::muted("PDF, Word, text, or photos. Upload Word worksheets with equations or diagrams as PDFs.").size(13.0)).wrap());
        ui.add(egui::Label::new(theme::muted("D.A.D keeps a local copy of the pages it reads so you can compare them with the drafts.").size(13.0)).wrap());
        ui.add_space(8.0);
        if working {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::text(st.log.last().map(String::as_str).unwrap_or("Opening worksheet…")));
            });
            if let Ok(mut job) = st.job.lock() && job.what == "Converting worksheet" {
                if job.pause_after_current {
                    ui.label("Pausing after this problem…");
                } else if pageshell::outline(ui, "Pause after this problem").clicked() {
                    job.pause_after_current = true;
                }
            }
            return;
        }
        if can_pick && ui.button("Choose worksheet files").clicked() {
            if let Some(paths) = crate::platform::pick_files("Math or science worksheets", worksheet::EXTENSIONS) {
                for path in paths {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    match crate::platform::read(&path) {
                        Ok(bytes) => st.worksheet_files.push(PickedFile { name, bytes }),
                        Err(error) => st.error = Some(format!("Could not open {name}: {error}")),
                    }
                }
            }
        }
        for file in &st.worksheet_files {
            ui.label(theme::text(&file.name));
        }
        if can_pick && !st.worksheet_files.is_empty() {
            ui.horizontal_wrapped(|ui| {
                if pageshell::primary(ui, "Read worksheet").clicked() {
                    read_worksheet(st, ctx, cfg);
                }
                if ui.link("Clear files").clicked() { st.worksheet_files.clear(); }
            });
        }
        if let Some(batch) = st.library.worksheet.as_mut() {
            let count = batch.pending.iter().filter(|problem| !problem.source.text().is_empty()).count().min(20);
            if !batch.pending.is_empty() {
                ui.label(theme::strong(format!("{} worksheet problems left to convert", batch.pending.len())));
                ui.horizontal_wrapped(|ui| {
                    if pageshell::primary_on(ui, &format!("Create {count} MyOpenMath drafts"), count > 0).clicked() {
                        convert = Some(count);
                    }
                    if ui.link("Discard remaining problems").clicked() {
                        batch.pending.clear();
                        batch.notes.clear();
                        save = true;
                    }
                });
                if count == 0 { ui.label("Enter the text of at least one problem to create a draft."); }
                egui::CollapsingHeader::new("Check extracted problems")
                    .id_salt("pending_worksheet_problems").show(ui, |ui| {
                        for problem in &mut batch.pending {
                            let title = format!("Problem {} · {}", problem.origin.number, problem.source.location);
                            egui::CollapsingHeader::new(title).id_salt((&batch.id, problem.origin.number)).show(ui, |ui| {
                                if let Some(original) = &problem.origin.original {
                                    egui::CollapsingHeader::new("Original worksheet page").show(ui, |ui| original.draw(ui, 240.0));
                                }
                                let label = ui.label("Extracted problem");
                                let edit = ui.add(egui::TextEdit::multiline(&mut problem.source.text)
                                    .desired_width(f32::INFINITY).desired_rows(3)).labelled_by(label.id);
                                save |= edit.changed();
                            });
                        }
                    });
            }
            for note in &batch.notes { ui.add(egui::Label::new(theme::muted(note).size(13.0)).wrap()); }
        }
        if let Some(error) = &st.error {
            ui.add(egui::Label::new(RichText::new(error).color(theme::red())).wrap());
        }
        if let Some(error) = &st.worksheet_save_error {
            ui.add(egui::Label::new(RichText::new(format!("Couldn't save worksheet progress: {error}")).color(theme::red())).wrap());
            save |= pageshell::outline(ui, "Retry saving progress").clicked();
        }
    });
    if working { return; }
    if save { save_worksheet(st); }
    if let Some(count) = convert && let Some(batch) = &st.library.worksheet {
        let questions = batch.pending.iter().filter(|problem| !problem.source.text().is_empty())
            .take(count).cloned().collect::<Vec<_>>();
        if !questions.is_empty() {
            convert_worksheet(st, ctx, cfg, questions);
            return;
        }
    }

    let Some(batch) = &st.library.worksheet else { return };
    let mut retry = None;
    let mut discard = None;
    if !batch.failures.is_empty() {
        ui.add_space(12.0);
        pageshell::card(ui, |ui| {
            pageshell::heading(ui, "Problems without a draft");
            for (index, failure) in batch.failures.iter().enumerate() {
                ui.push_id(failure.problem.origin.number, |ui| {
                    ui.add(egui::Label::new(theme::strong(format!("Problem {} · {}", failure.problem.origin.number, failure.problem.source.location))).wrap());
                    ui.add(egui::Label::new(theme::text(failure.problem.source.text())).wrap());
                    ui.add(egui::Label::new(theme::muted(&failure.error)).wrap());
                    ui.horizontal_wrapped(|ui| {
                        let retry_button = pageshell::outline(ui, "Retry conversion");
                        retry_button.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true,
                            format!("Retry conversion for problem {}", failure.problem.origin.number)));
                        if retry_button.clicked() { retry = Some(index); }
                        if ui.link("Skip problem").clicked() { discard = Some(index); }
                    });
                    ui.separator();
                });
            }
        });
    }
    if let Some(index) = retry {
        let problem = batch.failures[index].problem.clone();
        convert_worksheet(st, ctx, cfg, vec![problem]);
        return;
    }
    if let Some(index) = discard {
        if let Some(batch) = st.library.worksheet.as_mut() { batch.failures.remove(index); }
        save_worksheet(st);
    }

    if let Some(batch) = &st.library.worksheet {
        let drafts = worksheet_drafts(&st.library, &batch.id);
        if !drafts.is_empty() {
            let mut open = None;
            ui.add_space(12.0);
            pageshell::card(ui, |ui| {
                pageshell::heading(ui, "Review worksheet drafts");
                let approved = drafts.iter().filter(|(_, draft)| draft.reviewed).count();
                ui.label(theme::muted(format!("{approved} of {} drafts approved", drafts.len())));
                if let Some((_, next)) = drafts.iter().find(|(_, draft)| !draft.reviewed)
                    && pageshell::primary(ui, "Continue reviewing").clicked()
                {
                    open = Some((*next).clone());
                }
                egui::CollapsingHeader::new("All worksheet drafts").show(ui, |ui| for (number, draft) in &drafts {
                    ui.push_id(&draft.id, |ui| {
                        let title = MomQuestion::parse(&draft.src).name;
                        let response = ui.link(format!("Problem {number} · {title}"));
                        if response.clicked() { open = Some((*draft).clone()); }
                        ui.label(theme::muted(saved_status(draft)).size(13.0));
                    });
                });
            });
            if let Some(saved) = open { open_worksheet_draft(st, saved); }
        }
    }
}

fn worksheet_drafts<'a>(library: &'a Library, batch: &str) -> Vec<(usize, &'a Saved)> {
    let mut drafts: Vec<_> = library.questions.iter().filter_map(|draft| {
        let origin = draft.worksheet_origin.as_ref()?;
        (origin.batch == batch).then_some((origin.number, draft))
    }).collect();
    drafts.sort_by_key(|(number, _)| *number);
    drafts
}

fn open_worksheet_draft(st: &mut State, saved: Saved) {
    open_saved(st, saved);
    st.step = Step::Try;
}

fn worksheet_navigation(ui: &mut Ui, st: &mut State) -> bool {
    let Some(id) = st.current.as_ref().and_then(|current| current.saved_id.as_deref()) else { return false };
    let Some(origin) = st.library.questions.iter().find(|saved| saved.id == id)
        .and_then(|saved| saved.worksheet_origin.as_ref()) else { return false };
    let drafts = worksheet_drafts(&st.library, &origin.batch);
    let Some(position) = drafts.iter().position(|(_, draft)| draft.id == id) else { return false };
    let mut open = None;
    let mut back = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::strong(format!("Worksheet problem {} · draft {} of {}", origin.number, position + 1, drafts.len())));
        if let Some(previous) = position.checked_sub(1).and_then(|index| drafts.get(index))
            && pageshell::outline(ui, "Previous draft").clicked()
        {
            open = Some(previous.1.clone());
        }
        if let Some(next) = drafts.get(position + 1)
            && pageshell::outline(ui, "Next draft").clicked()
        {
            open = Some(next.1.clone());
        }
        if st.library.worksheet.as_ref().is_some_and(|batch| batch.id == origin.batch) {
            back = ui.link("Back to worksheet").clicked();
        }
    });
    ui.add_space(8.0);
    if let Some(saved) = open {
        open_worksheet_draft(st, saved);
        ui.scroll_to_cursor(Some(egui::Align::Min));
        ui.ctx().request_repaint();
        return true;
    }
    if back {
        st.worksheet_mode = true;
        st.step = Step::Describe;
        ui.scroll_to_cursor(Some(egui::Align::Min));
        ui.ctx().request_repaint();
    }
    back
}

fn saved_status(saved: &Saved) -> &'static str {
    if !saved.passed { "Needs fixing" }
    else if saved.reviewed { "Approved" }
    else { "Needs review" }
}

/// The AI as a teacher would name it.
fn ai_label(cfg: &crate::ai::AiCfg) -> String {
    let model = cfg.model.trim();
    match cfg.provider.as_str() {
        "claude-cli" => format!("Claude on this computer{}", if model.is_empty() { String::new() } else { format!(" ({})", model) }),
        "codex-cli" => "Codex on this computer".into(),
        "openrouter" => format!("OpenRouter: {model}"),
        "ollama" => format!("Ollama on this computer: {model}"),
        "cloud" => format!("Ollama Cloud: {model}"),
        "account" => "D.A.D's own AI".into(),
        _ => format!("{}: {model}", cfg.provider),
    }
}

/// OpenRouter models offered in the card: (label, model id).
const OPENROUTER_PICKS: [(&str, &str); 3] = [
    ("DeepSeek v4.1 Flash (best tested, well under a cent per question)", "deepseek/deepseek-v4.1-flash"),
    ("Claude Sonnet 5.5 (best writing, costs more)", "anthropic/claude-sonnet-5.5"),
    ("Free models (slower and uneven)", "openrouter/free"),
];
const CLAUDE_PICKS: [(&str, &str); 3] = [("Sonnet (recommended)", "sonnet"), ("Opus (slower, strongest)", "opus"), ("Haiku (fastest)", "haiku")];

/// "Your AI": which AI writes the questions, picked in plain words. AI tools found on this computer
/// come first (no key needed); an OpenRouter key works anywhere.
fn ai_card(ui: &mut Ui, st: &mut State, cfg: &crate::ai::AiCfg, chosen: bool) {
    #[cfg(not(target_arch = "wasm32"))]
    let local = crate::ai::cli::installed();
    #[cfg(target_arch = "wasm32")]
    let local: Vec<&'static str> = Vec::new();
    let open = st.ai_open || !chosen;
    pageshell::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::strong("Your AI").size(15.0));
            ui.label(theme::text(if chosen { ai_label(cfg) } else { "not picked yet".to_string() }).size(14.0));
            if chosen && !open && ui.link("Change").clicked() { st.ai_open = true; }
        });
        if !open { return; }
        ui.add_space(6.0);
        ui.add(egui::Label::new(theme::muted("Pick which AI writes your questions. An AI already on this computer needs no key.").size(13.0)).wrap());
        ui.add_space(6.0);
        if local.contains(&"claude-cli") {
            ui.horizontal_wrapped(|ui| {
                let labels: Vec<&str> = CLAUDE_PICKS.iter().map(|p| p.0).collect();
                let mut k = st.ai_model_pick.min(CLAUDE_PICKS.len() - 1);
                if pageshell::primary(ui, "Use Claude on This Computer").clicked() {
                    st.ai_pick = Some(AiPick { provider: "claude-cli", model: CLAUDE_PICKS[k].1.into(), key: None });
                }
                if dropdown(ui, "mom_ai_claude", &labels, &mut k, 230.0) { st.ai_model_pick = k; }
            });
            ui.label(theme::muted("Uses your Claude subscription through Claude Code.").size(12.0));
            ui.add_space(6.0);
        }
        if local.contains(&"codex-cli") {
            if pageshell::outline(ui, "Use Codex on This Computer").clicked() {
                st.ai_pick = Some(AiPick { provider: "codex-cli", model: String::new(), key: None });
            }
            ui.label(theme::muted("Uses your ChatGPT subscription through Codex.").size(12.0));
            ui.add_space(6.0);
        }
        egui::CollapsingHeader::new(theme::strong("Use an OpenRouter key").size(14.0)).id_salt("mom_ai_or").default_open(local.is_empty()).show(ui, |ui| {
            ui.add(egui::Label::new(theme::muted("Make a free account at openrouter.ai, add a few dollars, create a key, and paste it here. It's saved on this computer only.").size(12.0)).wrap());
            let labels: Vec<&str> = OPENROUTER_PICKS.iter().map(|p| p.0).collect();
            let mut k = st.ai_model_pick.min(OPENROUTER_PICKS.len() - 1);
            ui.horizontal_wrapped(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut st.ai_key).password(true).desired_width(260.0).hint_text("sk-or-..."));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "OpenRouter key"));
                if dropdown(ui, "mom_ai_ormodel", &labels, &mut k, 300.0) { st.ai_model_pick = k; }
            });
            let ok = st.ai_key.trim().len() > 10;
            if pageshell::primary_on(ui, "Use OpenRouter", ok).clicked() && ok {
                st.ai_pick = Some(AiPick { provider: "openrouter", model: OPENROUTER_PICKS[k].1.into(), key: Some(std::mem::take(&mut st.ai_key)) });
            }
        });
        ui.label(theme::muted("Other AIs (Ollama, your own server) are under Account, then AI conversion.").size(12.0));
        if ui.link(format!("Keep my current AI ({})", ai_label(cfg))).clicked() { st.ai_keep = true; st.ai_open = false; }
    });
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
            st.current = None;
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
    if std::mem::take(&mut st.scroll_to_question) { ui.scroll_to_cursor(Some(egui::Align::Min)); }
    if worksheet_navigation(ui, st) { return; }
    let Some(c) = st.current.as_ref() else { st.step = Step::Describe; return };
    let working = busy(st);
    let src = c.q.emit();
    let stale = st.preview_html.is_none() || st.preview_for != src;
    if stale && !working && st.error.is_none() && c.passed == Some(true) {
        start_preview(st, ctx);
    }
    #[cfg(target_arch = "wasm32")]
    match overlay::take_action() {
        Some(overlay::Action::Close) | None => {}
    }
    pageshell::card(ui, |ui| {
        let c = st.current.as_ref().unwrap();
        ui.add(egui::Label::new(theme::serif(if c.q.name.is_empty() { "Your question" } else { &c.q.name }).size(20.0)).wrap());
        ui.add_space(6.0);
        if let Some(location) = &c.worksheet_source {
            ui.add(egui::Label::new(theme::strong(format!("Worksheet transcription · {location}"))).wrap());
            show_worksheet_source(ui, c);
            if let Some(original) = current_original(st) {
                egui::CollapsingHeader::new("Original worksheet page")
                    .id_salt("review_original").show(ui, |ui| original.draw(ui, 240.0));
            }
            ui.separator();
        }
        match c.passed {
            Some(true) => {
                let verdict = if !c.untested.is_empty() {
                    "Automated checks passed for the parts the checker supports. Review the remaining parts in the student preview."
                } else if c.worksheet_source.is_some() {
                    "✔ Passed the MyOpenMath checker. Compare it with the worksheet before approving."
                } else {
                    "✔ It works: we checked it with 3 different sets of numbers."
                };
                ui.add(egui::Label::new(RichText::new(verdict).color(theme::green()).size(14.0)).wrap());
            }
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
                ui.add_space(8.0);
                if !working {
                    let label = if stale { "Load student preview" } else { "Inspect student preview" };
                    if pageshell::outline(ui, label).clicked() {
                        if stale { start_preview(st, ctx); } else { open_sandbox(st, ctx, false); }
                    }
                }
                error_box(ui, st);
                return;
            }
            None if working => { ui.horizontal(|ui| { ui.spinner(); ui.label(theme::muted("Checking it…")); }); }
            None => { ui.label(theme::muted("Changed since the last check. Open Advanced tools and press Check It Again.")); }
        }
        let c = st.current.as_ref().unwrap();
        if !c.untested.is_empty() {
            ui.label(theme::muted(format!("Check part {} manually in the student preview.",
                c.untested.iter().map(|p| ((b'a' + *p as u8) as char).to_string()).collect::<Vec<_>>().join(", "))).size(13.0));
        }

        ui.add_space(14.0);
        ui.label(theme::strong("Try it as a student").size(15.0));
        let preview_help = if c.worksheet_source.is_some() {
            "Open the student preview, answer the question, and compare the wording, values, and worked solution with your worksheet."
        } else {
            "Answer the question and select Submit Question. Open the Written Example to check the solution, or try a similar question."
        };
        ui.add(egui::Label::new(theme::muted(preview_help).size(13.0)).wrap());
        if st.current.as_ref().is_some_and(|c| c.q.detailed_solution_field().is_empty()) {
            ui.add(egui::Label::new(RichText::new("This one has no worked solution. Pick \"Make the worked solution clearer\" below and press Rewrite It to add one.").color(theme::red()).size(13.0)).wrap());
        }
        ui.add_space(6.0);
        let ready = !stale && !working && st.current.as_ref().is_some_and(|c| c.passed == Some(true));
        if !working && stale && st.error.is_some() && st.current.as_ref().is_some_and(|c| c.passed == Some(true)) {
            if pageshell::outline(ui, "Retry student preview").clicked() {
                start_preview(st, ctx);
            }
        } else {
            ui.add_enabled_ui(ready, |ui| ui.horizontal_wrapped(|ui| {
                let preview = pageshell::primary(ui, if ready { "Try It as a Student" } else { "Getting it ready…" });
                #[cfg(not(target_arch = "wasm32"))]
                if ready && std::mem::take(&mut st.focus_student_review) { preview.request_focus(); }
                if preview.clicked() { open_sandbox(st, ctx, false); }
                #[cfg(not(target_arch = "wasm32"))]
                if pageshell::outline(ui, "Try with text controls")
                    .on_hover_text("Open the student preview as native text and form controls").clicked()
                {
                    open_sandbox(st, ctx, true);
                }
            }));
        }

        ui.add_space(14.0);
        let (seen, reviewed) = st.current.as_ref().map(|c| (c.preview_opened, c.reviewed)).unwrap_or_default();
        if reviewed {
            ui.horizontal(|ui| {
                ui.label(RichText::new("✔ Approved and saved").color(theme::green()));
                if pageshell::primary(ui, "Add It to MyOpenMath").clicked() { st.step = Step::Copy; }
            });
        } else {
            ui.label(theme::strong("Happy with it?").size(15.0));
            ui.add_space(4.0);
            if seen && let Some(c) = st.current.as_mut() {
                ui.checkbox(&mut c.preview_confirmed, "I checked how this question looks and scores in the student preview");
            }
            let confirmed = st.current.as_ref().is_some_and(|c| c.preview_confirmed);
            if pageshell::primary_on(ui, "Looks Good", confirmed).clicked() && confirmed {
                approve(st);
            }
            if !seen { crate::kit::gated_reason(ui, "Open the student preview first."); }
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

    });
}

fn show_worksheet_source(ui: &mut Ui, current: &Current) {
    if let Some((_, source)) = current.request.split_once("\n\nSource worksheet problem:\n") {
        egui::ScrollArea::vertical().id_salt("worksheet_source_text").max_height(125.0).show(ui, |ui| {
            ui.add(egui::Label::new(theme::text(source)).wrap());
        });
    }
}

fn current_original(st: &State) -> Option<&worksheet::OriginalPage> {
    let id = st.current.as_ref()?.saved_id.as_ref()?;
    st.library.questions.iter().find(|draft| &draft.id == id)?
        .worksheet_origin.as_ref()?.original.as_ref()
}

#[cfg(not(target_arch = "wasm32"))]
fn browser_workspace(ui: &mut Ui, st: &mut State, surface: crate::mom::desktop_browser::Surface) {
    use crate::mom::desktop_browser::Surface;
    let student = surface == Surface::Student;
    let working = busy(st);
    if student && st.student_text_mode && st.student_url.is_some() && st.student_text.is_none()
        && !working && st.error.is_none()
    {
        load_student_text(st, ui.ctx(), None);
    }
    let working = busy(st);
    let disconnected = student && st.student_url.is_some() && crate::mom::desktop_browser::disconnected(surface);
    let player_visible = !disconnected && if st.student_text_mode { st.student_text.is_some() } else { st.browser.has_page() };
    let can_approve = student && !working && player_visible && st.error.is_none()
        && st.current.as_ref().is_some_and(|c| c.preview_opened && c.passed == Some(true));
    let escape = ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let approve_key = can_approve && ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
    let mut back = false;
    let mut approved = false;
    let mut reload = false;
    let mut toggle_view = student && st.student_url.is_some() && !working
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::ALT, egui::Key::T));
    ui.horizontal_wrapped(|ui| {
        back = pageshell::outline(ui, if student { "Back to review" } else { "Back to transfer" }).clicked();
        ui.label(theme::serif(surface.title()).size(22.0));
        if student {
            let label = if st.student_text_mode { "Browser view" } else { "Text controls" };
            toggle_view |= ui.add_enabled_ui(!working && st.student_url.is_some(), |ui| pageshell::outline(ui, label))
                .inner.on_hover_text("Switch preview view with Alt+T").clicked();
            if st.current.as_ref().is_some_and(|current| current.passed == Some(true)) {
                approved = pageshell::primary_on(ui, "Approve and continue", can_approve)
                    .on_hover_text("Confirm that you checked the question in the student preview").clicked() && can_approve;
            }
            if !working && (disconnected || (st.error.is_some() && (!st.student_text_mode || st.student_url.is_none()))) {
                reload = pageshell::outline(ui, "Reload student preview").clicked();
            }
        }
    });
    if reload {
        open_sandbox(st, ui.ctx(), st.student_text_mode);
        return;
    }
    if escape || back || approved || approve_key {
        st.browser.close();
        if approved || approve_key {
            if let Some(c) = st.current.as_mut() { c.preview_confirmed = true; }
            approve(st);
        } else if student {
            st.focus_student_review = true;
        }
        ui.ctx().request_repaint();
        return;
    }
    if toggle_view {
        let edits = st.student_text.take().filter(|_| st.student_text_mode).map(|text| text.sync());
        st.student_text_mode = !st.student_text_mode;
        st.error = None;
        if let Some(edits) = edits { load_student_text(st, ui.ctx(), Some(edits)); }
        ui.ctx().request_repaint();
        return;
    }
    if let Some(c) = &st.current {
        ui.add(egui::Label::new(theme::text(&c.q.name)).wrap());
    }
    if student {
        let guidance = if st.current.as_ref().is_some_and(|current| current.passed == Some(true)) {
            "Check the answer and worked solution before approving. Esc to return. Ctrl/⌘+Enter to approve."
        } else {
            "Inspect the question and worked solution. Esc returns to review."
        };
        ui.label(theme::muted(guidance));
        if let Some(current) = &st.current && let Some(location) = &current.worksheet_source {
            egui::CollapsingHeader::new(format!("Compare worksheet transcription · {location}"))
                .id_salt("workspace_worksheet_source").show(ui, |ui| show_worksheet_source(ui, current));
            if let Some(original) = current_original(st) {
                egui::CollapsingHeader::new("Original worksheet page")
                    .id_salt("workspace_worksheet_original").show(ui, |ui| original.draw(ui, 175.0));
            }
        }
    } else if let Some(destination) = st.poster.destination() {
        ui.add(egui::Label::new(theme::muted(destination)).wrap());
    }
    let status = if working {
        st.log.last().map(String::as_str)
    } else if student {
        st.error.as_deref()
    } else {
        st.poster.message()
    };
    ui.separator();
    if disconnected {
        ui.label("The student browser closed. Reload the preview to start a new attempt.");
        return;
    }
    if student && st.student_text_mode {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        if let Some(status) = status { ui.label(theme::muted(status)); }
        if st.error.is_some() {
            if pageshell::outline(ui, "Refresh text controls").clicked() { load_student_text(st, ui.ctx(), None); }
        } else if let Some(text) = st.student_text.as_mut() {
            if let Some(action) = text.draw(ui, working) { load_student_text(st, ui.ctx(), Some(action)); }
        } else if working {
            ui.spinner();
            ui.label(theme::muted("Loading student text…"));
        }
        return;
    }
    st.browser.show(ui, working, status);
}

#[cfg(not(target_arch = "wasm32"))]
fn load_student_text(st: &mut State, ctx: &egui::Context, action: Option<StudentAction>) {
    use crate::mom::desktop_browser::{self, Surface};
    let Some(url) = st.student_url.clone() else { return };
    st.log.clear();
    let job = start(st, "Reading student preview");
    let ctx = ctx.clone();
    run_bg(async move {
        let result = desktop_browser::tab(Surface::Student, f64::from(ctx.pixels_per_point()))
            .and_then(|tab| StudentText::read(&tab, &url, action));
        finish(&job, &ctx, Outcome::StudentText(result));
    });
}

fn open_sandbox(st: &mut State, ctx: &egui::Context, text_mode: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = text_mode;
        let can_approve = st.current.as_ref().is_some_and(|c| c.passed == Some(true));
        overlay::open(st.preview_html.as_deref().unwrap_or(""), can_approve, ctx);
        if let Some(c) = st.current.as_mut() { c.preview_opened = true; }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::mom::desktop_browser::{self, Surface};
        let Some(html) = st.preview_html.clone() else { return };
        st.student_text_mode = text_mode;
        st.student_text = None;
        st.student_url = None;
        st.log.clear();
        st.browser.open(Surface::Student);
        let job = start(st, "Opening student preview");
        let ctx = ctx.clone();
        run_bg(async move {
            let result = (|| {
                let path = crate::store::store_dir().join("mom").join("preview.html");
                crate::platform::write(&path, html.as_bytes()).map_err(|error| format!("Could not write the preview: {error}"))?;
                let full = std::fs::canonicalize(path).map_err(|error| error.to_string())?;
                let url = url::Url::from_file_path(full).map_err(|_| "Could not open the student preview file.")?;
                let tab = desktop_browser::tab(Surface::Student, f64::from(ctx.pixels_per_point()))?;
                tab.navigate(url.as_str())?;
                if !tab.wait_for("document.querySelector('#questionwrap0 > .question') || [...document.querySelectorAll('#app button')].some(b => b.innerText.trim() === 'Start' && b.getClientRects().length)", std::time::Duration::from_secs(10))? {
                    return Err("The student player did not load. Return to review and try again.".into());
                }
                Ok(url.to_string())
            })();
            finish(&job, &ctx, Outcome::StudentBrowser(result));
        });
    }
}

/// The teacher tried it and likes it: save it and show how to add it to MyOpenMath.
fn approve(st: &mut State) {
    let Some(c) = st.current.as_mut() else { return };
    if c.passed != Some(true) || !c.preview_opened || !c.preview_confirmed { return; }
    c.reviewed = true;
    save_current(st);
    st.step = Step::Copy;
    st.scroll_to_question = true;
}

fn copy_step(ui: &mut Ui, ctx: &egui::Context, st: &mut State) {
    if std::mem::take(&mut st.scroll_to_question) { ui.scroll_to_cursor(Some(egui::Align::Min)); }
    if worksheet_navigation(ui, st) { return; }
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

fn open_saved(st: &mut State, saved: Saved) {
    #[cfg(not(target_arch = "wasm32"))]
    { st.poster.clear_result(); }
    st.worksheet_mode = saved.worksheet_source.is_some();
    st.current = Some(Current {
        q: MomQuestion::parse(&saved.src),
        request: saved.request.clone(),
        passed: Some(saved.passed),
        problems: saved.problems,
        untested: saved.untested,
        worksheet_source: saved.worksheet_source,
        saved_id: Some(saved.id),
        preview_opened: saved.reviewed,
        preview_confirmed: saved.reviewed,
        reviewed: saved.reviewed,
    });
    st.request = saved.request;
    st.preview_html = None;
    st.error = None;
    st.note = None;
    st.scroll_to_question = true;
    st.step = if saved.reviewed { Step::Copy } else { Step::Try };
}

/// My Questions: everything saved, with a plain status and Open / Delete.
fn side(ui: &mut Ui, ctx: &egui::Context, st: &mut State) {
    pageshell::card(ui, |ui| {
        if std::mem::take(&mut st.jump_library) { ui.scroll_to_cursor(Some(egui::Align::Min)); }
        pageshell::heading(ui, "My Questions");
        ui.add_space(4.0);
        if st.library.questions.is_empty() {
            ui.label(theme::muted("Converted drafts and approved questions appear here.").size(13.0));
        }
        let mut open: Option<Saved> = None;
        let mut delete: Option<String> = None;
        for s in &st.library.questions {
            let q = MomQuestion::parse(&s.src);
            let name = if q.name.is_empty() { "Untitled question".to_string() } else { q.name.clone() };
            ui.add_space(6.0);
            ui.add(egui::Label::new(theme::text(&name).size(14.0)).wrap());
            if let Some(source) = &s.worksheet_source {
                ui.add(egui::Label::new(theme::muted(source).size(12.0)).wrap());
            }
            ui.add_enabled_ui(!busy(st), |ui| ui.horizontal(|ui| {
                let (status, fill, fg) = if !s.passed {
                    (saved_status(s), theme::error_wash(), theme::red())
                } else if s.reviewed {
                    (saved_status(s), theme::accent_faded(), theme::accent_col())
                } else {
                    (saved_status(s), theme::highlighter(), theme::on_highlighter())
                };
                crate::kit::pill(ui, status, fill, fg);
                if ui.link("Open").clicked() { open = Some(s.clone()); }
                if st.confirm_delete.as_deref() == Some(s.id.as_str()) {
                    if ui.button(RichText::new("Yes, delete").color(theme::red())).clicked() { delete = Some(s.id.clone()); }
                    if ui.button("Keep it").clicked() { st.confirm_delete = None; }
                } else if ui.link(RichText::new("Delete").color(theme::muted_col())).clicked() {
                    st.confirm_delete = Some(s.id.clone());
                }
            }));
        }
        if let Some(s) = open {
            open_saved(st, s);
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
            } else if st.engine == "local" {
                "Built in: D.A.D runs IMathAS on this computer (no Docker needed). Type a server address to use one instead."
            } else {
                "The IMathAS engine that checks questions. Start it with engine/run.sh."
            }).size(12.0));
            let r = ui.add(egui::TextEdit::singleline(&mut st.engine).desired_width(240.0).hint_text(DEFAULT_ENGINE));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Checker address"));
            if r.lost_focus() { let _ = crate::store::save_json("mom/engine.json", &st.engine); }
        });
    });
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
        if let Some(c) = st.current.as_mut() { c.passed = None; c.problems.clear(); c.reviewed = false; c.preview_opened = false; c.preview_confirmed = false; }
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

    pub enum Action { Close }

    pub fn take_action() -> Option<Action> {
        match ACTION.swap(0, Ordering::Relaxed) { 2 => Some(Action::Close), _ => None }
    }

    fn button(doc: &web_sys::Document, text: &str, primary: bool, code: u8) -> Option<web_sys::Element> {
        let b = doc.create_element("button").ok()?;
        b.set_text_content(Some(text));
        let _ = b.set_attribute("type", "button");
        let _ = b.set_attribute("style", &format!(
            "font:600 15px 'Source Sans 3',system-ui,sans-serif;padding:8px 18px;border-radius:10px;cursor:pointer;border:1px solid {};background:{};color:{}",
            if primary { "#234839" } else { "var(--dad-edge,#9c8672)" },
            if primary { "#234839" } else { "var(--dad-card,#fffdf8)" },
            if primary { "#fff5df" } else { "var(--dad-ink,#234839)" }));
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
        let _ = panel.set_attribute("style", "position:fixed;inset:0;z-index:30;display:flex;flex-direction:column;background:var(--dad-desk,#fff5df)");
        let Ok(bar) = doc.create_element("div") else { return };
        let _ = bar.set_attribute("style", "display:flex;flex-wrap:wrap;gap:10px;align-items:center;padding:12px 20px;border-bottom:1px solid var(--dad-edge,#9c8672);background:var(--dad-card,#fffdf8);font:15px 'Source Sans 3',system-ui,sans-serif;color:var(--dad-ink,#234839)");
        let Ok(title) = doc.create_element("div") else { return };
        let _ = title.set_attribute("style", "margin-right:auto;min-width:0");
        title.set_inner_html(if can_approve {
            "<b style=\"font-size:17px\">Try it as a student</b> &nbsp;Answer it and press Submit, then Show Detailed Solution. Try \"Try a similar question\" once or twice."
        } else {
            "<b style=\"font-size:17px\">Try it as a student</b> &nbsp;This question still has a problem, so it can't be approved yet."
        });
        let _ = bar.append_child(&title);
        if let Some(b) = button(&doc, "Back", false, 2) { let _ = bar.append_child(&b); }
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
    fn worksheet_progress_can_resume_without_duplicating_completed_drafts() {
        let problem = |number| WorksheetProblem {
            source: worksheet::SourceQuestion { location: "Worksheet.txt".into(), text: "Find x.".into(), original: None },
            origin: WorksheetOrigin { batch: "active".into(), number, original: None },
        };
        let mut library = Library {
            worksheet: Some(WorksheetBatch {
                id: "active".into(), pending: vec![problem(1), problem(2), problem(3)],
                notes: Vec::new(), failures: Vec::new(),
            }),
            ..Default::default()
        };
        assert!(record_worksheet_item(&mut library, problem(1).origin, Ok(Saved { id: "first".into(), ..Default::default() })));
        assert!(record_worksheet_item(&mut library, problem(2).origin, Err(WorksheetFailure { problem: problem(2), error: "Time out.".into() })));
        let bytes = serde_json::to_vec(&library).unwrap();
        let mut library: Library = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(library.worksheet.as_ref().unwrap().pending[0].origin.number, 3);
        assert!(!record_worksheet_item(&mut library, problem(1).origin, Ok(Saved::default())));
        assert!(record_worksheet_item(&mut library, problem(2).origin, Ok(Saved { id: "second".into(), ..Default::default() })));
        assert_eq!(library.questions.len(), 2);
        assert!(library.worksheet.unwrap().failures.is_empty());
    }

    #[test]
    fn worksheet_review_keeps_original_order_after_retries_and_reopen() {
        let make_draft = |id: &str, batch: &str, number| Saved {
            id: id.into(),
            worksheet_origin: Some(WorksheetOrigin { batch: batch.into(), number, original: None }),
            ..Default::default()
        };
        let library = Library { worksheet: None, questions: vec![
            make_draft("retried", "worksheet", 2),
            make_draft("next", "worksheet", 3),
            make_draft("different", "another worksheet", 1),
            make_draft("first", "worksheet", 1),
            Saved { id: "older draft".into(), ..Default::default() },
        ] };
        let library: Library = serde_json::from_slice(&serde_json::to_vec(&library).unwrap()).unwrap();
        let order: Vec<_> = worksheet_drafts(&library, "worksheet").into_iter()
            .map(|(number, draft)| (number, draft.id.as_str())).collect();
        assert_eq!(order, [(1, "first"), (2, "retried"), (3, "next")]);
    }

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
    fn opening_the_student_player_does_not_approve_a_question() {
        let mut st = State {
            current: Some(Current {
                passed: Some(true), preview_opened: true, ..Default::default()
            }),
            ..Default::default()
        };
        approve(&mut st);
        assert!(!st.current.unwrap().reviewed);
        assert!(st.library.questions.is_empty());
    }
}

/// Desktop MyOpenMath transfer, driven in D.A.D’s browser panel.
#[cfg(not(target_arch = "wasm32"))]
mod autoadd {
    use super::*;
    use crate::mom::desktop_browser::{self, Surface};
    use crate::mom::livecdp::LiveTab;
    use crate::mom::poster::{self, Added, Assessment, Course, PublishState};
    use std::time::Duration;

    const PREFS: &str = "mom/poster.json";

    #[derive(Serialize, Deserialize, Default, Clone)]
    #[serde(default)]
    struct Prefs { course: Option<u64>, assessment: Option<u64> }

    fn publication_path(st: &State, cid: u64, aid: u64) -> Option<String> {
        let id = st.current.as_ref()?.saved_id.as_deref()?;
        if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()) { return None; }
        Some(format!("mom/publications/{id}-{cid}-{aid}.json"))
    }

    #[derive(Default)]
    pub struct Poster {
        loaded: bool,
        prefs: Prefs,
        courses: Vec<Course>,
        assessments: Vec<Assessment>,
        assessments_for: Option<u64>,
        assessments_error: Option<String>,
        result: Option<Result<String, String>>,
        user: String,
        pass: String,
        has_login: Option<bool>,
    }

    impl Poster {
        pub(super) fn clear_result(&mut self) { self.result = None; }

        pub(super) fn message(&self) -> Option<&str> {
            self.result.as_ref().map(|result| match result { Ok(message) | Err(message) => message.as_str() })
        }

        pub(super) fn destination(&self) -> Option<String> {
            let course = self.courses.iter().find(|course| Some(course.id) == self.prefs.course)?;
            let assessment = self.assessments.iter().find(|assessment| Some(assessment.id) == self.prefs.assessment)?;
            Some(format!("{} · {}", course.name, assessment.name))
        }
    }

    fn tab(ctx: &egui::Context) -> Result<LiveTab, String> {
        desktop_browser::tab(Surface::Transfer, f64::from(ctx.pixels_per_point()))
    }

    fn log(job: &Arc<Mutex<Job>>, ctx: &egui::Context, line: &str) {
        if let Ok(mut j) = job.lock() { j.log.push(line.to_string()); }
        ctx.request_repaint();
    }

    /// Signed in, by the saved password or by the teacher in the window (waits up to 10 minutes).
    fn ensure_signed_in(t: &mut LiveTab, job: &Arc<Mutex<Job>>, ctx: &egui::Context) -> Result<(), String> {
        log(job, ctx, "Opening MyOpenMath…");
        if poster::signed_in(t)? { return Ok(()); }
        if let Some((u, p)) = poster::saved_login() {
            log(job, ctx, "Signing in with your saved MyOpenMath login…");
            if poster::sign_in_with(t, &u, &p)? { return Ok(()); }
            log(job, ctx, "The saved login didn't work.");
        }
        desktop_browser::set_manual_sign_in(true);
        log(job, ctx, "Sign in to MyOpenMath in the browser panel. D.A.D continues when you are signed in.");
        let result = (|| {
            let started = std::time::Instant::now();
            while desktop_browser::transfer_open() && desktop_browser::manual_sign_in()
                && started.elapsed() < Duration::from_secs(600)
            {
                if poster::wait_signed_in(t, Duration::from_secs(1))? {
                    return if desktop_browser::manual_sign_in() { Ok(()) } else { Err("Sign-in canceled.".into()) };
                }
            }
            if desktop_browser::transfer_open() && desktop_browser::manual_sign_in() {
                Err("Sign-in timed out. Connect to MyOpenMath to try again.".into())
            } else {
                Err("Sign-in canceled.".into())
            }
        })();
        desktop_browser::set_manual_sign_in(false);
        result
    }

    fn start_connect(st: &mut State, ctx: &egui::Context) {
        st.log.clear();
        st.poster.result = None;
        st.browser.open(Surface::Transfer);
        let job = start(st, "MyOpenMath");
        let ctx = ctx.clone();
        run_bg(async move {
            let r = tab(&ctx).and_then(|mut t| { ensure_signed_in(&mut t, &job, &ctx)?; log(&job, &ctx, "Reading your courses…"); poster::courses(&mut t) });
            finish(&job, &ctx, Outcome::Courses(r));
        });
    }

    fn start_assessments(st: &mut State, ctx: &egui::Context, cid: u64) {
        st.poster.assessments_error = None;
        st.log.clear();
        let job = start(st, "MyOpenMath");
        let ctx = ctx.clone();
        run_bg(async move {
            log(&job, &ctx, "Reading the assessments…");
            let r = tab(&ctx).and_then(|mut t| poster::assessments(&mut t, cid));
            finish(&job, &ctx, Outcome::Assessments(cid, r));
        });
    }

    fn start_add(st: &mut State, ctx: &egui::Context, cid: u64, aid: u64, publish: bool) {
        let Some(q) = st.current.as_ref().map(|c| c.q.clone()) else { return };
        let path = publication_path(st, cid, aid);
        if publish && path.is_none() { return; }
        st.log.clear();
        st.poster.result = None;
        st.browser.open(Surface::Transfer);
        let job = start(st, "MyOpenMath");
        let ctx = ctx.clone();
        run_bg(async move {
            let r = tab(&ctx).and_then(|mut t| {
                ensure_signed_in(&mut t, &job, &ctx)?;
                let (j, c) = (job.clone(), ctx.clone());
                let mut progress = |l: &str| log(&j, &c, l);
                if let Some(path) = path.as_deref().filter(|_| publish) {
                    poster::publish_question(&mut t, cid, aid, &q, path, &mut progress)
                } else {
                    poster::preview_question(&mut t, cid, aid, &q, &mut progress)
                }
            });
            finish(&job, &ctx, Outcome::Posted { result: r, preview: !publish });
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
        if st.poster.prefs.course != Some(cid) { return; }
        match r {
            Ok(a) => { st.poster.assessments = a; st.poster.assessments_for = Some(cid); }
            Err(e) => { st.poster.assessments_error = Some(e); }
        }
    }

    pub fn posted(st: &mut State, r: Result<Added, String>) {
        st.poster.result = Some(match r {
            Ok(Added::Saved(p)) => Ok(format!("Verified question {} in MyOpenMath assessment {}.", p.question, p.assessment)),
            Ok(Added::SavedToSet(p, error)) => Err(format!("Question {} is in your MyOpenMath question set. Attachment is unconfirmed. {error}", p.question)),
            Ok(Added::Filled) => Ok("Save is disabled in this preview. Return to transfer to publish to the selected assessment.".into()),
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
            // Keep the destination controls visible against the card in both themes.
            ui.style_mut().visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, theme::edge());
            pageshell::heading(ui, "Add it to MyOpenMath for me");
            ui.add_space(4.0);
            if st.poster.courses.is_empty() {
                ui.add(egui::Label::new(theme::muted("Connect to MyOpenMath in D.A.D. Pick a course and assessment to preview or publish the question.").size(13.0)).wrap());
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
                if !working && k != before && k != usize::MAX {
                    st.poster.prefs.course = Some(st.poster.courses[k].id);
                    st.poster.prefs.assessment = None;
                    st.poster.assessments.clear();
                    st.poster.assessments_error = None;
                    st.poster.result = None;
                    let _ = crate::store::save_json(PREFS, &st.poster.prefs);
                }
                if let Some(cid) = st.poster.prefs.course {
                    if st.poster.assessments_for != Some(cid) && !working {
                        st.poster.assessments_for = Some(cid);
                        start_assessments(st, ctx, cid);
                    }
                    field_label(ui, "Assessment");
                    if let Some(error) = &st.poster.assessments_error {
                        ui.add(egui::Label::new(theme::muted(format!("Could not load assessments. {error}"))).wrap());
                        if !working && pageshell::outline(ui, "Retry loading assessments").clicked() {
                            start_assessments(st, ctx, cid);
                        }
                    } else if st.poster.assessments.is_empty() {
                        ui.label(theme::muted(if working { "Reading the assessments…" } else { "This course has no assessments yet. Make one in MyOpenMath first." }).size(13.0));
                    } else {
                        let names: Vec<&str> = st.poster.assessments.iter().map(|a| a.name.as_str()).collect();
                        let mut k = st.poster.prefs.assessment.and_then(|id| st.poster.assessments.iter().position(|a| a.id == id)).unwrap_or(usize::MAX);
                        let shown = if k == usize::MAX { "Pick an assessment" } else { names[k] };
                        let before = k;
                        egui::ComboBox::from_id_salt("mom_post_assess").selected_text(shown).width(340.0).height(520.0).show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut k, i, *n); }
                        });
                        if !working && k != before && k != usize::MAX {
                            st.poster.prefs.assessment = Some(st.poster.assessments[k].id);
                            st.poster.result = None;
                            let _ = crate::store::save_json(PREFS, &st.poster.prefs);
                        }
                    }
                }
                ui.add_space(8.0);
                let target = st.poster.prefs.course.zip(st.poster.prefs.assessment).filter(|(cid, aid)| {
                    st.poster.courses.iter().any(|course| course.id == *cid)
                        && st.poster.assessments_for == Some(*cid)
                        && st.poster.assessments.iter().any(|assessment| assessment.id == *aid)
                });
                if let Some((cid, aid)) = target {
                    let receipt = publication_path(st, cid, aid);
                    let publication = receipt.as_ref().and_then(|path| crate::store::load_json::<PublishState>(path));
                    ui.add_enabled_ui(!working, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if pageshell::outline(ui, "Preview in MyOpenMath").clicked() { start_add(st, ctx, cid, aid, false); }
                            match publication {
                                None | Some(PublishState::ReadyToSave) => {
                                    if pageshell::primary_on(ui, "Publish to selected assessment", receipt.is_some()).clicked() {
                                        start_add(st, ctx, cid, aid, true);
                                    }
                                }
                                Some(PublishState::SavedToSet(_)) => {
                                    if pageshell::primary(ui, "Retry attachment").clicked() { start_add(st, ctx, cid, aid, true); }
                                }
                                Some(PublishState::Attached(p)) => {
                                    ui.label(theme::strong(format!("Published · question {}", p.question)));
                                }
                                Some(PublishState::AwaitingConfirmation) => {
                                    ui.label(theme::muted("Save unconfirmed. Check your MyOpenMath question set."));
                                }
                            }
                        });
                    });
                    if receipt.is_none() {
                        crate::kit::gated_reason(ui, "Publishing requires a saved, approved question.");
                    }
                } else {
                    crate::kit::gated_reason(ui, "Pick a course and an assessment first.");
                }
            }
            if mine && ui.link("Show MyOpenMath browser").clicked() { st.browser.open(Surface::Transfer); }
            if working && mine && !st.log.is_empty() {
                ui.add_space(8.0);
                for line in st.log.iter().rev().take(3).collect::<Vec<_>>().into_iter().rev() {
                    ui.horizontal(|ui| {
                        ui.spinner();
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

    pub(super) fn connection(ui: &mut Ui, st: &mut State) {
        pageshell::card(ui, |ui| {
            pageshell::heading(ui, "MyOpenMath connection");
            ui.add(egui::Label::new(theme::muted("Save your instructor login here, or sign in through the browser panel when adding a question.").size(13.0)).wrap());
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

#[cfg(not(target_arch = "wasm32"))]
pub fn connection_settings(ui: &mut Ui) {
    ST.with(|state| autoadd::connection(ui, &mut state.borrow_mut()));
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
    pub struct Prefs { server: String, token: String, course: Option<u64>, assessment: Option<u64> }

    impl Default for Prefs {
        fn default() -> Prefs { Prefs { server: "ws://127.0.0.1:8793".into(), token: String::new(), course: None, assessment: None } }
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
        if let Some(c) = p.course { set("course", (c as f64).into())?; }
        if let Some(a) = p.assessment { set("assessment", (a as f64).into())?; }
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
