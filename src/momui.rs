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

use crate::mom::author::{author_async_logged, author_full, post_engine, AuthorCritic, Budget, HttpEngine, ModelChat, AuthorEngine};
use crate::mom::engine::{SelftestResponse, DEFAULT_SEEDS};
use crate::mom::form::{self, Answer, Change, Difficulty, Form, Rounding, Topic, AI_CHOOSES, COURSES, MAX_PARTS};
use crate::mom::MomQuestion;
use crate::mom::worksheet;
use crate::mom::question_help;
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
const WRITING_PREFERENCE: &str = "mom/writing_preference.json";

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    follow_ups: Vec<String>,
    /// The mom file.
    pub src: String,
    /// The self-test verdict when it was saved.
    pub passed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    needs_check: bool,
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

#[derive(Deserialize, Serialize)]
struct ShareFile {
    questions: Vec<SharedQuestion>,
}

#[derive(Deserialize, Serialize)]
struct SharedQuestion {
    src: String,
}

pub(crate) fn is_question_file(name: &str, bytes: &[u8]) -> bool {
    let extension = name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
    matches!(extension.as_deref(), Some("php" | "json"))
        || extension.as_deref() == Some("txt")
            && std::str::from_utf8(bytes).is_ok_and(|text| !MomQuestion::parse(text).qtext.is_empty())
}

fn imported_sources(file: &PickedFile) -> Result<Vec<String>, String> {
    let text = std::str::from_utf8(&file.bytes).map_err(|_| "Save the question as a UTF-8 file and try again.")?;
    if text.trim_start().starts_with('{') || file.name.to_ascii_lowercase().ends_with(".json") {
        let package: ShareFile = serde_json::from_str(text)
            .map_err(|_| "Choose a D.A.D question package containing a questions list.")?;
        Ok(package.questions.into_iter().map(|question| question.src).collect())
    } else {
        Ok(vec![text.to_owned()])
    }
}

fn export_shared_questions(questions: &[Saved]) -> Result<bool, String> {
    let Some(path) = crate::platform::save_path("dad-shared-questions.json", "D.A.D questions", &["json"]) else {
        return Ok(false);
    };
    let package = ShareFile { questions: questions.iter().map(|draft| SharedQuestion { src: draft.src.clone() }).collect() };
    let bytes = serde_json::to_vec_pretty(&package).map_err(|error| error.to_string())?;
    crate::platform::write(&path, &bytes).map_err(|error| error.to_string())?;
    Ok(true)
}

fn import_questions(ui: &mut Ui, st: &mut State) {
    st.question_files.extend(crate::platform::take_picked());
    pageshell::title(ui, "", "Import MyOpenMath questions",
        "Bring in MyOpenMath question code or a D.A.D package shared by another teacher.");
    ui.add_space(12.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        pageshell::card(ui, |ui| {
            if pageshell::outline(ui, "Choose question files").clicked()
                && let Some(paths) = crate::platform::pick_files("MyOpenMath questions or D.A.D packages", &["php", "txt", "json"])
            {
                for path in paths {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    match crate::platform::read(&path) {
                        Ok(bytes) => st.question_files.push(PickedFile { name, bytes }),
                        Err(error) => st.error = Some(format!("Could not open {name}: {error}")),
                    }
                }
            }
            for file in &st.question_files { ui.label(theme::text(&file.name)); }
            let caption = ui.label(theme::strong("Or paste question code"));
            ui.add(egui::TextEdit::multiline(&mut st.import_source).desired_width(f32::INFINITY).desired_rows(5))
                .labelled_by(caption.id);
            if pageshell::primary_on(ui, "Import into My Questions", !busy(st) &&
                (!st.question_files.is_empty() || !st.import_source.trim().is_empty())).clicked()
            {
                import_drafts(st);
            }
            if let Some(error) = &st.error {
                ui.add(egui::Label::new(RichText::new(error).color(theme::red())).wrap());
            }
        });
        ui.add_space(12.0);
        let ctx = ui.ctx().clone();
        side(ui, &ctx, st);
    });
}

fn import_drafts(st: &mut State) {
    let mut known: std::collections::HashSet<String> = st.library.questions.iter()
        .map(|draft| MomQuestion::parse(&draft.src).emit()).collect();
    let mut errors = Vec::new();
    let mut drafts = Vec::new();
    let mut duplicates = 0;
    let paste = PickedFile { name: "Pasted code".into(), bytes: st.import_source.as_bytes().to_vec() };
    for file in st.question_files.iter().chain((!st.import_source.trim().is_empty()).then_some(&paste)) {
        match imported_sources(file) {
            Ok(sources) => for src in sources {
                let mut q = MomQuestion::parse(&src);
                if q.qtext.trim().is_empty() {
                    errors.push(format!("{} has a question with no Question Text section.", file.name));
                    continue;
                }
                if q.name.trim().is_empty() { q.name = "Imported question".into(); }
                let src = q.emit();
                if !known.insert(src.clone()) { duplicates += 1; continue; }
                let request = format!("Repair this imported MyOpenMath question while preserving its original task.\n\n{src}");
                drafts.push(Saved { id: crate::model::new_id(), src, request, needs_check: true, saved_at: timestamp(), ..Default::default() });
            },
            Err(error) => errors.push(format!("{}: {error}", file.name)),
        }
    }
    let count = drafts.len();
    let before = st.library.questions.len();
    st.library.questions.extend(drafts);
    if count > 0 && let Err(error) = crate::store::save_json(LIBRARY, &st.library) {
        st.library.questions.truncate(before);
        st.error = Some(format!("Could not save the imported questions: {error}"));
        return;
    }
    let drafts = if count == 1 { "draft" } else { "drafts" };
    let repeated = if duplicates == 1 { "duplicate" } else { "duplicates" };
    st.lib_note = Some(format!("Imported {count} {drafts}. Skipped {duplicates} {repeated}."));
    if errors.is_empty() {
        st.question_files.clear();
        st.import_source.clear();
    }
    st.error = (!errors.is_empty()).then(|| errors.join("\n"));
}

fn share_questions(ui: &mut Ui, st: &mut State) {
    pageshell::title(ui, "", "Share MyOpenMath questions",
        "Download a D.A.D package another teacher can import. Open an approved question to add it to MyOpenMath.");
    ui.add_space(12.0);
    let mut open = None;
    let mut export = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        pageshell::card(ui, |ui| {
            if st.library.questions.is_empty() {
                ui.label(theme::muted("Create, convert, or import a question to share it here."));
            } else if pageshell::primary(ui, "Download all questions").clicked() {
                export = Some(st.library.questions.as_slice());
            }
            for draft in &st.library.questions {
                ui.push_id(&draft.id, |ui| {
                    ui.separator();
                    let q = MomQuestion::parse(&draft.src);
                    ui.add(egui::Label::new(theme::strong(q.name)).wrap());
                    ui.horizontal_wrapped(|ui| {
                        if pageshell::outline(ui, "Download question").clicked() { export = Some(std::slice::from_ref(draft)); }
                        let label = if draft.reviewed { "Add to MyOpenMath" } else { "Open for review" };
                        if pageshell::outline(ui, label).clicked() { open = Some(draft.clone()); }
                    });
                });
            }
            if let Some(note) = &st.lib_note { ui.label(theme::muted(note)); }
            error_box(ui, st);
        });
    });
    if let Some(questions) = export {
        match export_shared_questions(questions) {
            Ok(true) => { st.error = None; st.lib_note = Some("Question package saved.".into()); }
            Ok(false) => {}
            Err(error) => st.error = Some(format!("Could not save the question package: {error}")),
        }
    }
    if let Some(saved) = open { open_saved(st, saved); }
}

/// The question on the right.
#[derive(Clone, Default)]
struct Current {
    q: MomQuestion,
    request: String,
    follow_ups: Vec<String>,
    follow_up_draft: String,
    follow_up_failed: bool,
    video_link_draft: String,
    video_search_topic: String,
    video_suggestion: Option<question_help::VideoSuggestion>,
    video_error: Option<String>,
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
    Authored { follow_up: Option<String>, q: Option<MomQuestion>, passed: bool, problems: Vec<String>, untested: Vec<usize>, error: Option<String> },
    WorksheetRead { questions: Vec<worksheet::SourceQuestion>, notes: Vec<String>, retry_files: Vec<PickedFile> },
    WorksheetItem(WorksheetOrigin, Result<Saved, WorksheetFailure>),
    WorksheetFinished,
    Tested { source: String, saved_id: Option<String>, result: Result<SelftestResponse, String>, then_save: bool },
    Preview(Result<String, String>),
    VideoSearched { source: String, result: Result<question_help::VideoSuggestion, String> },
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Destination { #[default] Questions, Import, Convert, Share }

pub(crate) fn request_destination(ctx: &egui::Context, destination: Destination) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("dad_destination_request"), destination));
    ctx.request_repaint();
}

pub(crate) fn active_destination(ctx: &egui::Context) -> Destination {
    ctx.data(|data| data.get_temp(egui::Id::new("dad_destination_active"))).unwrap_or_default()
}

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
    tool_page: Option<Destination>,
    question_files: Vec<PickedFile>,
    import_source: String,
    worksheet_save_error: Option<String>,
    scroll_to_question: bool,
    form: Form,
    topic_image: Option<PickedFile>,
    topic_image_error: Option<String>,
    setup_video_link: String,
    change: Change,
    change_other: String,
    writing_preference: String,
    preference_feedback: Option<String>,
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
    /// The highlighted entry in the card's AI dropdown.
    ai_choice: usize,
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
        state.tool_page = None;
        state.worksheet_mode = true;
        if !busy(&state) { state.step = Step::Describe; }
    });
}

pub fn open_question_file(name: &str, bytes: Vec<u8>) {
    ST.with(|state| {
        let mut state = state.borrow_mut();
        #[cfg(not(target_arch = "wasm32"))]
        state.browser.close();
        state.question_files.push(PickedFile { name: name.into(), bytes });
        state.tool_page = Some(Destination::Import);
    });
}

pub(crate) fn stage_figure(title: String, spec: String, line: Option<crate::mom::line_variant::LineVariant>) {
    ST.with(|state| {
        let mut st = state.borrow_mut();
        if !st.inited { st.form = crate::store::load_json(FORM).unwrap_or_default(); }
        st.form.figure = Some((title, spec));
        st.form.line_variant = line;
        st.tool_page = None;
        st.worksheet_mode = false;
        st.step = Step::Describe;
        st.scroll_to_question = true;
        st.error = crate::store::save_json(FORM, &st.form).err();
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
            if st.form.figure.is_none() {
                st.worksheet_mode |= st.library.worksheet.is_some();
                st.form = crate::store::load_json(FORM).unwrap_or_default();
            }
            st.form.fix();
            st.writing_preference = crate::store::load_json(WRITING_PREFERENCE).unwrap_or_default();
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
        let destination = st.tool_page.unwrap_or_else(|| {
            if st.step == Step::Describe && st.worksheet_mode { Destination::Convert } else { Destination::Questions }
        });
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("dad_destination_active"), destination));
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
            if !busy(&st) {
                let requested = ctx.data_mut(|data| {
                    let id = egui::Id::new("dad_destination_request");
                    let result = data.get_temp::<Destination>(id);
                    data.remove::<Destination>(id);
                    result
                });
                match requested {
                    Some(Destination::Import | Destination::Share) => st.tool_page = requested,
                    Some(Destination::Convert) => {
                        st.tool_page = None;
                        st.worksheet_mode = true;
                        st.step = Step::Describe;
                        st.scroll_to_question = true;
                    }
                    Some(Destination::Questions) => {
                        st.tool_page = None;
                        if st.step == Step::Describe { st.worksheet_mode = false; }
                    }
                    None => {}
                }
            }
            match st.tool_page {
                Some(Destination::Import) => { import_questions(ui, &mut st); return; }
                Some(Destination::Share) => { share_questions(ui, &mut st); return; }
                _ => {}
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
        Outcome::VideoSearched { source, result } => {
            if let Some(current) = st.current.as_mut().filter(|current| current.q.emit() == source) {
                match result {
                    Ok(video) => current.video_suggestion = Some(video),
                    Err(error) => current.video_error = Some(error),
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        Outcome::Courses(r) => autoadd::courses_in(st, r),
        #[cfg(not(target_arch = "wasm32"))]
        Outcome::Assessments(cid, r) => autoadd::assessments_in(st, cid, r),
        #[cfg(not(target_arch = "wasm32"))]
        Outcome::Posted { result, preview } => {
            if preview && result.is_err() { st.browser.close(); }
            autoadd::posted(st, result);
        }
        Outcome::Authored { follow_up, q, passed, problems, untested, error } => {
            if let Some(message) = &follow_up
                && (!passed || q.is_none())
            {
                if let Some(current) = st.current.as_mut() {
                    current.follow_up_draft = message.clone();
                    current.follow_up_failed = true;
                }
                return;
            }
            st.error = error;
            if let Some(mut q) = q {
                // A question needs a name (MyOpenMath's Description); a model sometimes leaves it out.
                if q.name.trim().is_empty() {
                    q.name = st.form.short_name();
                }
                // A rewrite keeps the question's place in My Questions.
                let saved_id = st.current.as_ref().and_then(|c| c.saved_id.clone());
                let worksheet_source = st.current.as_ref().and_then(|c| c.worksheet_source.clone());
                let mut follow_ups = st.current.as_mut().map(|c| std::mem::take(&mut c.follow_ups)).unwrap_or_default();
                follow_ups.extend(follow_up);
                st.current = Some(Current { q, request: st.request.clone(), follow_ups, passed: Some(passed), problems, untested, saved_id, worksheet_source, ..Default::default() });
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
        Outcome::Tested { source, saved_id, result, then_save } => {
            let Some(c) = st.current.as_mut().filter(|c| c.saved_id == saved_id && c.q.emit() == source) else {
                return;
            };
            match result {
                Ok(rep) => {
                    c.passed = Some(rep.pass);
                    c.problems = rep.problems();
                    c.untested = rep.untested_parts.clone();
                    st.error = None;
                    if then_save { save_current(st); }
                }
                Err(e) => st.error = Some(e),
            }
        }
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
                    s.needs_check = false;
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
    write_question(st, ctx, cfg, request, None, None);
}

fn write_question(st: &mut State, ctx: &egui::Context, cfg: &crate::ai::AiCfg, request: String, follow_up: Option<String>, setup: Option<(Form, Option<Vec<u8>>)>) {
    st.log.clear();
    let job = start(st, if follow_up.is_some() { "Revising" } else { "Writing" });
    let preference = crate::store::load_json::<String>(WRITING_PREFERENCE).unwrap_or_default();
    let request = form::with_writing_preference(&request, &preference);
    let video_help = if setup.is_some() {
        question_help::video_url(&st.setup_video_link).ok()
    } else {
        st.current.as_ref().and_then(|current| question_help::attached_video(&current.q))
    };
    let (engine, cfg, ctx) = (st.engine.clone(), cfg.clone(), ctx.clone());
    run_bg(async move {
        let (form, image) = setup.map_or((None, None), |(form, image)| (Some(form), image));
        let mut chat = ModelChat { cfg: &cfg, tools: false, reference_image: image.as_deref(), video_help };
        let mut critic = SetupCheck { form, engine: engine.clone() };
        let mut eng = HttpEngine { base: engine };
        let log_job = job.clone();
        let log_ctx = ctx.clone();
        let run = author_full(&request, &mut chat, &mut eng, &mut critic, Budget::tries(MAX_TRIES), move |line| {
            if let Ok(mut j) = log_job.lock() { j.log.push(line.to_string()); }
            log_ctx.request_repaint();
        }).await;
        let untested = run.report.as_ref().map(|r| r.untested_parts.clone()).unwrap_or_default();
        let mut problems = run.problems;
        let passed = run.passed && run.critic_ok != Some(false);
        if run.critic_ok == Some(false) { problems.extend(run.critic_notes); }
        finish(&job, &ctx, Outcome::Authored { follow_up, q: run.question, passed, problems, untested, error: run.error });
    });
}

struct SetupCheck {
    form: Option<Form>,
    engine: String,
}

impl AuthorCritic for SetupCheck {
    async fn critique(&mut self, _: &str, question: &MomQuestion) -> Result<Option<Vec<String>>, String> {
        let Some(form) = self.form.as_ref() else { return Ok(None) };
        if form.help_style == form::HelpStyle::WrittenHelp {
            if !question.detailed_solution_field().is_empty() || !question_help::has_written_help(question) {
                return Ok(Some(vec!["Use $helptext in Common Control for a conceptual hint. Leave the Detailed Solution and ANSWER section empty.".into()]));
            }
        } else if question.detailed_solution_field().is_empty() {
            return Ok(Some(vec!["Add a worked solution in $solutionguide for this draft.".into()]));
        }
        if !form.checks_answer_setup() { return Ok(None); }
        let issue = if matches!(question.qtype.as_str(), "multipart" | "conditional") {
            let source = question.emit();
            for seed in DEFAULT_SEEDS {
                let response = post_engine(&self.engine, "/api/vars", &serde_json::json!({"src": source, "seed": seed})).await;
                let types = response.ok().and_then(|report| {
                    serde_json::from_value::<Vec<String>>(report.pointer("/vars/anstypes")?.clone()).ok()
                }).filter(|parts| !parts.is_empty());
                let Some(types) = types else {
                    return Ok(Some(vec!["The checker could not read the part types. Define the included parts as an ordered array in $anstypes.".into()]));
                };
                if let Some(issue) = form.answer_setup_issue(&types.iter().map(String::as_str).collect::<Vec<_>>()) {
                    return Ok(Some(vec![issue]));
                }
            }
            None
        } else {
            form.answer_setup_issue(&[question.qtype.as_str()])
        };
        Ok(issue.map(|issue| vec![issue]))
    }
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
        let mut chat = ModelChat { cfg: &cfg, tools: false, reference_image: None, video_help: None };
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
                    follow_ups: Vec::new(),
                    src: q.emit(),
                    passed: run.passed,
                    needs_check: false,
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
    if busy(st) { return; }
    let Some(c) = &st.current else { return };
    let source = c.q.emit();
    let saved_id = c.saved_id.clone();
    let job = start(st, if then_save { "Testing and saving" } else { "Testing" });
    let (engine, ctx) = (st.engine.clone(), ctx.clone());
    run_bg(async move {
        let result = HttpEngine { base: engine }.selftest(&source, &DEFAULT_SEEDS).await;
        finish(&job, &ctx, Outcome::Tested { source, saved_id, result, then_save });
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
        follow_ups: c.follow_ups.clone(),
        src: c.q.emit(),
        passed: c.passed == Some(true),
        needs_check: c.passed.is_none(),
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

fn questions_header(ui: &mut Ui, worksheet: bool) {
    // The worksheet mode names itself, so the title, the dock's Convert and the Upload step agree.
    let (heading, sub) = if worksheet {
        ("Convert a Worksheet", "Upload a worksheet, check each question D.A.D writes from it, then add them to MyOpenMath.")
    } else {
        ("MyOpenMath Questions", "Create questions or convert a worksheet, then try the questions before adding them to MyOpenMath.")
    };
    let title = |ui: &mut Ui| pageshell::title(ui, "", heading, sub);
    let dark = theme::is_dark();
    let id = egui::Id::new(("dad_wordmark", dark));
    let ctx = ui.ctx();
    let texture = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)).or_else(|| {
        let bytes: &[u8] = if dark {
            include_bytes!("../assets/dad-logo-dark.png")
        } else {
            include_bytes!("../assets/dad-logo.png")
        };
        let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.into_rgba8();
        let rgba = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize], image.as_raw());
        let texture = ctx.load_texture("D.A.D wordmark", rgba, egui::TextureOptions::LINEAR);
        ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
        Some(texture)
    });
    let Some(texture) = texture else { title(ui); return };
    let available = ui.available_width();
    let width = available.min(208.0);
    let draw_logo = |ui: &mut Ui| {
        let size = texture.size_vec2() * (width / texture.size_vec2().x);
        ui.add(egui::Image::new((texture.id(), size)).alt_text("D.A.D · Dynamic Assessment Developer"));
    };
    if available < 520.0 {
        draw_logo(ui);
        title(ui);
    } else {
        ui.horizontal(|ui| {
            let title_width = available - width - ui.spacing().item_spacing.x;
            ui.allocate_ui_with_layout(egui::vec2(title_width, 0.0), egui::Layout::top_down(egui::Align::Min), title);
            // On the content's right edge, where the columns below end (the title's own width varies).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), draw_logo);
        });
    }
}

fn page(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg, chosen: bool) {
    questions_header(ui, st.worksheet_mode && st.step == Step::Describe);
    ui.add_space(12.0);
    steps(ui, st);
    ui.add_space(12.0);
    let wide = ui.available_width() >= 900.0;
    if !wide && ui.link("My Questions").clicked() {
        st.jump_library = true;
    }
    if wide {
        let side_w = 320.0;
        ui.horizontal_top(|ui| {
            // The gap is the explicit 16 below; egui's item spacing would add 10 on each side of it.
            // Each column gets the spacing back for its own content.
            let spacing = ui.spacing().item_spacing.x;
            ui.spacing_mut().item_spacing.x = 0.0;
            let main_w = ui.available_width() - side_w - 16.0;
            ui.allocate_ui_with_layout(egui::vec2(main_w, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                column(ui, "mom_main", |ui| main(ui, ctx, st, cfg));
            });
            ui.add_space(16.0);
            ui.allocate_ui_with_layout(egui::vec2(side_w, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                column(ui, "mom_side", |ui| side_column(ui, ctx, st, cfg, chosen));
            });
        });
    } else {
        column(ui, "mom_stack", |ui| {
            main(ui, ctx, st, cfg);
            ui.add_space(12.0);
            side_column(ui, ctx, st, cfg, chosen);
        });
    }
}

/// The Questions page's side column: which AI writes (while describing), then My Questions.
/// The AI is a setting beside the work, not a gate in front of it.
fn side_column(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg, chosen: bool) {
    let using_line = !st.worksheet_mode && st.form.line_variant.as_ref().is_some_and(|line| line.enabled);
    if st.step == Step::Describe && !using_line && !busy(st) {
        ai_card(ui, st, cfg, chosen);
        ui.add_space(12.0);
    }
    side(ui, ctx, st);
}

/// The switch for the advanced tools, at the foot of the main column it adds to, with the cards'
/// own inset so it lines up with their content.
fn advanced_switch(ui: &mut Ui, st: &mut State) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        square_checks(ui);
        if ui.checkbox(&mut st.advanced, theme::muted("Show advanced tools").size(13.0))
            .on_hover_text("Question code, test log, and checker").changed()
        {
            let _ = crate::store::save_json(ADVANCED, &st.advanced);
        }
    });
}

/// One scrolling column of the Questions page: the quiet scroller (no rail in the gutter at rest)
/// over a short cue strip that says "More below" while the column really continues.
fn column(ui: &mut Ui, salt: &str, add: impl FnOnce(&mut Ui)) {
    const CUE_H: f32 = 22.0;
    let jump = egui::Id::new((salt, "more_below_jump"));
    let mut area = egui::ScrollArea::vertical().id_salt(salt).max_height((ui.available_height() - CUE_H).max(80.0)).auto_shrink([false, true]);
    if let Some(y) = ui.ctx().data_mut(|d| d.remove_temp::<f32>(jump)) {
        area = area.vertical_scroll_offset(y);
    }
    let out = crate::kit::quiet_scroll_area(ui, area, add);
    let more = out.state.offset.y + out.inner_rect.height() < out.content_size.y - 4.0;
    // The cue is also the way down: a press scrolls most of a screen.
    let sense = if more { egui::Sense::click() } else { egui::Sense::hover() };
    let (bar, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), CUE_H), sense);
    if more {
        crate::kit::more_below(ui, bar, "More below");
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Scroll down"));
        if resp.clicked() {
            let y = (out.state.offset.y + out.inner_rect.height() * 0.8).min(out.content_size.y - out.inner_rect.height());
            ui.ctx().data_mut(|d| d.insert_temp(jump, y));
            ui.ctx().request_repaint();
        }
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
        // A command, not a step: it sits apart from the progress pills, at the row's far end.
        if has_q && !busy(st) && ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            crate::kit::tertiary(ui, egui_phosphor::regular::PLUS, "New Question", 36.0)
                .on_hover_text("Start a new question. Your course and question options stay picked")
        }).inner.clicked() {
            st.step = Step::Describe;
            st.worksheet_mode = false;
            st.current = None;
            st.topic_image = None;
            st.topic_image_error = None;
            st.setup_video_link.clear();
            st.error = None;
            st.preview_html = None;
            if st.form.figure.take().is_some() {
                st.form.line_variant = None;
                st.error = crate::store::save_json(FORM, &st.form).err();
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
            Step::Describe => {
                leave();
                if std::mem::take(&mut st.scroll_to_question) { ui.scroll_to_cursor(Some(egui::Align::Min)); }
                if st.worksheet_mode { worksheet_form(ui, ctx, st, cfg); } else { describe(ui, ctx, st, cfg); }
            }
            Step::Try => try_it(ui, ctx, st, cfg),
            Step::Copy => { leave(); copy_step(ui, ctx, st) }
        }
    }
    advanced_switch(ui, st);
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
        if can_pick && pageshell::primary(ui, "Choose worksheet files").clicked() {
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
    if saved.needs_check { "Needs checking" }
    else if !saved.passed { "Needs fixing" }
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
        control_edges(ui);
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::strong("Your AI").size(15.0));
            // What will actually write the next question, picked or not: one state, one label.
            ui.label(theme::text(ai_label(cfg)).size(14.0));
            if chosen && !open && ui.link("Change").clicked() { st.ai_open = true; }
        });
        if !open { return; }
        ui.add_space(4.0);
        // Every ready-to-use AI in one list, so the card asks one question with one button.
        let mut choices: Vec<(String, &'static str, Option<AiPick>)> = Vec::new();
        if local.contains(&"claude-cli") {
            for (label, model) in CLAUDE_PICKS {
                choices.push((format!("Claude · {label}"), "On this computer. Uses your Claude subscription through Claude Code.",
                    Some(AiPick { provider: "claude-cli", model: model.into(), key: None })));
            }
        }
        if local.contains(&"codex-cli") {
            choices.push(("Codex".into(), "On this computer. Uses your ChatGPT subscription through Codex.",
                Some(AiPick { provider: "codex-cli", model: String::new(), key: None })));
        }
        choices.push(("Keep my current AI".into(), "The AI set up in Settings.", None));
        let mut k = st.ai_choice.min(choices.len() - 1);
        field_label(ui, "Switch to");
        let labels: Vec<&str> = choices.iter().map(|c| c.0.as_str()).collect();
        let w = ui.available_width().min(380.0);
        if dropdown(ui, "mom_ai_choice", &labels, &mut k, w) { st.ai_choice = k; }
        ui.add(egui::Label::new(theme::muted(choices[k].1).size(12.0)).wrap());
        ui.add_space(6.0);
        if crate::kit::secondary(ui, "Use This AI", 36.0).clicked() {
            match choices[k].2.take() {
                Some(pick) => st.ai_pick = Some(pick),
                None => { st.ai_keep = true; st.ai_open = false; }
            }
        }
        ui.add_space(8.0);
        egui::CollapsingHeader::new(theme::strong("Use an OpenRouter key").size(14.0)).id_salt("mom_ai_or").default_open(local.is_empty()).show(ui, |ui| {
            ui.add(egui::Label::new(theme::muted("Make a free account at openrouter.ai, add a few dollars, create a key, and paste it here. It's saved on this computer only.").size(12.0)).wrap());
            let labels: Vec<&str> = OPENROUTER_PICKS.iter().map(|p| p.0).collect();
            let mut k = st.ai_model_pick.min(OPENROUTER_PICKS.len() - 1);
            ui.horizontal_wrapped(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut st.ai_key).password(true).desired_width(260.0).hint_text("sk-or-..."));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "OpenRouter key"));
                let w = ui.available_width().min(300.0);
                if dropdown(ui, "mom_ai_ormodel", &labels, &mut k, w) { st.ai_model_pick = k; }
            });
            let ok = st.ai_key.trim().len() > 10;
            if ui.add_enabled_ui(ok, |ui| crate::kit::secondary(ui, "Use OpenRouter", 36.0)).inner.clicked() && ok {
                st.ai_pick = Some(AiPick { provider: "openrouter", model: OPENROUTER_PICKS[k].1.into(), key: Some(std::mem::take(&mut st.ai_key)) });
            }
        });
        ui.label(theme::muted("Other AIs (Ollama, your own server): Settings, then AI conversion.").size(12.0));
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

fn answer_type_picker(ui: &mut Ui, part: usize, answer: &mut Option<Answer>) {
    let label = |answer: Answer| {
        let (code, name, _) = answer.details();
        format!("{name} ({code})")
    };
    let selected = answer.map_or_else(|| AI_CHOOSES.to_string(), label);
    egui::ComboBox::from_id_salt(("mom_answer_type", part))
        .selected_text(selected)
        .width(ui.available_width().min(300.0))
        .height(470.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(answer, None, AI_CHOOSES)
                .on_hover_text("Choose an answer format that suits the question.");
            for option in Answer::ALL {
                ui.selectable_value(answer, Some(option), label(option))
                    .on_hover_text(option.details().2);
            }
        });
}

fn describe(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    let working = busy(st);
    if let Some((title, _)) = &st.form.figure {
        let title = title.clone();
        let mut remove = false;
        let mut question = None;
        let mut edited = false;
        pageshell::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                pageshell::heading(ui, &title);
                remove = ui.link("Remove figure").clicked();
            });
            if remove { return; }
            if let Some(line) = &mut st.form.line_variant {
                ui.horizontal_wrapped(|ui| {
                    edited |= ui.radio_value(&mut line.enabled, false, "Keep figure fixed").changed();
                    edited |= ui.radio_value(&mut line.enabled, true, "Vary the line with each question").changed();
                });
                if line.enabled {
                    question = line_question_form(ui, line, working, &mut edited);
                }
            } else if title == "Coordinate Grid" {
                ui.add(egui::Label::new(theme::muted(
                    "To vary a line, use a single y = mx + b with whole-number coefficients, a plain x/y grid, and no added titles, equation labels, or points. Include zero and number each unit on both axes."
                ).size(13.0)).wrap());
            }
        });
        if remove || edited {
            if remove {
                st.form.figure = None;
                st.form.line_variant = None;
            }
            if let Err(error) = crate::store::save_json(FORM, &st.form) {
                st.error = Some(error);
            }
        }
        if let Some(q) = question {
            st.request = "Use the numbered line graph to ask for the y-value at the selected x. Keep the plot, correct answer, and worked solution tied to the same line parameters.".into();
            st.current = Some(Current { q, request: st.request.clone(), ..Default::default() });
            st.step = Step::Try;
            st.preview_html = None;
            st.scroll_to_question = true;
            start_test(st, ctx, false);
            return;
        }
        if st.form.line_variant.as_ref().is_some_and(|line| line.enabled) {
            error_box(ui, st);
            return;
        }
        ui.add_space(12.0);
    }
    pageshell::card(ui, |ui| {
        control_edges(ui);
        pageshell::heading(ui, "What question do you need?");
        ui.add(egui::Label::new(theme::muted("Describe it in your own words. The AI decides anything you leave open.").size(13.0)).wrap());
        ui.add_space(8.0);
        let f = &mut st.form;
        // Inset like the dropdowns' text below it.
        let description = ui.add(egui::TextEdit::multiline(&mut f.topic_text).desired_width(f32::INFINITY)
            .margin(egui::Margin::symmetric(12, 8))
            .desired_rows(3).hint_text("For example: Ask students to compare two linear graphs."));
        description.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Describe the question"));

        // Course and Topic side by side: two choices that narrow the same thing.
        let pair_w = ((ui.available_width() - 16.0) / 2.0).clamp(140.0, 320.0);
        ui.horizontal_wrapped(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                field_label(ui, "Course");
                let names: Vec<&str> = COURSES.iter().map(|c| c.0).collect();
                if ai_dropdown(ui, "mom_course", &names, &mut f.course, pair_w) { f.topic = Topic::Ai; f.fix(); }
            });
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                field_label(ui, "Topic");
                let mut topics: Vec<&str> = vec![AI_CHOOSES];
                topics.extend_from_slice(f.topics());
                topics.push("Use my description or picture");
                let other = topics.len() - 1;
                let mut ti = match f.topic { Topic::Ai => 0, Topic::Listed(i) => i + 1, Topic::Typed => other };
                if dropdown(ui, "mom_topic", &topics, &mut ti, pair_w) {
                    f.topic = if ti == 0 { Topic::Ai } else if ti == other { Topic::Typed } else { Topic::Listed(ti - 1) };
                }
            });
        });

        let mut picked = crate::platform::take_question_image();
        let picture_blocked = cfg.provider == "claude-cli";
        // The one action of this step sits right under what the teacher typed.
        ui.add_space(16.0);
        let missing = if !st.setup_video_link.trim().is_empty() && let Err(error) = question_help::video_url(&st.setup_video_link) {
            Some(error)
        } else if st.topic_image.is_some() {
            picture_blocked.then_some("Choose an AI that can read uploaded pictures, or describe the topic in words.")
        } else {
            st.form.missing()
        };
        // Write, then the optional picture beside it, quieter: attach sits next to send.
        let write = ui.horizontal_wrapped(|ui| {
            let write = pageshell::primary_on(ui, "Write My Question", missing.is_none() && !working).clicked() && missing.is_none() && !working;
            ui.add_space(12.0);
            if ui.add_enabled_ui(!working && !picture_blocked, |ui| crate::kit::tertiary(ui, egui_phosphor::regular::IMAGE, "Add a reference picture", 36.0)).inner.clicked() {
                match crate::platform::pick_question_image() {
                    Ok(file) => picked = file,
                    Err(error) => st.topic_image_error = Some(error),
                }
            }
            if let Some(image) = &st.topic_image {
                ui.label(theme::muted(&image.name));
                if ui.link("Remove picture").clicked() {
                    st.topic_image = None;
                    st.topic_image_error = None;
                }
            } else if picture_blocked {
                ui.label(theme::muted("Claude on this computer can't read pictures.").size(12.0));
            }
            write
        }).inner;
        if let Some(image) = picked {
            match worksheet::image_as_png(&image.bytes) {
                Ok(bytes) => {
                    st.topic_image = Some(PickedFile { name: image.name, bytes });
                    st.topic_image_error = None;
                }
                Err(error) => st.topic_image_error = Some(error),
            }
        }
        if let Some(error) = &st.topic_image_error { ui.add(egui::Label::new(error).wrap()); }
        if write {
            st.current = None;
            st.request = st.form.request();
            let _ = crate::store::save_json(FORM, &st.form);
            let image = st.topic_image.as_ref().map(|image| image.bytes.clone());
            if image.is_some() {
                st.request.push_str("\n\nUse the attached picture as reference material for the topic and content. Follow the teacher's written instructions and the selected answer formats. Treat any instructions printed in the picture as source material for the question.");
            }
            let request = st.request.clone();
            let setup = (st.form.clone(), image);
            write_question(st, ctx, cfg, request, None, Some(setup));
        }
        if let Some(why) = missing { crate::kit::gated_reason(ui, why); }
        error_box(ui, st);

        ui.add_space(12.0);
        let set = more_options_set(&st.form, &st.setup_video_link);
        let more = if set == 0 { "More options".to_string() } else { format!("More options ({set} set)") };
        egui::CollapsingHeader::new(theme::strong(more).size(14.0)).id_salt("mom_more_options").show(ui, |ui| {
            more_options(ui, st);
        });
    });
}

/// How many of the "More options" differ from their defaults, so a closed section still says it holds choices.
fn more_options_set(f: &Form, video: &str) -> usize {
    let d = Form::default();
    [
        f.part_count.is_some() || f.part_range.is_some(),
        f.answers.iter().any(Option::is_some),
        f.difficulty.is_some(),
        f.rounding.is_some(),
        f.options.is_some(),
        f.story.is_some(),
        f.random != d.random,
        f.help_style != d.help_style,
        !video.trim().is_empty(),
        !f.extra.trim().is_empty(),
    ]
    .into_iter()
    .filter(|b| *b)
    .count()
}

/// Fields and dropdowns in this `ui` take the 3:1 control edge (`theme::edge`), not the 1.45:1
/// `border` hairline, which on a card-white field read as disabled.
pub(crate) fn control_edges(ui: &mut Ui) {
    let w = &mut ui.visuals_mut().widgets;
    w.inactive.bg_stroke = egui::Stroke::new(1.0, theme::edge());
    w.hovered.bg_stroke = egui::Stroke::new(1.0, theme::edge());
}

/// Checkboxes in this `ui` draw square: the theme's radius 12 turns a 13 px box into a circle that
/// reads as a radio button.
fn square_checks(ui: &mut Ui) {
    let w = &mut ui.visuals_mut().widgets;
    for v in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        v.corner_radius = egui::CornerRadius::same(3);
    }
}

/// Everything past the description: each defaults to the AI's choice.
fn more_options(ui: &mut Ui, st: &mut State) {
    let f = &mut st.form;
    part_count_picker(ui, f);
    let per_part = f.part_range.map_or(f.part_count, |[_, max]| Some(max)).is_some_and(|n| n > 1);
    field_label(ui, if per_part { "MyOpenMath answer type for each part" } else { "MyOpenMath answer type" });
    for i in 0..f.answers.len() {
        ui.horizontal_wrapped(|ui| {
            if per_part {
                let optional = if f.part_range.is_some_and(|[min, _]| i >= min) { " (optional)" } else { "" };
                ui.label(theme::text(format!("Part ({}){optional}", (b'a' + i as u8) as char)));
            }
            answer_type_picker(ui, i, &mut f.answers[i]);
        });
        if let Some(answer) = f.answers[i] {
            ui.add(egui::Label::new(theme::muted(answer.details().2).size(13.0)).wrap());
        }
    }
    if f.answers.iter().any(Option::is_some) {
        ui.checkbox(&mut f.prefer_answer_types, "Let AI choose a different type if it fits better");
    }
    egui::CollapsingHeader::new("Compare MyOpenMath answer types")
        .id_salt("mom_answer_types_help")
        .show(ui, |ui| {
            for answer in Answer::ALL {
                let (code, name, description) = answer.details();
                ui.label(theme::strong(format!("{name} ({code})")));
                ui.add(egui::Label::new(description).wrap());
                ui.add_space(4.0);
            }
            ui.hyperlink_to("MyOpenMath question-type documentation", "https://dev.imathas.com/help.html#questiontypes");
        });

    egui::Grid::new("mom_question_options").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        let width = ((ui.available_width() - 16.0) / 2.0).clamp(88.0, 190.0);
        ui.vertical(|ui| {
            field_label(ui, "Difficulty");
            ai_pick(ui, "mom_diff", &Difficulty::ALL, Difficulty::label, &mut f.difficulty, width);
        });
        if f.uses_rounding() {
            ui.vertical(|ui| {
                field_label(ui, "Round answers to");
                ai_pick(ui, "mom_round", &Rounding::ALL, Rounding::label, &mut f.rounding, width);
            });
        }
        ui.end_row();
        if f.uses_options() {
            ui.vertical(|ui| {
                field_label(ui, "Answer choices");
                let labels = ["2 choices", "3 choices", "4 choices", "5 choices", "6 choices"];
                let mut k = f.options.map(|n| n - 2);
                if ai_dropdown(ui, "mom_opts", &labels, &mut k, width) { f.options = k.map(|k| k + 2); }
            });
        }
        ui.vertical(|ui| {
            field_label(ui, "Real-world situation");
            let mut k = f.story.map(|b| if b { 0 } else { 1 });
            if ai_dropdown(ui, "mom_story", &["Yes, use a story", "No, just the math"], &mut k, width) { f.story = k.map(|k| k == 0); }
        });
    });

    field_label(ui, "Options");
    ui.scope(|ui| { square_checks(ui); ui.checkbox(&mut f.random, "Give every student different numbers") });
    field_label(ui, "Student help");
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut f.help_style, form::HelpStyle::DetailedSolution, "Detailed solution");
        ui.radio_value(&mut f.help_style, form::HelpStyle::WrittenHelp, "Written help");
    });
    let help_description = match f.help_style {
        form::HelpStyle::DetailedSolution => "Work through the question step by step and show the answer.",
        form::HelpStyle::WrittenHelp => "Give students a way to get started without revealing the answer.",
    };
    ui.add(egui::Label::new(theme::muted(help_description).size(13.0)).wrap());
    let video = ui.add(egui::TextEdit::singleline(&mut st.setup_video_link)
        .desired_width(f32::INFINITY).hint_text("Optional video link"));
    video.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Optional video link"));
    ui.add(egui::Label::new(theme::muted("You can also find a video with AI after the question is written.").size(13.0)).wrap());

    let f = &mut st.form;
    field_label(ui, "Anything else? (optional)");
    let r = ui.add(egui::TextEdit::multiline(&mut f.extra).desired_rows(2).desired_width(f32::INFINITY)
        .hint_text("For example: use money amounts, or ask for the y-intercept too"));
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Anything else (optional)"));
}

fn part_count_picker(ui: &mut Ui, form: &mut Form) {
    field_label(ui, "Number of parts");
    let mut mode = if form.part_range.is_some() { 2 } else if form.part_count.is_some() { 1 } else { 0 };
    let before = mode;
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut mode, 0, AI_CHOOSES);
        ui.radio_value(&mut mode, 1, "Exact number");
        ui.radio_value(&mut mode, 2, "Range");
    });
    if mode != before {
        let count = form.part_range.map(|[min, _]| min).or(form.part_count).unwrap_or(1);
        form.part_count = (mode == 1).then_some(count);
        form.part_range = (mode == 2).then_some([count, (count + 1).min(MAX_PARTS)]);
    }
    if let Some([min, max]) = &mut form.part_range {
        ui.horizontal(|ui| {
            let from = ui.label("Minimum");
            ui.add(egui::DragValue::new(min).range(1..=*max)).labelled_by(from.id);
            let to = ui.label("Maximum");
            ui.add(egui::DragValue::new(max).range(*min..=MAX_PARTS)).labelled_by(to.id);
        });
    } else if let Some(count) = &mut form.part_count {
        let label = ui.label("Parts");
        ui.add(egui::DragValue::new(count).range(1..=MAX_PARTS)).labelled_by(label.id);
    }
    form.fix();
}

fn line_question_form(
    ui: &mut Ui,
    line: &mut crate::mom::line_variant::LineVariant,
    working: bool,
    edited: &mut bool,
) -> Option<MomQuestion> {
    ui.add(egui::Label::new(theme::muted(
        "Students read a y-value from a numbered grid with the same axis limits across versions. Enter whole-number ranges, or repeat a value to keep it fixed."
    ).size(13.0)).wrap());
    ui.add_space(8.0);
    egui::Grid::new("mom_line_ranges").spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("");
        ui.label(theme::strong("From"));
        ui.label(theme::strong("To"));
        ui.end_row();
        for (name, range) in [
            ("Slope", &mut line.slope),
            ("Y-intercept", &mut line.intercept),
            ("X-value to read", &mut line.read_x),
        ] {
            ui.label(name);
            for (value, end) in range.iter_mut().zip(["from", "to"]) {
                let response = ui.add(egui::DragValue::new(value).update_while_editing(false));
                *edited |= response.drag_stopped() || (response.changed() && !response.dragged());
                response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, format!("{name} {end}")));
            }
            ui.end_row();
        }
    });
    *edited |= ui.checkbox(&mut line.mark_point, "Mark the point students should read").changed();
    ui.add_space(8.0);
    let question = line.question();
    let create = pageshell::primary_on(ui, "Create graph question", question.is_ok() && !working).clicked();
    match question {
        Ok(question) if create && !working => Some(question),
        Ok(_) if working => {
            crate::kit::gated_reason(ui, "Wait for the current question check to finish.");
            None
        }
        Err(reason) => {
            crate::kit::gated_reason(ui, reason);
            None
        }
        _ => None,
    }
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
                    "✔ Passed 3 automated MyOpenMath checks."
                };
                ui.add(egui::Label::new(RichText::new(verdict).color(theme::green()).size(14.0)).wrap());
            }
            Some(false) => {
                ui.add(egui::Label::new(RichText::new("✖ This question didn’t pass the MyOpenMath checker.").color(theme::red()).size(15.0)).wrap());
                ui.label(theme::muted("Check the details below, or go back and change your choices.").size(13.0));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled_ui(!working, |ui| pageshell::primary(ui, "Rewrite with AI")).inner.clicked() {
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
            None => {
                ui.label(theme::muted("Run the MyOpenMath checker to prepare a student preview."));
                if pageshell::primary(ui, "Check question").clicked() { start_test(st, ctx, true); }
                error_box(ui, st);
                return;
            }
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
        } else if c.q.detailed_solution_field().is_empty() {
            "Answer the question and select Submit Question. Check any written help or attached video in the student preview."
        } else {
            "Answer the question and select Submit Question. Open the Written Example to check the solution, or try a similar question."
        };
        ui.add(egui::Label::new(theme::muted(preview_help).size(13.0)).wrap());
        ui.add_space(6.0);
        let ready = !stale && !working && st.current.as_ref().is_some_and(|c| c.passed == Some(true));
        if !working && stale && st.error.is_some() && st.current.as_ref().is_some_and(|c| c.passed == Some(true)) {
            if pageshell::outline(ui, "Retry student preview").clicked() {
                start_preview(st, ctx);
            }
        } else {
            // Filled until the preview has been opened once; after that "Looks Good" is the step forward.
            let opened = st.current.as_ref().is_some_and(|c| c.preview_opened);
            ui.add_enabled_ui(ready, |ui| ui.horizontal_wrapped(|ui| {
                let label = if ready { "Try It as a Student" } else { "Getting it ready…" };
                let preview = if opened { pageshell::outline(ui, label) } else { pageshell::primary(ui, label) };
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
            if pageshell::primary_on(ui, "Looks Good", confirmed && !working).clicked() && confirmed && !working {
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
            if ui.add_enabled_ui(!working && other_ok, |ui| pageshell::outline(ui, "Rewrite It")).inner.clicked() && other_ok && !working {
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
    if st.step == Step::Try {
        ui.add_space(12.0);
        question_video(ui, ctx, st, cfg);
        ui.add_space(12.0);
        question_follow_ups(ui, ctx, st, cfg);
    }
}

fn question_video(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    let working = busy(st);
    let searching = st.job.lock().is_ok_and(|job| job.busy && job.what == "Finding a video");
    let Some(current) = st.current.as_mut() else { return };
    let attached = question_help::attached_video(&current.q);
    let mut change = None;
    let mut search = None;
    pageshell::card(ui, |ui| {
        egui::CollapsingHeader::new("Help video").id_salt("mom_help_video").show(ui, |ui| {
            if let Some(video) = &attached {
                ui.horizontal_wrapped(|ui| {
                    ui.hyperlink_to("Open attached video", video.as_str());
                    if ui.add_enabled(!working, egui::Button::new("Remove video")).clicked() {
                        change = Some(None);
                    }
                });
            }
            let link = ui.add(egui::TextEdit::singleline(&mut current.video_link_draft)
                .desired_width(f32::INFINITY).hint_text("Paste a video link"));
            link.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Video link"));
            let can_attach = !working && !current.video_link_draft.trim().is_empty();
            if ui.add_enabled_ui(can_attach, |ui| pageshell::outline(ui, "Attach link")).inner.clicked() && can_attach {
                match question_help::video_url(&current.video_link_draft) {
                    Ok(video) => change = Some(Some(video)),
                    Err(error) => current.video_error = Some(error.into()),
                }
            }
            ui.add_space(10.0);
            let topic = ui.add(egui::TextEdit::singleline(&mut current.video_search_topic)
                .desired_width(f32::INFINITY).hint_text("Video topic (defaults to this question’s title)"));
            topic.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Video search topic"));
            if ui.add_enabled(!working, egui::Button::new("Find a video with AI")).clicked() {
                let topic = current.video_search_topic.trim();
                search = Some(if topic.is_empty() { current.q.name.clone() } else { topic.into() });
                current.video_error = None;
                current.video_suggestion = None;
            }
            if searching {
                ui.horizontal(|ui| { ui.spinner(); ui.label("Looking for a lesson…"); });
            }
            if let Some(video) = &current.video_suggestion {
                ui.hyperlink_to(&video.name, video.url.as_str());
                if ui.add_enabled(!working, egui::Button::new("Attach suggested video")).clicked() {
                    change = Some(Some(video.url.clone()));
                }
            }
            if let Some(error) = &current.video_error { ui.add(egui::Label::new(error).wrap()); }
        });
    });
    if let Some(video) = change {
        question_help::set_video(&mut current.q, video.as_ref());
        current.video_link_draft.clear();
        current.video_error = None;
        current.video_suggestion = None;
        current.passed = None;
        current.problems.clear();
        current.preview_opened = false;
        current.preview_confirmed = false;
        current.reviewed = false;
        st.preview_html = None;
        start_test(st, ctx, false);
    } else if let Some(query) = search {
        let source = current.q.emit();
        let (ctx, cfg) = (ctx.clone(), cfg.clone());
        let job = start(st, "Finding a video");
        run_bg(async move {
            let result = question_help::suggest_video(&cfg, &query).await;
            finish(&job, &ctx, Outcome::VideoSearched { source, result });
        });
    }
}

fn question_follow_ups(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    let working = busy(st);
    let revising = st.job.lock().is_ok_and(|job| job.busy && job.what == "Revising");
    let Some(current) = st.current.as_mut() else { return };
    let mut send = None;
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, "Follow up with AI");
        if !current.follow_ups.is_empty() {
            egui::ScrollArea::vertical().id_salt("mom_follow_ups").max_height(165.0)
                .stick_to_bottom(true).show(ui, |ui| {
                    for message in &current.follow_ups {
                        ui.label(theme::strong("You"));
                        ui.add(egui::Label::new(message).wrap());
                        ui.label(theme::muted("D.A.D. revised the question and the checker passed.").size(13.0));
                        ui.add_space(6.0);
                    }
                });
        }
        let input = ui.add_enabled(!working, egui::TextEdit::multiline(&mut current.follow_up_draft)
            .desired_width(f32::INFINITY).desired_rows(3)
            .hint_text("Ask for a change, such as “Keep the graph and add a second part.”"));
        input.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, !working, "Question follow-up"));
        let can_send = !working && !current.follow_up_draft.trim().is_empty();
        if ui.add_enabled_ui(can_send, |ui| pageshell::outline(ui, "Send follow-up")).inner.clicked() && can_send {
            let message = std::mem::take(&mut current.follow_up_draft).trim().to_string();
            let mut context = current.request.clone();
            if !current.follow_ups.is_empty() {
                context.push_str("\n\nRecent follow-ups already reflected in the current draft:");
                for earlier in current.follow_ups.iter().rev().take(8).rev() {
                    context.push_str("\n- ");
                    context.push_str(earlier);
                }
            }
            let request = form::change_request(&context, &current.q.emit(), Change::Other, &message);
            current.follow_up_failed = false;
            send = Some((request, message));
        }
        if revising {
            ui.horizontal(|ui| { ui.spinner(); ui.label("Revising and checking your question…"); });
        } else if current.follow_up_failed {
            ui.add(egui::Label::new("The revision didn’t pass the checker. Your previous question is still available. Edit your follow-up or send it again.").wrap());
        }

        ui.add_space(12.0);
        egui::CollapsingHeader::new("Save my preference").id_salt("mom_writing_preference").show(ui, |ui| {
            ui.add(egui::Label::new("Describe a choice you want D.A.D. to remember for future AI-written questions and revisions. It applies after you save it here.").wrap());
            let field = ui.add(egui::TextEdit::multiline(&mut st.writing_preference)
                .desired_width(f32::INFINITY).desired_rows(3)
                .hint_text("For example: Keep worked solutions short and show units at each step."));
            field.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Question writing preference"));
            if field.changed() { st.preference_feedback = None; }
            if pageshell::outline(ui, "Save preference").clicked() {
                let preference = st.writing_preference.trim();
                st.preference_feedback = Some(match crate::store::save_json(WRITING_PREFERENCE, &preference) {
                    Ok(()) if preference.is_empty() => "Writing preference cleared.".into(),
                    Ok(()) => "Writing preference saved.".into(),
                    Err(error) => format!("Could not save the writing preference. {error}"),
                });
            }
            if let Some(feedback) = &st.preference_feedback { ui.label(feedback); }
        });
    });
    if let Some((request, message)) = send { write_question(st, ctx, cfg, request, Some(message), None); }
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
            "Check the answer and any student help before approving. Esc to return. Ctrl/⌘+Enter to approve."
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
            // Indented to the steps' text (after "3.  "), not past it.
            ui.horizontal(|ui| {
                ui.add_space(17.0);
                if pageshell::outline(ui, &format!("Copy {name}")).clicked() {
                    ui.ctx().copy_text(value.clone());
                    copied = Some(format!("Copied. Now paste it into {place} in MyOpenMath."));
                }
            });
            if *name == "Detailed Solution" {
                ui.horizontal(|ui| {
                    ui.add_space(17.0);
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
            if pageshell::outline(ui, "Copy This Question").clicked() {
                let c = st.current.as_ref().unwrap();
                ui.ctx().copy_text(bookmarklet_package(&c.q));
                st.note = Some("Copied. Now click your Fill MyOpenMath bookmark in MyOpenMath.".into());
            }
        });
    });
}

fn open_saved(st: &mut State, saved: Saved) {
    st.tool_page = None;
    #[cfg(not(target_arch = "wasm32"))]
    { st.poster.clear_result(); }
    st.worksheet_mode = saved.worksheet_source.is_some();
    st.current = Some(Current {
        q: MomQuestion::parse(&saved.src),
        request: saved.request.clone(),
        follow_ups: saved.follow_ups,
        passed: (!saved.needs_check).then_some(saved.passed),
        problems: saved.problems,
        untested: saved.untested,
        worksheet_source: saved.worksheet_source,
        saved_id: Some(saved.id),
        preview_opened: saved.reviewed,
        preview_confirmed: saved.reviewed,
        reviewed: saved.reviewed,
        ..Default::default()
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
                let (status, fill, fg) = if !s.passed && !s.needs_check {
                    (saved_status(s), theme::error_wash(), theme::red())
                } else if s.reviewed {
                    (saved_status(s), theme::accent_faded(), theme::accent_col())
                } else {
                    (saved_status(s), theme::highlighter(), theme::on_highlighter())
                };
                crate::kit::pill(ui, status, fill, fg);
                ui.add_space(4.0);
                if crate::kit::secondary(ui, "Open", 32.0).clicked() { open = Some(s.clone()); }
                // Delete sits at the far end of the row, away from Open.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if st.confirm_delete.as_deref() == Some(s.id.as_str()) {
                        if crate::kit::secondary(ui, "Keep it", 32.0).clicked() { st.confirm_delete = None; }
                        if crate::kit::danger_text_button(ui, "Yes, delete").clicked() { delete = Some(s.id.clone()); }
                    } else if ui.add(egui::Button::new(RichText::new("Delete").color(theme::muted_col())).frame(false).min_size(egui::vec2(0.0, 32.0))).clicked() {
                        st.confirm_delete = Some(s.id.clone());
                    }
                });
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
    if !st.advanced { return; }
    ui.add_space(10.0);
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
    fn follow_up_keeps_the_previous_draft_on_failure_and_requires_review_on_success() {
        let mut st = State {
            current: Some(Current {
                q: MomQuestion { name: "Original".into(), ..Default::default() },
                passed: Some(true),
                reviewed: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let outcome = |passed| Outcome::Authored {
            follow_up: Some("Use cents.".into()),
            q: Some(MomQuestion { name: "Revision".into(), ..Default::default() }),
            passed, problems: Vec::new(), untested: Vec::new(), error: None,
        };
        take(&mut st, outcome(false));
        let current = st.current.as_ref().unwrap();
        assert_eq!(current.q.name, "Original");
        assert!(current.reviewed && current.follow_up_failed);
        assert_eq!(current.follow_up_draft, "Use cents.");
        take(&mut st, outcome(true));
        let current = st.current.unwrap();
        assert_eq!(current.q.name, "Revision");
        assert!(!current.reviewed);
        assert_eq!(current.follow_ups, ["Use cents."]);
    }

    #[test]
    fn checker_result_stays_with_the_draft_it_tested() {
        let first = MomQuestion { name: "Earlier question".into(), ..Default::default() };
        let mut state = State {
            current: Some(Current {
                q: MomQuestion { name: "Current question".into(), ..Default::default() },
                ..Default::default()
            }),
            ..Default::default()
        };
        take(&mut state, Outcome::Tested {
            source: first.emit(),
            saved_id: None,
            result: Ok(SelftestResponse { pass: true, ..Default::default() }),
            then_save: true,
        });
        assert_eq!(state.current.unwrap().passed, None);
        assert!(state.library.questions.is_empty());
    }

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
