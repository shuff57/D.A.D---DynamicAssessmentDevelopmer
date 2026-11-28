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
/// Hover text of "Stop adding". True to what the add does: it waits a moment just before Save.
#[cfg(not(target_arch = "wasm32"))]
const STOP_ADDING_HOVER: &str = "D.A.D waits a moment just before MyOpenMath's Save so you can press this. Press it in that moment and nothing is added. Once Save is clicked, the question stays in MyOpenMath.";

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
    /// Set on a revision of a question already in MyOpenMath (the revision is a new question
    /// with the same name; this is what tells the two apart in My Questions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revised_from: Option<Revision>,
}

/// What a revision of a published question came from. It stays in D.A.D: the name, which is
/// MyOpenMath's Description, is not changed, so nothing here reaches MyOpenMath.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
struct Revision {
    /// Seconds since 1970 when the earlier version was added to MyOpenMath.
    added_at: u64,
    /// 1 for the first revision of a published question, 2 for a revision of that revision, and so on.
    number: u32,
}

impl Revision {
    /// The revision made now of a published question that was itself `previous` (if it was a revision).
    fn after(previous: Option<Revision>, added_at: u64) -> Revision {
        Revision { added_at, number: previous.map_or(0, |p| p.number) + 1 }
    }

    /// The second line on the question's row, and on its finish.
    fn line(&self, now: u64) -> String {
        let date = short_date(self.added_at, now);
        if self.number <= 1 { format!("Revised from the question added on {date}") }
        else { format!("Revision {}, from the question added on {date}", self.number) }
    }
}

/// "Nov 25", with the year when it is not this year's ("Nov 25, 2025"). UTC, like the other dates in the app.
fn short_date(secs: u64, now: u64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let (y, m, d) = crate::time::civil_from_days((secs / 86_400) as i64);
    let (this_year, ..) = crate::time::civil_from_days((now / 86_400) as i64);
    let month = MONTHS[(m - 1) as usize];
    if y == this_year { format!("{month} {d}") } else { format!("{month} {d}, {y}") }
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
    // A .txt is question code by what is in it, even half a question, not a worksheet to transcribe.
    matches!(extension.as_deref(), Some("php" | "json"))
        || matches!(extension.as_deref(), Some("txt" | "md"))
            && std::str::from_utf8(bytes).is_ok_and(|text| !MomQuestion::parse(text).qtext.is_empty() || looks_like_question_code(text))
}

/// The records of a "Back Up My Questions" file (a whole library), or None for any other file.
fn backup_records(file: &PickedFile) -> Option<Vec<Saved>> {
    let text = std::str::from_utf8(&file.bytes).ok()?;
    let library: Library = serde_json::from_str(text).ok()?;
    (!library.questions.is_empty()).then_some(library.questions)
}

fn imported_sources(file: &PickedFile) -> Result<Vec<String>, String> {
    let text = std::str::from_utf8(&file.bytes).map_err(|_| "Save the question as a UTF-8 file and try again.")?;
    if text.trim_start().starts_with('{') || file.name.to_ascii_lowercase().ends_with(".json") {
        let package: ShareFile = serde_json::from_str(text)
            .map_err(|_| "Choose a D.A.D question package containing a questions list.")?;
        Ok(package.questions.into_iter().map(|question| question.src).collect())
    } else {
        Ok(vec![unwrap_reply(text)])
    }
}

/// Question code copied out of a chat with an AI: drop the sentence before it and the ``` fences
/// around it, as the writer's own replies are read (`author::parse_reply`).
fn unwrap_reply(text: &str) -> String {
    let code = fenced_or_raw(text);
    // Both MyOpenMath boxes in one paste without D.A.D's markers (a reply copied as text, or a
    // teacher's notes): split them, so it is one question and not "Common Control only".
    // Only when the parser finds no Question Text itself (the older `//question text` and `///`
    // layouts it already reads stay its job).
    if !code.contains("// === ") && MomQuestion::parse(&code).qtext.trim().is_empty() && let Some((control, qtext)) = split_boxes(&code) {
        return joined_paste(&control, &qtext);
    }
    code
}

/// Common Control and Question Text in one text: at a "Question Text" heading line, else where the
/// code ends and the lines with `$answerbox` begin. None when it is not both.
fn split_boxes(text: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let heading = |l: &str, name: &str| {
        let t = l.trim().trim_start_matches('/').trim().trim_end_matches(':').trim().to_ascii_lowercase();
        t == name
    };
    if let Some(i) = lines.iter().position(|l| heading(l, "question text")) {
        let control: Vec<&str> = lines[..i].iter().copied().filter(|l| !heading(l, "common control")).collect();
        let qtext = lines[i + 1..].join("\n");
        return (!qtext.trim().is_empty() && assigns_a_variable(&control.join("\n"))).then(|| (control.join("\n"), qtext));
    }
    if !assigns_a_variable(text) || !text.contains("$answerbox") { return None; }
    let code_line = |l: &str| { let t = l.trim(); t.starts_with("//") || assigns_a_variable(t) || crate::mom::format::looks_like_code(t) };
    let last = lines.iter().rposition(|l| code_line(l))?;
    let qtext = lines[last + 1..].join("\n");
    qtext.contains("$answerbox").then(|| (lines[..=last].join("\n"), qtext))
}

fn fenced_or_raw(text: &str) -> String {
    let fence = |l: &&str| l.trim_start().starts_with("```");
    // A fenced block is the code, whatever is in it (as every chat app shows it): from the opening
    // fence to the closing one, or to the end when the closing fence was not copied.
    if text.lines().any(|l| fence(&l)) {
        // Every fenced block, in order (an unclosed last one runs to the end).
        let mut blocks: Vec<Vec<&str>> = Vec::new();
        let mut open: Option<Vec<&str>> = None;
        for line in text.lines() {
            if fence(&line) {
                match open.take() { Some(block) => blocks.push(block), None => open = Some(Vec::new()) }
            } else if let Some(block) = open.as_mut() {
                block.push(line);
            }
        }
        blocks.extend(open);
        let blocks: Vec<String> = blocks.into_iter().map(|b| b.join("\n")).collect();
        // Chat apps often answer "Common Control: ```…``` Question Text: ```…```": two blocks with no
        // D.A.D markers are the two MyOpenMath boxes, whichever order they came in.
        if let [a, b, ..] = blocks.as_slice() && !text.contains("// === ") {
            // Only when they have the two boxes' shape: one sets variables, the other shows answer
            // boxes and sets none ("Version 1 / Version 2" stays the first block).
            let boxes = |c: &str, q: &str| assigns_a_variable(c) && q.contains("$answerbox") && !assigns_a_variable(q);
            if boxes(a, b) { return joined_paste(a, b); }
            if boxes(b, a) { return joined_paste(b, a); }
        }
        return blocks.into_iter().next().unwrap_or_default();
    }
    // No fence: drop any sentence before D.A.D's first section marker.
    let start = text.find("// === ").unwrap_or(0);
    text[start..].to_string()
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

/// Beside a file button: the window also takes dropped files (desktop), as file apps do.
fn drop_hint(ui: &mut Ui) {
    if cfg!(not(target_arch = "wasm32")) {
        ui.add_space(10.0);
        ui.label(theme::muted("or drop files on this window").size(13.0));
    }
}

fn import_questions(ui: &mut Ui, st: &mut State) {
    let before = st.question_files.len();
    st.question_files.extend(crate::platform::take_picked());
    if st.question_files.len() > before { st.import_hint = None; }
    pageshell::title(ui, "", "Import MyOpenMath questions",
        "Bring in MyOpenMath question code, a D.A.D package from another teacher, or your own backup.");
    // A worksheet is the other "bring it in" job, and it has its own place: say so before the picker
    // refuses the file.
    if crate::kit::link(ui, "Have a worksheet (PDF, Word or a photo)? Use Convert instead.", true).clicked() {
        request_destination(ui.ctx(), Destination::Convert);
    }
    ui.add_space(12.0);
    // The page's column: the quiet bar and the clickable More below cue, as on Questions.
    column(ui, "mom_import_page", |ui| {
        pageshell::card(ui, |ui| {
            // Inputs look like inputs: the same 3:1 edges as the Questions form.
            control_edges(ui);
            if ui.horizontal_wrapped(|ui| {
                    // With nothing pasted or chosen, picking files is the one available action, so it is
                    // the filled one; once there is something to import, Import is.
                    let r = if st.import_source.trim().is_empty() && st.question_files.is_empty() {
                        pageshell::primary(ui, "Choose question files")
                    } else {
                        pageshell::outline(ui, "Choose question files")
                    };
                    drop_hint(ui);
                    r
                }).inner.clicked()
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
            // A list of chosen files, each with Remove, as file pickers in other apps have.
            let mut remove = None;
            for (i, file) in st.question_files.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(theme::text(&file.name));
                    if crate::kit::link(ui, "Remove", false).clicked() { remove = Some(i); }
                });
            }
            if let Some(i) = remove { st.question_files.remove(i); }
            // It takes more than code (a chat reply, a problem in words), so the label says "a question".
            let caption = ui.label(theme::strong("Or paste a question"));
            // Fixed ids for both boxes: the rows around them come and go with the hint, and an id
            // taken from position would change, losing focus (and the focus moved to the second box).
            let paste = ui.add(egui::TextEdit::multiline(&mut st.import_source).id_salt("mom_import_paste").desired_width(f32::INFINITY).desired_rows(5)
                .margin(egui::Margin::symmetric(12, 8))
                .hint_text("MyOpenMath code, a reply from an AI chat, or a problem in your own words"))
                .labelled_by(caption.id);
            if paste.changed() {
                st.import_hint = None;
                // A reply pasted from an AI chat is cleaned in the box itself, so what will be
                // imported is what is shown; the reply as pasted stays one press away.
                if st.import_source.contains("```") {
                    let code = fenced_or_raw(&st.import_source);
                    if !code.trim().is_empty() {
                        st.import_original = Some(std::mem::replace(&mut st.import_source, code));
                    }
                } else {
                    st.import_original = None;
                }
            }
            let from_chat = st.import_original.is_some();
            if from_chat {
                let undo = ui.horizontal_wrapped(|ui| {
                    ui.label(theme::muted("Removed the AI chat's own words around the code: this is the code D.A.D will import.").size(12.5));
                    crate::kit::link(ui, "Undo", false).clicked()
                }).inner;
                if undo {
                    if let Some(original) = st.import_original.take() { st.import_source = original; }
                }
            }
            // Esc backs out of a hint the last press raised; nothing typed is lost.
            if st.import_hint.is_some() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) { st.import_hint = None; }
            let paste_send = paste.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
            // One filled button: while the paste waits on a next step (its Question Text, or writing
            // a question from words), that step's button is the one.
            let waiting = st.import_hint.is_some();
            let can_import = !busy(st) && (!st.question_files.is_empty() || !st.import_source.trim().is_empty());
            let shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter));
            if !waiting {
                // The shortcut other apps use to send, shown beside the button it presses.
                let pressed = ui.horizontal(|ui| {
                    let r = pageshell::primary_on(ui, "Import into My Questions", can_import).clicked();
                    ui.label(theme::muted(format!("or {shortcut}")).size(12.0));
                    r
                }).inner;
                if (pressed || paste_send) && can_import { import_drafts(st); }
            }
            match st.import_hint.clone() {
                Some(NotAQuestion::Words(words)) => {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(theme::strong("This looks like a problem in words, not MyOpenMath code.").size(14.0)).wrap());
                    ui.add(egui::Label::new(theme::muted("D.A.D can write a MyOpenMath question from it.").size(13.0)).wrap());
                    if pageshell::primary(ui, "Write a Question From It").clicked() {
                        // A description typed on step 1 but not written yet is a draft: the words go
                        // under it. One left from a question already written is not kept.
                        let draft = if st.current.is_none() { st.form.topic_text.trim_end().to_string() } else { String::new() };
                        start_new_question(st);
                        st.form.topic_text = if draft.is_empty() { words } else { format!("{draft}\n{words}") };
                        st.write_now = true;
                        st.error = crate::store::save_json(FORM, &st.form).err();
                        st.import_source.clear();
                        st.import_hint = None;
                        request_destination(ui.ctx(), Destination::Questions);
                    }
                }
                Some(NotAQuestion::Link(url)) => {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(theme::strong("That's a link. D.A.D can't read web pages.").size(14.0)).wrap());
                    // Steps, one per line, that stay on screen while the teacher is in the browser.
                    let steps: &[&str] = if url.contains("myopenmath.com") {
                        &["1. Open the link (the button below) and go to that question's editor in MyOpenMath. Not your question? Ask its author for its Common Control and Question Text boxes.",
                          "2. Copy its Common Control box and paste it in the box above, in place of the link.",
                          "3. D.A.D then shows a second box for its Question Text."]
                    } else {
                        &["If it's a worksheet: save or print it as a PDF and drop the PDF on Convert.",
                          "If it's MyOpenMath question code: copy the code itself and paste it above, in place of the link."]
                    };
                    for step in steps { ui.add(egui::Label::new(theme::muted(*step).size(13.0)).wrap()); }
                    if pageshell::primary(ui, "Open It in Your Browser").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        // The link has done its job: empty the box so the code pasted next replaces it.
                        st.import_source.clear();
                    }
                }
                Some(hint @ (NotAQuestion::ControlOnly(_) | NotAQuestion::QtextOnly(_))) => {
                    ui.add_space(8.0);
                    let (title, why) = if matches!(hint, NotAQuestion::ControlOnly(_)) {
                        // Where the missing half lives depends on where the code came from.
                        if from_chat {
                            ("Now paste the question's text", "The code above has no Question Text. Ask the AI chat for the question's text too and paste it here, or paste MyOpenMath's Question Text box.")
                        } else {
                            ("Now paste the question's text", "The code above has no Question Text. In MyOpenMath's question editor it is the box labelled Question Text, under Common Control.")
                        }
                    } else {
                        ("Now paste MyOpenMath's Common Control box", "The text above is a Question Text box: it shows answer boxes but sets nothing. In MyOpenMath's editor, Common Control is the box above it.")
                    };
                    let caption = ui.label(theme::strong(title));
                    ui.add(egui::Label::new(theme::muted(why).size(13.0)).wrap());
                    let other = ui.add(egui::TextEdit::multiline(&mut st.import_qtext).id_salt("mom_import_second").desired_width(f32::INFINITY).desired_rows(3)
                        .margin(egui::Margin::symmetric(12, 8)))
                        .labelled_by(caption.id);
                    // The next paste goes here, so the cursor does too (not left in the first box).
                    if std::mem::take(&mut st.import_focus_second) { other.request_focus(); }
                    let ok = !busy(st) && !st.import_qtext.trim().is_empty();
                    let send = other.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
                    let pressed = ui.horizontal(|ui| {
                        let r = pageshell::primary_on(ui, "Import into My Questions", ok).clicked();
                        ui.label(theme::muted(format!("or {shortcut}")).size(12.0));
                        r
                    }).inner;
                    if (pressed || send) && ok {
                        import_drafts(st);
                    }
                }
                None => {}
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

/// A fresh step 1. The question on screen goes to My Questions first (as "Needs review" if it was
/// not approved), so starting another never throws away what the AI just wrote.
fn start_new_question(st: &mut State) {
    save_current(st);
    st.add_elsewhere = false;
    // "New" means a blank description; course, topic and question options stay picked.
    st.form.topic_text.clear();
    st.step = Step::Describe;
    st.worksheet_mode = false;
    st.current = None;
    st.topic_image = None;
    st.topic_image_error = None;
    st.error = None;
    st.preview_html = None;
    if st.form.figure.take().is_some() {
        st.form.line_variant = None;
        st.error = crate::store::save_json(FORM, &st.form).err();
    }
}

/// What a paste that is not a whole question is, so Import can say what to do with it instead of
/// naming a missing section.
#[derive(Clone, Debug, PartialEq)]
enum NotAQuestion {
    /// A web address: D.A.D can't read MyOpenMath pages, but it can open this one in the browser.
    Link(String),
    /// A problem in words: it is a description for step 1.
    Words(String),
    /// Code with no Question Text: most often MyOpenMath's Common Control box on its own.
    ControlOnly(String),
    /// MyOpenMath's Question Text box on its own (it shows `$answerbox`, but sets nothing).
    QtextOnly(String),
}

fn not_a_question(text: &str) -> NotAQuestion {
    let t = text.trim();
    // A link anywhere in a short paste is the link ("Here is the question: https://…"), as a mail
    // app or an address box reads it; code with a link in a comment stays code.
    let link = t.split_whitespace().map(|w| w.trim_matches(|c: char| matches!(c, '(' | ')' | '<' | '>' | ',' | '"' | '\''))
        .trim_end_matches('.'))
        .find(|w| w.starts_with("http://") || w.starts_with("https://") || w.starts_with("www.") || w.contains("myopenmath.com/"));
    if let Some(w) = link && t.lines().count() <= 3 && !looks_like_question_code(t) {
        let url = if w.starts_with("http") { w.to_string() } else { format!("https://{w}") };
        return NotAQuestion::Link(url);
    }
    let sets = assigns_a_variable(t);
    if !sets && t.contains("$answerbox") { return NotAQuestion::QtextOnly(t.to_string()); }
    if sets || looks_like_question_code(t) { NotAQuestion::ControlOnly(t.to_string()) } else { NotAQuestion::Words(t.to_string()) }
}

/// A line that sets an IMathAS variable: `$a = …`, `$answer[0] = …`.
fn assigns_a_variable(text: &str) -> bool {
    text.lines().any(|line| {
        let t = line.trim_start();
        t.strip_prefix('$').is_some_and(|rest| {
            let name = rest.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'[' || *b == b']').count();
            let after = rest[name..].trim_start();
            // "$y = 2x + 3$. What is the slope?" is math written with $…$: a `$` that closes it (then
            // a space, punctuation or the end) is not a variable.
            let closes = after.char_indices().any(|(i, c)| c == '$' && after[i + 1..].chars().next().is_none_or(|n| n.is_whitespace() || ".,;:!?)".contains(n)));
            name > 0 && after.starts_with('=') && !after.starts_with("==") && !closes
        })
    })
}

/// The shape of IMathAS code, not just its characters: a math problem in words has `$` in its
/// prices ("A shirt costs $25"), so a `$` alone is not code. Code assigns a variable (`$a = …`),
/// calls a macro on a line of its own, or carries D.A.D's section markers.
fn looks_like_question_code(text: &str) -> bool {
    // A first line like "$5,000 is invested…" or "$f(x) = 3x$" is a problem in words (a price, or
    // math written with $…$), not code, though it starts with `$`.
    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let prose_dollar = first.strip_prefix('$').is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()) || rest.contains('$'));
    // "Factor (x^2 - 9)" or "Simplify (x+2)(x-3)" is an instruction in words: IMathAS statements
    // start with `$`, a lowercase keyword or a lowercase macro written right against its `(`.
    let prose_call = first.starts_with(|c: char| c.is_ascii_uppercase())
        || first.split_once('(').is_some_and(|(name, _)| name.ends_with(' ')
            && !["if", "for", "foreach", "while"].contains(&name.trim()));
    let looks = crate::mom::format::looks_like_code(text) && !prose_dollar && !prose_call;
    assigns_a_variable(text) || text.contains("// ===") || text.contains("$answerbox") || text.contains("rand(") || looks
}

/// MyOpenMath's two editor boxes, pasted apart, as one question.
fn joined_paste(control: &str, qtext: &str) -> String {
    format!("// === COMMON CONTROL ===\n{}\n\n// === QUESTION TEXT ===\n{}\n", control.trim(), qtext.trim())
}

fn import_drafts(st: &mut State) {
    // A Common Control paste waiting for its Question Text: the two boxes make one question.
    if !st.import_qtext.trim().is_empty() {
        match &st.import_hint {
            Some(NotAQuestion::ControlOnly(control)) => st.import_source = joined_paste(control, &st.import_qtext),
            Some(NotAQuestion::QtextOnly(qtext)) => st.import_source = joined_paste(&st.import_qtext, qtext),
            _ => {}
        }
    }
    st.import_hint = None;
    let mut known: std::collections::HashSet<String> = st.library.questions.iter()
        .map(|draft| MomQuestion::parse(&draft.src).emit()).collect();
    let mut errors = Vec::new();
    let mut drafts = Vec::new();
    let mut duplicates = 0;
    // A chosen file that is half a question: its code moves into the paste box, where the box for
    // the other half appears, so the message never points at a box that is not there.
    let mut unfinished: Vec<(String, String)> = Vec::new();
    let paste = PickedFile { name: "Pasted code".into(), bytes: st.import_source.as_bytes().to_vec() };
    for file in st.question_files.iter().chain((!st.import_source.trim().is_empty()).then_some(&paste)) {
        // A "Back Up My Questions" file holds whole records: restore them as they were (approved
        // stays approved), each under a fresh id, instead of as drafts to check again.
        if let Some(saved) = backup_records(file) {
            for mut record in saved {
                let src = MomQuestion::parse(&record.src).emit();
                if !known.insert(src) { duplicates += 1; continue; }
                record.id = crate::model::new_id();
                record.worksheet_origin = None;
                drafts.push(record);
            }
            continue;
        }
        match imported_sources(file) {
            Ok(sources) => for src in sources {
                let mut q = MomQuestion::parse(&src);
                if q.qtext.trim().is_empty() {
                    if std::ptr::eq(file, &paste) {
                        st.import_hint = Some(not_a_question(&src));
                        st.import_focus_second = true;
                    } else {
                        unfinished.push((file.name.clone(), src));
                    }
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
    let moved = st.import_hint.is_none() && st.import_source.trim().is_empty() && !unfinished.is_empty();
    if moved {
        let (name, src) = &unfinished[0];
        st.import_source = src.clone();
        st.import_hint = Some(not_a_question(src));
        st.import_focus_second = true;
        // The other chosen files are in (or were duplicates): clear the list so a second press
        // does not bring them in again, and the half question's box is the one next step.
        let _ = name;
        st.question_files.clear();
    }
    for (name, _) in unfinished.iter().skip(usize::from(moved)) {
        errors.push(format!("{name} is half a question (no Question Text). Import it on its own to add the other half."));
    }
    let count = drafts.len();
    let before = st.library.questions.len();
    st.library.questions.extend(drafts);
    if count > 0 && let Err(error) = crate::store::save_json(LIBRARY, &st.library) {
        st.library.questions.truncate(before);
        st.error = Some(format!("Could not save the imported questions: {error}"));
        return;
    }
    let drafts = if count == 1 { "question" } else { "questions" };
    let repeated = if duplicates == 1 { "duplicate" } else { "duplicates" };
    // "Imported 0 questions" beside an explanation only contradicts it.
    if count > 0 || duplicates > 0 {
        st.lib_note = Some(if duplicates == 0 { format!("Imported {count} {drafts}.") } else { format!("Imported {count} {drafts}. Skipped {duplicates} {repeated}.") });
    }
    // Files that came in leave the list even while a hint about the paste is up; the paste stays
    // for its next step.
    if errors.is_empty() { st.question_files.clear(); }
    if errors.is_empty() && st.import_hint.is_none() {
        st.import_source.clear();
        st.import_qtext.clear();
    }
    st.error = (!errors.is_empty()).then(|| errors.join("\n"));
}

/// A check problem in a teacher's words: the checker's own terms ("seed", "IMathAS") are for the AI
/// that repairs the question, not for the row a teacher reads.
fn plain_problem(problem: &str) -> String {
    // The checker's most common finding, said in one short line (the full sentence doubled the row).
    if problem.starts_with("The answer the code computes is not right") {
        return "The answer is wrong for every set of numbers tried.".into();
    }
    problem
        .replace(", at every seed", " for every set of numbers it was tested with")
        .replace(" at every seed", " for every set of numbers it was tested with")
        .replace("at some seeds", "for some sets of numbers")
        .replace("IMathAS", "MyOpenMath")
}

/// A saved question's state in words and colour, the same four as My Questions.
fn share_status(s: &Saved, published: &std::collections::HashSet<String>) -> (&'static str, egui::Color32) {
    if published.contains(&s.id) { ("In MyOpenMath", theme::green()) }
    else if !s.passed && !s.needs_check { (saved_status(s), theme::red()) }
    else if s.reviewed { ("Ready to add", theme::accent_col()) }
    else { (saved_status(s), theme::on_attention()) }
}

fn share_questions(ui: &mut Ui, st: &mut State) {
    pageshell::title(ui, "", "Share MyOpenMath questions",
        "Download a D.A.D package another teacher can import with Import.");
    ui.add_space(12.0);
    let mut export = None;
    let mut export_ready: Option<Vec<Saved>> = None;
    // The page's column: the quiet bar and the clickable More below cue, as on Questions.
    column(ui, "mom_share_page", |ui| {
        pageshell::card(ui, |ui| {
            if st.library.questions.is_empty() {
                ui.label(theme::muted("Create, convert, or import a question to share it here."));
            } else {
                // The questions ready to share come first; "all" includes drafts still being fixed or
                // reviewed, so it is the quieter choice and says so.
                if st.published.is_none() { st.published = Some(published_ids()); }
                let ready: Vec<Saved> = st.library.questions.iter()
                    .filter(|s| st.published.as_ref().is_some_and(|p| p.contains(&s.id)) || (s.reviewed && s.passed))
                    .cloned().collect();
                ui.horizontal_wrapped(|ui| {
                    if pageshell::primary_on(ui, &format!("Download {} ready question{}", ready.len(), if ready.len() == 1 { "" } else { "s" }), !ready.is_empty()).clicked() && !ready.is_empty() {
                        export_ready = Some(ready.clone());
                    }
                    if pageshell::outline(ui, "Download all questions").clicked() {
                        export = Some(st.library.questions.as_slice());
                    }
                });
                ui.add(egui::Label::new(theme::muted("All includes questions that still need review or fixing.").size(12.0)).wrap());
            }
            if st.published.is_none() { st.published = Some(published_ids()); }
            let published = st.published.clone().unwrap_or_default();
            for draft in &st.library.questions {
                ui.push_id(&draft.id, |ui| {
                    ui.separator();
                    let q = MomQuestion::parse(&draft.src);
                    // The name leads, the description is quieter (as in My Questions), and one
                    // question's download is a small action on its own row, so Download all stays
                    // the page's one large button.
                    let (title, about) = q.name.split_once(" - ").map_or((q.name.as_str(), ""), |(t, a)| (t.trim(), a.trim()));
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(theme::strong(title)).wrap());
                        // Its state, as in My Questions, so a question that needs fixing isn't sent
                        // on by mistake.
                        let (status, color) = share_status(draft, &published);
                        if published.contains(&draft.id) { crate::kit::status_done_w(ui, status, color, 0.0); } else { crate::kit::status_tag(ui, status, color); }
                        // Download right after what it downloads, not across the full width.
                        // Share only downloads: opening a question is My Questions' job.
                        ui.add_space(8.0);
                        if crate::kit::tertiary(ui, egui_phosphor::regular::DOWNLOAD_SIMPLE, "Download", 32.0).clicked() {
                            export = Some(std::slice::from_ref(draft));
                        }
                    });
                    if !about.is_empty() { ui.add(egui::Label::new(theme::muted(about).size(12.0)).wrap()); }
                });
            }
            if let Some(note) = &st.lib_note { ui.label(theme::muted(note)); }
            error_box(ui, st);
        });
    });
    let ready_owned = export_ready;
    let export = export.or(ready_owned.as_deref());
    if let Some(questions) = export {
        match export_shared_questions(questions) {
            Ok(true) => { st.error = None; st.lib_note = Some("Question package saved.".into()); }
            Ok(false) => {}
            Err(error) => st.error = Some(format!("Could not save the question package: {error}")),
        }
    }
}

/// The question on the right.
#[derive(Clone, Default)]
struct Current {
    q: MomQuestion,
    request: String,
    follow_ups: Vec<String>,
    follow_up_draft: String,
    /// The change being sent now, put back in the box if the teacher stops it.
    follow_up_pending: String,
    follow_up_failed: bool,
    /// Why the last rewrite failed, in the teacher's words (the AI's error, or the checker).
    follow_up_why: Option<String>,
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
    /// Set when this is a revision of a question already in MyOpenMath.
    revised_from: Option<Revision>,
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
    /// Counts jobs (and stops): a write the teacher stopped finds a different number when it ends,
    /// and its result is dropped.
    generation: u64,
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
    /// The key just saved in the AI card, being tried against its AI (named, so a result that comes
    /// back after the teacher switched AI acts on the AI it was for).
    key_check: Option<(String, String, Arc<Mutex<Option<Result<(), String>>>>)>,
    /// What that try found, for which AI: the sentence, and whether it is a problem.
    key_check_msg: Option<(String, String, bool)>,
    /// The key field in the AI card shows its text.
    ai_key_shown: bool,
    /// Write as soon as step 1 is drawn ("Write a Question From It" on Import).
    write_now: bool,
    /// Said on the writing card: a press that waits for the write to finish.
    wait_note: Option<String>,
    /// When the write on screen started (egui time, seconds).
    write_started: f64,
    /// A question already in MyOpenMath, opened to add it to another assessment too.
    add_elsewhere: bool,
    /// A question was written while the teacher was on another screen; the dock says "Ready".
    ready_unseen: bool,
    /// A job failed while the teacher was on another screen; the dock says "Needs a look".
    failed_unseen: bool,
    /// "I'm done with this worksheet" was pressed and waits for its confirm.
    confirm_close_worksheet: bool,
    /// Saved questions with a receipt saying they are in a MyOpenMath assessment (read once, again
    /// after each publish).
    published: Option<std::collections::HashSet<String>>,
    /// What a dropped photo became, said once beside it.
    topic_image_note: Option<String>,
    /// The paste was not a whole question: what it looks like, and so what to do instead.
    import_hint: Option<NotAQuestion>,
    /// MyOpenMath's Question Text box, when the paste above was its Common Control box.
    import_qtext: String,
    worksheet_save_error: Option<String>,
    scroll_to_question: bool,
    form: Form,
    topic_image: Option<PickedFile>,
    topic_image_error: Option<String>,
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
    /// The highlighted entry in the card's AI dropdown.
    ai_choice: usize,
    /// A choice made in the card, applied to the app's settings after the frame.
    ai_pick: Option<AiPick>,
    /// The teacher (or the first-open pick) has chosen an AI; read once a frame from `AI_PICKED`.
    ai_chosen: bool,
    /// An AI-chat reply as it was pasted into Import, before the box was cleaned to its code (Undo).
    import_original: Option<String>,
    /// The Import hint just appeared: put the cursor in its second box, where the next paste goes.
    import_focus_second: bool,
    /// A saved question was just opened: scroll My Questions back to its top.
    side_to_top: bool,
    /// A My Questions count was pressed: scroll to the first question of that group (0 fix, 1 review, 2 ready).
    jump_to_group: Option<u8>,
    /// "Keep my current AI" was pressed.
    ai_keep: bool,
    /// The first-open check that picks a working AI by itself has run.
    ai_auto_checked: bool,
    /// A key typed into the AI card for the AI in use (when it has none).
    ai_card_key: String,
    /// "Sign In to D.A.D" was pressed in the AI card: the app opens the Account window.
    open_account: bool,
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

thread_local! {
    /// The label as of the Questions page's last frame, for the dock while that page holds the
    /// state (Import and Share are drawn by it).
    static JOB_LABEL: std::cell::Cell<Option<(usize, &'static str)>> = const { std::cell::Cell::new(None) };
}

/// What a running job is called, for the dock while the teacher is on another screen. None when
/// nothing runs, or while the Questions page itself is drawing (it shows the job in full).
/// The dock item (0 Questions, 1 Convert) and what to call it while a job runs elsewhere.
pub fn job_label() -> Option<(usize, &'static str)> {
    ST.with(|state| {
        let Ok(st) = state.try_borrow() else { return JOB_LABEL.with(std::cell::Cell::get) };
        job_label_of(&st).or_else(|| JOB_LABEL.with(std::cell::Cell::get))
    })
}

fn job_label_of(st: &State) -> Option<(usize, &'static str)> {
    let job = st.job.lock().ok()?;
    // Finished while the teacher was elsewhere (collected or still waiting to be): say so until
    // they are back on Questions.
    let written = job.outcomes.iter().any(|o| matches!(o, Outcome::Authored { q: Some(_), .. }));
    if !job.busy && (st.ready_unseen || written) { return Some((0, "Ready ✔")); }
    // Ended badly while the teacher was elsewhere: it is not silent either.
    if !job.busy && st.failed_unseen { return Some((0, "Needs a look")); }
    job.busy.then_some(match job.what {
        "Writing" => (0, "Writing…"),
        "Revising" => (0, "Rewriting…"),
        "MyOpenMath" => (0, "Adding…"),
        // A worksheet job lives on Convert, so its word goes there.
        "Reading worksheet" => (1, "Reading…"),
        "Converting worksheet" => (1, "Converting…"),
        _ => (0, "Working…"),
    })
}

/// Ctrl/Cmd+N on the Questions screen: the same as the New Question command.
pub fn new_question_shortcut() {
    ST.with(|state| {
        let mut st = state.borrow_mut();
        if !busy(&st) { start_new_question(&mut st); st.tool_page = None; }
    });
}

/// Step 1 of a new question is on screen, where a dropped photo is the reference picture (as an
/// image dropped on a message box attaches to it), not a worksheet to convert.
/// `reads_pictures`: the AI in use can read a picture (Claude on this computer can't), else a photo
/// goes on to Convert as before.
pub fn picture_drop_target(reads_pictures: bool) -> bool {
    reads_pictures && ST.with(|state| {
        let st = state.borrow();
        st.tool_page.is_none() && !st.worksheet_mode && st.step == Step::Describe && !busy(&st)
    })
}

/// A photo dropped on step 1 becomes the question's reference picture. False when step 1 is not
/// on screen or the file is not a picture, so the caller routes it as usual.
pub fn take_dropped_picture(name: &str, bytes: &[u8], reads_pictures: bool) -> bool {
    let picture = name.rsplit_once('.').is_some_and(|(_, ext)| ["png", "jpg", "jpeg", "webp"].iter().any(|e| ext.eq_ignore_ascii_case(e)));
    if !picture || !picture_drop_target(reads_pictures) { return false; }
    ST.with(|state| {
        let mut st = state.borrow_mut();
        match worksheet::image_as_png(bytes) {
            Ok(png) => {
                st.topic_image = Some(PickedFile { name: name.into(), bytes: png });
                st.topic_image_error = None;
                st.topic_image_note = Some("Added as this question's picture. To turn a worksheet photo into questions, use Convert.".into());
            }
            Err(error) => st.topic_image_error = Some(error),
        }
    });
    true
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
        // The state is out of ST while this page draws; the dock reads the job from here.
        JOB_LABEL.with(|l| l.set(job_label_of(&st)));
        if !st.inited {
            st.inited = true;
            st.engine = crate::store::load_json::<String>("mom/engine.json").unwrap_or_else(|| DEFAULT_ENGINE.to_string());
            if st.engine == crate::mom::author::BUNDLED_ENGINE { crate::mom::author::warm_bundled_engine(); }
            st.library = crate::store::load_json(LIBRARY).unwrap_or_default();
            if st.form.figure.is_none() {
                // Open on Convert only while a worksheet still has work in it (problems waiting or
                // drafts to review), not for good after one upload.
                // A draft approved but not yet in MyOpenMath is work left too (the receipts say which).
                let published = published_ids();
                let work_left = st.library.worksheet.as_ref().is_some_and(|b| !b.pending.is_empty() || !b.failures.is_empty()
                    || worksheet_drafts(&st.library, &b.id).iter().any(|(_, d)| !d.reviewed || (cfg!(not(target_arch = "wasm32")) && !published.contains(&d.id))));
                st.worksheet_mode |= work_left;
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
        st.ai_chosen = chosen;
        // D.A.D picks the AI itself (Tesler): the one already set up when it can run, else an AI on
        // this computer. Only when none is ready does the card stay open and ask.
        if !chosen && !st.ai_auto_checked {
            st.ai_auto_checked = true;
            match auto_ai(&cfg) {
                Some(AutoAi::Keep) => st.ai_keep = true,
                Some(AutoAi::Pick(pick)) => st.ai_pick = Some(pick),
                None => {}
            }
        }
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
            // Taken now, never saved for later: a press replayed after a job ends would replace
            // what the job just made. Import and Share open while a job runs (it keeps going);
            // Convert waits, and the writing card says so.
            {
                let requested = ctx.data_mut(|data| {
                    let id = egui::Id::new("dad_destination_request");
                    let result = data.get_temp::<Destination>(id);
                    data.remove::<Destination>(id);
                    result
                });
                // Only jobs a page swap would break hold Convert back; a quick check lets it through.
                // A worksheet job lives on Convert, so Convert always opens to it.
                let working = busy(&st) && st.job.lock().map(|j| matches!(j.what, "Writing" | "Revising" | "MyOpenMath")).unwrap_or(false);
                match requested {
                    Some(Destination::Import | Destination::Share) => st.tool_page = requested,
                    Some(Destination::Convert) if working => {
                        st.tool_page = None;
                        st.wait_note = Some("Convert is free once this is done. Press Stop to go now.".into());
                    }
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
        if std::mem::take(&mut st.open_account) {
            self.account.open();
        }
        if std::mem::take(&mut st.ai_keep) {
            let _ = crate::store::save_json(AI_PICKED, &true);
            st.ai_open = false;
        }
        JOB_LABEL.with(|l| l.set(None));
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
                    // The AI's own failure (busy, no answer) is not the checker's.
                    current.follow_up_why = Some(match &error {
                        Some(e) => format!("The rewrite didn't come back: {} Your question below is unchanged.", form::plain_error(e)),
                        None => "The rewrite didn't pass the checker, so it wasn't used. Your question below is unchanged.".into(),
                    });
                }
                return;
            }
            if error.is_some() && q.is_none() { st.failed_unseen = true; }
            st.error = error;
            if let Some(mut q) = q {
                // A question needs a name (MyOpenMath's Description); a model sometimes leaves it out.
                if q.name.trim().is_empty() {
                    q.name = st.form.short_name();
                }
                // A rewrite keeps the question's place in My Questions, unless that question is
                // already in MyOpenMath: then the rewrite is a new question, and the published
                // version stays as it is (the receipt is for that version).
                let mut saved_id = st.current.as_ref().and_then(|c| c.saved_id.clone());
                if st.published.is_none() { st.published = Some(published_ids()); }
                let new_version = saved_id.as_ref().is_some_and(|id| st.published.as_ref().is_some_and(|p| p.contains(id)));
                // A revision that is not itself published is rewritten in place and stays a revision of
                // the same original; one that is published is revised into yet another new question.
                let earlier = saved_id.as_ref().and_then(|id| st.library.questions.iter().find(|s| &s.id == id));
                let mut revised_from = earlier.and_then(|s| s.revised_from).or(st.current.as_ref().and_then(|c| c.revised_from));
                if new_version {
                    let added_at = saved_id.as_deref().and_then(added_at).or(earlier.map(|s| s.saved_at)).unwrap_or_else(timestamp);
                    revised_from = Some(Revision::after(revised_from, added_at));
                    saved_id = None;
                }
                let worksheet_source = st.current.as_ref().and_then(|c| c.worksheet_source.clone());
                let mut follow_ups = st.current.as_mut().map(|c| std::mem::take(&mut c.follow_ups)).unwrap_or_default();
                follow_ups.extend(follow_up);
                st.current = Some(Current { q, request: st.request.clone(), follow_ups, passed: Some(passed), problems, untested, saved_id, revised_from, worksheet_source, ..Default::default() });
                // In My Questions at once (as "Needs review"), so closing D.A.D before approving it
                // never loses a question the AI just wrote.
                save_current(st);
                if new_version { st.lib_note = Some("Saved as a new question. The earlier version stays in MyOpenMath as it was.".into()); }
                st.wait_note = None;
                st.ready_unseen = true;
                st.step = Step::Try;
                st.preview_html = None;
                st.note = None;
            }
        }
        Outcome::WorksheetRead { questions, mut notes, retry_files } => {
            if questions.is_empty() { st.failed_unseen = true; }
            let id = crate::model::new_id();
            let pending: Vec<_> = questions.into_iter().enumerate().map(|(index, mut source)| WorksheetProblem {
                origin: WorksheetOrigin { batch: id.clone(), number: index + 1, original: source.original.take() },
                source,
            }).collect();
            if pending.is_empty() && notes.is_empty() {
                notes.push("No problems were found in this worksheet.".into());
            }
            st.library.worksheet = Some(WorksheetBatch { id, pending, notes, failures: Vec::new() });
            // The files were kept chosen while reading (so Stop leaves them); a finished read clears them, a failed one keeps them.
            st.worksheet_files = retry_files;
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
        j.generation += 1;
        j.what = what;
        j.pause_after_current = false;
        j.log.clear();
    }
    st.error = None;
    st.note = None;
    st.wait_note = None;
    st.lib_note = None;
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
    st.write_started = ctx.input(|i| i.time);
    let preference = crate::store::load_json::<String>(WRITING_PREFERENCE).unwrap_or_default();
    let request = form::with_writing_preference(&request, &preference);
    // A new question starts without a video (one is added after writing, in the Help video card);
    // a revision keeps the one attached.
    let video_help = if setup.is_some() { None } else { st.current.as_ref().and_then(|current| question_help::attached_video(&current.q)) };
    let (engine, cfg, ctx) = (st.engine.clone(), cfg.clone(), ctx.clone());
    let generation = job.lock().map(|j| j.generation).unwrap_or(0);
    run_bg(async move {
        let (form, image) = setup.map_or((None, None), |(form, image)| (Some(form), image));
        let mut chat = ModelChat { cfg: &cfg, tools: false, reference_image: image.as_deref(), video_help };
        let mut critic = SetupCheck { form, engine: engine.clone() };
        let mut eng = HttpEngine { base: engine };
        let log_job = job.clone();
        let log_ctx = ctx.clone();
        // D.A.D AI charges one question's credits first; any other AI goes straight on.
        if let Err(e) = crate::ai::start_service(&cfg, "question", 0).await {
            if job.lock().map(|j| j.generation != generation).unwrap_or(true) { return; }
            finish(&job, &ctx, Outcome::Authored { follow_up, q: None, passed: false, problems: Vec::new(), untested: Vec::new(), error: Some(e) });
            return;
        }
        let run = author_full(&request, &mut chat, &mut eng, &mut critic, Budget::tries(MAX_TRIES), move |line| {
            if let Ok(mut j) = log_job.lock() && j.generation == generation { j.log.push(line.to_string()); }
            log_ctx.request_repaint();
        }).await;
        let untested = run.report.as_ref().map(|r| r.untested_parts.clone()).unwrap_or_default();
        let mut problems = run.problems;
        let passed = run.passed && run.critic_ok != Some(false);
        if run.critic_ok == Some(false) { problems.extend(run.critic_notes); }
        // Stopped by the teacher meanwhile: the answer is not wanted.
        if job.lock().map(|j| j.generation != generation).unwrap_or(true) { return; }
        finish(&job, &ctx, Outcome::Authored { follow_up, q: run.question, passed, problems, untested, error: run.error });
    });
}

/// Stop a write: the screen is free at once, and the answer, when it comes, is dropped.
fn stop_job(st: &mut State) {
    if let Ok(mut j) = st.job.lock() {
        j.busy = false;
        j.generation += 1;
        j.log.clear();
    }
    st.log.clear();
    st.wait_note = None;
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
    // Kept chosen: a stopped read leaves the files ready to read again.
    st.worksheet_files = files.clone();
    let generation = job.lock().map(|j| j.generation).unwrap_or(0);
    let (cfg, ctx) = (cfg.clone(), ctx.clone());
    run_bg(async move {
        let (questions, notes) = worksheet::read(&files, &cfg, |line| {
            if let Ok(mut j) = job.lock() && j.generation == generation { j.log.push(line); }
            ctx.request_repaint();
        }).await;
        if job.lock().map(|j| j.generation != generation).unwrap_or(true) { return; }
        let retry_files = if questions.is_empty() { files } else { Vec::new() };
        finish(&job, &ctx, Outcome::WorksheetRead { questions, notes, retry_files });
    });
}

fn convert_worksheet(st: &mut State, ctx: &egui::Context, cfg: &crate::ai::AiCfg, questions: Vec<WorksheetProblem>) {
    let batch_total = st.library.worksheet.as_ref().map_or(questions.len(), |b| {
        st.library.questions.iter().filter(|q| q.worksheet_origin.as_ref().is_some_and(|o| o.batch == b.id)).count() + b.pending.len() + b.failures.len()
    }).max(questions.len());
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
            // Counted in the worksheet's own numbers, as the line above it ("2 of 5 converted").
            let _ = (index, total);
            progress(format!("Converting and checking problem {} of {batch_total}…", question.origin.number));
            let request = question.source.request();
            // D.A.D AI: one question's credits per problem. When they run out, this problem says so and
            // the rest wait in the batch, as after Pause.
            let charged = crate::ai::start_service(&cfg, "question", 0).await;
            let out_of_credit = charged.is_err();
            let run = match charged {
                Ok(_) => author_async_logged(&request, &mut chat, &mut engine, MAX_TRIES, |_| {}).await,
                Err(e) => crate::mom::author::AuthorRun { error: Some(e), ..Default::default() },
            };
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
                    revised_from: None,
                })
            } else {
                let error = run.error.or_else(|| run.problems.into_iter().next())
                    .unwrap_or_else(|| "The AI did not produce a MyOpenMath question.".into());
                Err(WorksheetFailure { problem: question, error })
            };
            if let Ok(mut job) = job.lock() { job.outcomes.push(Outcome::WorksheetItem(origin, item)); }
            ctx.request_repaint();
            if out_of_credit { break; }
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
    // Drawn on its own as Try it opens, not at the teacher's press: what the last job said stays
    // (such as "Saved as a new question" after revising a published one).
    let (note, lib_note) = (st.note.take(), st.lib_note.take());
    let job = start(st, "Drawing the preview");
    (st.note, st.lib_note) = (note, lib_note);
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
    // Another try at the same description replaces a row that failed its check, so a run of
    // retries leaves one question, not a pile of attempts. A row that passed is never replaced:
    // two passing versions of one description are two questions.
    if c.saved_id.is_none() && c.worksheet_source.is_none() {
        c.saved_id = st.library.questions.iter()
            .find(|s| s.request == c.request && !s.passed && !s.reviewed && s.worksheet_origin.is_none())
            .map(|s| s.id.clone());
    }
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
        revised_from: c.revised_from,
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
    dropdown_grouped(ui, id, labels, cur, width, &[])
}

/// [`dropdown`] with quiet, unselectable headings before some entries: `heads` pairs an entry's
/// index with the heading drawn above it.
fn dropdown_grouped(ui: &mut Ui, id: &str, labels: &[&str], cur: &mut usize, width: f32, heads: &[(usize, &str)]) -> bool {
    let before = *cur;
    let shown = labels.get(*cur).copied().unwrap_or("");
    // Up to 520 tall, so a course's whole topic list (and "Other" at its end) shows without
    // scrolling where there is room. A list taller than the room below its field was moved beside
    // the field, over the page title, so it is capped to the room below (or above, when that is
    // all there is) and scrolls: it stays attached to the field it changes.
    let screen = ui.ctx().content_rect();
    let field_top = ui.cursor().min.y;
    let below = screen.max.y - (field_top + 36.0) - 16.0;
    let above = field_top - screen.min.y - 16.0;
    let room = if below >= 200.0 || below >= above { below } else { above };
    let height = room.clamp(120.0, 520.0);
    egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).height(height).show_ui(ui, |ui| {
        for (i, l) in labels.iter().enumerate() {
            if let Some((_, head)) = heads.iter().find(|(at, _)| *at == i) {
                if i > 0 { ui.add_space(4.0); }
                ui.label(theme::muted(*head).size(11.5));
            }
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
    // The wordmark is part of every page's header now (`pageshell::header`).
    pageshell::title(ui, "", heading, sub);
}

fn page(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg, chosen: bool) {
    questions_header(ui, st.worksheet_mode && st.step == Step::Describe);
    ui.add_space(12.0);
    let wide = ui.available_width() >= 900.0;
    // Narrow, My Questions sits under the form: the way down to it is a command on the step row,
    // beside New Question, not a stray label above the form.
    steps(ui, st, !wide);
    ui.add_space(12.0);
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
                if std::mem::take(&mut st.side_to_top) {
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("mom_side", "more_below_jump")), 0.0f32));
                }
                column(ui, "mom_side", |ui| side_column(ui, ctx, st, cfg, chosen));
            });
        });
    } else {
        // An AI that isn't ready blocks Write, and the form says to finish it in the Your AI card:
        // stacked, that card comes first so the fix is on screen. A ready AI stays under the form.
        let fix_first = st.step == Step::Describe && !busy(st) && !ai_ready(cfg)
            && !(!st.worksheet_mode && st.form.line_variant.as_ref().is_some_and(|line| line.enabled));
        column(ui, "mom_stack", |ui| {
            if fix_first {
                ai_card(ui, st, cfg, chosen);
                ui.add_space(12.0);
            }
            main(ui, ctx, st, cfg);
            ui.add_space(12.0);
            if fix_first { side(ui, ctx, st) } else { side_column(ui, ctx, st, cfg, chosen) }
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

/// Settings' switch for the Questions page's advanced tools (question code, test log, checker):
/// a developer setting, so it lives in Settings rather than as a second disclosure on the page.
pub fn advanced_setting(ui: &mut Ui) {
    ST.with(|cell| {
        let mut st = cell.borrow_mut();
        if !st.inited { st.advanced = crate::store::load_json(ADVANCED).unwrap_or(false); }
        ui.scope(|ui| {
            square_checks(ui);
            // The box's edge at 3:1, like every other control.
            control_edges(ui);
            if ui.checkbox(&mut st.advanced, "Show advanced tools on the Questions page (question code, test log, checker)").changed() {
                let _ = crate::store::save_json(ADVANCED, &st.advanced);
            }
        });
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
    // Scrolled down, the top edge fades into the page, so a card cut there reads as scrolled
    // past, not broken (the bottom has its More below cue).
    // A column may pin a one-line summary over that edge while scrolled (My Questions: its done
    // count and what is left), so the totals stay in view while the rows scroll under them.
    let sticky = ui.ctx().data(|d| d.get_temp::<String>(egui::Id::new((salt, "sticky"))));
    if out.state.offset.y > 2.0 {
        let r = out.inner_rect;
        let bg = ui.visuals().panel_fill;
        let p = ui.painter();
        let lines: Vec<&str> = sticky.as_deref().map(|s| s.lines().filter(|l| !l.is_empty()).collect()).unwrap_or_default();
        let band = if lines.is_empty() { 0.0 } else { 10.0 + 18.0 * lines.len() as f32 };
        if !lines.is_empty() {
            p.rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(r.width(), band)), 0.0, bg);
            // In line with the rows' content (inside the card's padding).
            for (i, line) in lines.iter().enumerate() {
                p.text(egui::pos2(r.left() + 21.0, r.top() + 14.0 + 18.0 * i as f32), egui::Align2::LEFT_CENTER, *line, egui::FontId::new(12.5, theme::semibold()), theme::text_col());
            }
        }
        for i in 0..12 {
            let a = (230.0 * (1.0 - i as f32 / 12.0)) as u8;
            let c = egui::Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), a);
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(r.left(), r.top() + band + i as f32 * 2.0), egui::vec2(r.width(), 2.0)), 0.0, c);
        }
    }
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
fn steps(ui: &mut Ui, st: &mut State, narrow: bool) {
    let has_q = st.current.is_some();
    // The last step closes out once a receipt says this question is in MyOpenMath.
    if st.published.is_none() { st.published = Some(published_ids()); }
    let in_mom = st.current.as_ref().and_then(|c| c.saved_id.as_ref()).is_some_and(|id| st.published.as_ref().is_some_and(|p| p.contains(id)));
    let reviewed = st.current.as_ref().is_some_and(|c| c.reviewed);
    ui.horizontal_wrapped(|ui| {
        for (i, (step, name, open)) in [(Step::Describe, if st.worksheet_mode { "Upload" } else { "Describe it" }, true), (Step::Try, "Try it", has_q), (Step::Copy, "Add to MyOpenMath", reviewed)].into_iter().enumerate() {
            if i > 0 { ui.label(theme::muted("›").size(18.0)); }
            // A step behind the teacher shows a tick, so the row says how far along they are.
            let done = match step { Step::Describe => has_q, Step::Try => reviewed, Step::Copy => in_mom };
            let label = if done && (st.step != step || step == Step::Copy) { format!("✔ {}   {name}", i + 1) } else { format!("{}   {name}", i + 1) };
            // During a job the pills keep their look (the page isn't frozen); they just don't move.
            let r = ui.add_enabled_ui(open, |ui| step_pill(ui, &label, st.step == step)).inner;
            if r.clicked() && !busy(st) { st.step = step; }
        }
        // Commands, not steps: they sit apart from the progress pills, at the row's far end.
        let jump = narrow && !st.library.questions.is_empty();
        // On the finish, Write Another Question is the one next step; a second "new" in the corner
        // would compete with it.
        let finish = st.step == Step::Copy && in_mom && !st.add_elsewhere;
        let has_q = has_q && !finish;
        if (has_q && !busy(st)) || jump {
            let (new, down) = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let new = (has_q && !busy(st)).then(|| {
                    let shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::N));
                    // The button meets the column edge; its shortcut sits just left of it, as Ctrl+Enter
                    // is shown under each box.
                    let pressed = crate::kit::tertiary(ui, egui_phosphor::regular::PLUS, "New Question", 36.0)
                        .on_hover_text(format!("Start a new question ({shortcut}). Your course and question options stay picked"))
                        .clicked();
                    ui.label(theme::muted(&shortcut).size(11.5));
                    pressed
                });
                let down = jump.then(|| crate::kit::tertiary(ui, egui_phosphor::regular::ARROW_DOWN, "My Questions", 36.0)
                    .on_hover_text("Go down to your saved questions").clicked());
                (new.unwrap_or(false), down.unwrap_or(false))
            }).inner;
            if new { start_new_question(st); }
            if down { st.jump_library = true; }
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
    st.ready_unseen = false;
    st.failed_unseen = false;
    let what = st.job.lock().map(|j| j.what).unwrap_or("");
    if busy(st) && matches!(what, "Writing" | "Revising") {
        leave();
        // Stopping a revision keeps the question as it was; stopping a write goes back to its description.
        if writing(ui, st) {
            stop_job(st);
            // With a question on screen (a rewrite, or "Rewrite with AI" after a failed check),
            // Stop goes back to it; a first write goes back to its description.
            st.step = if what == "Revising" || st.current.is_some() { Step::Try } else { Step::Describe };
            // A stopped change goes back in its box, ready to send again.
            if let Some(c) = st.current.as_mut() && what == "Revising" { c.follow_up_draft = std::mem::take(&mut c.follow_up_pending); }
        }
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
    advanced(ui, ctx, st, cfg);
}

/// The AI that reads worksheets: pages and photos go to it as pictures, which Claude on this
/// computer cannot take. When Claude was picked and Codex is here too, worksheets go to Codex.
fn worksheet_ai(cfg: &crate::ai::AiCfg) -> crate::ai::AiCfg {
    #[cfg(not(target_arch = "wasm32"))]
    if cfg.provider == "claude-cli" && crate::ai::cli::installed().contains(&"codex-cli") {
        let mut codex = cfg.clone();
        codex.provider = "codex-cli".into();
        codex.model = String::new();
        return codex;
    }
    cfg.clone()
}

fn worksheet_form(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    let rerouted = cfg.provider == "claude-cli";
    let cfg = &worksheet_ai(cfg);
    st.worksheet_files.extend(crate::platform::take_picked());
    let working = busy(st);
    let can_pick = st.library.worksheet.as_ref().is_none_or(|batch| batch.pending.is_empty() && batch.failures.is_empty());
    let mut save = false;
    let mut convert = None;
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, "Upload a math or science worksheet");
        ui.add(egui::Label::new(theme::muted("PDF, Word, text or photos.").size(13.0)).wrap());
        // Said before the upload, not after it fails.
        if cfg.provider == "claude-cli" {
            ui.add(egui::Label::new(RichText::new("Claude on this computer can't read PDFs or photos. Upload a Word or text file, or pick another AI under Your AI.").color(theme::red()).size(13.0)).wrap());
        }
        // How it is read (which AI, the local copy, Word with equations) is there before the upload
        // (Stage B), one level down: the card itself is the one thing to do.
        egui::CollapsingHeader::new(theme::muted("How D.A.D reads worksheets").size(12.5)).id_salt("mom_ws_how").show(ui, |ui| {
            if rerouted {
                ui.add(egui::Label::new(theme::muted("Codex on this computer reads them, because Claude can't read pages as pictures.").size(12.5)).wrap());
            }
            ui.add(egui::Label::new(theme::muted("Word worksheets with equations or diagrams read best saved as PDFs.").size(12.5)).wrap());
            ui.add(egui::Label::new(theme::muted("D.A.D keeps a copy of the pages it reads, so you can compare them with the drafts.").size(12.5)).wrap());
        });
        ui.add_space(8.0);
        if working {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::text(st.log.last().map(String::as_str).unwrap_or("Opening worksheet…")));
            });
            let mut stop_reading = false;
            if let Ok(mut job) = st.job.lock() {
                if job.what == "Converting worksheet" {
                    ui.label(theme::muted("About a minute per problem. Drafts are saved as each one finishes.").size(13.0));
                    if job.pause_after_current {
                        ui.label("Pausing after this problem…");
                    } else if pageshell::outline(ui, "Pause after this problem").clicked() {
                        job.pause_after_current = true;
                    }
                } else if job.what == "Reading worksheet" {
                    ui.label(theme::muted("About half a minute a page.").size(13.0));
                    stop_reading = crate::kit::secondary(ui, "Stop", 32.0).on_hover_text("Stop reading; the files stay chosen").clicked();
                }
            }
            if stop_reading { stop_job(st); }
            return;
        }
        let choose = can_pick && ui.horizontal_wrapped(|ui| {
            let r = pageshell::primary(ui, "Choose worksheet files");
            drop_hint(ui);
            r
        }).inner.clicked();
        if choose {
            let offered: Vec<&str> = worksheet::EXTENSIONS.iter().chain(worksheet::IPHONE_PHOTOS).copied().collect();
            if let Some(paths) = crate::platform::pick_files("Math or science worksheets", &offered) {
                for path in paths {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    if let Some(help) = worksheet::iphone_photo_help(&name) { st.error = Some(help); continue; }
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
                // Counted as done, out of the whole worksheet: "2 of 5", not "3 left".
                let made = st.library.questions.iter().filter(|q| q.worksheet_origin.as_ref().is_some_and(|o| o.batch == batch.id)).count();
                let total = made + batch.pending.len() + batch.failures.len();
                ui.label(theme::strong(format!("{made} of {total} worksheet problems converted")));
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
            let mut finished_open: Option<Saved> = None;
            if st.published.is_none() { st.published = Some(published_ids()); }
            ui.add_space(12.0);
            pageshell::card(ui, |ui| {
                pageshell::heading(ui, "Review worksheet drafts");
                let approved = drafts.iter().filter(|(_, draft)| draft.reviewed).count();
                let published = st.published.clone().unwrap_or_default();
                let added = drafts.iter().filter(|(_, d)| published.contains(&d.id)).count();
                let line = if added > 0 { format!("{approved} of {} drafts approved · {added} in MyOpenMath", drafts.len()) } else { format!("{approved} of {} drafts approved", drafts.len()) };
                ui.label(theme::muted(line));
                let mut add_next = None;
                if let Some((_, next)) = drafts.iter().find(|(_, draft)| !draft.reviewed) {
                    if pageshell::primary(ui, "Continue reviewing").clicked() { open = Some((*next).clone()); }
                } else if let Some((_, next)) = drafts.iter().find(|(_, d)| !published.contains(&d.id)) {
                    // All approved: the last stretch is adding them, one press each.
                    if pageshell::primary(ui, "Add the Next One to MyOpenMath").clicked() { add_next = Some((*next).clone()); }
                } else if added > 0 {
                    ui.label(RichText::new("✔ Every draft from this worksheet is in MyOpenMath.").color(theme::green()));
                }
                if let Some(next) = add_next { open = None; finished_open = Some(next); }
                egui::CollapsingHeader::new("All worksheet drafts").show(ui, |ui| for (number, draft) in &drafts {
                    ui.push_id(&draft.id, |ui| {
                        let title = MomQuestion::parse(&draft.src).name;
                        let response = ui.link(format!("Problem {number} · {title}"));
                        if response.clicked() { open = Some((*draft).clone()); }
                        let status = if published.contains(&draft.id) { "In MyOpenMath" } else if draft.reviewed { "Ready to add" } else { saved_status(draft) };
                        ui.label(theme::muted(status).size(13.0));
                    });
                });
            });
            // Finished with this worksheet, whatever is left: the drafts stay in My Questions, and
            // D.A.D stops opening on Convert for it.
            ui.add_space(6.0);
            if st.confirm_close_worksheet {
                let waiting = st.library.worksheet.as_ref().map_or(0, |b| b.pending.len() + b.failures.len());
                let ask = if waiting > 0 { format!("Close this worksheet? Its drafts stay in My Questions; the {waiting} problems not converted yet are dropped.") } else { "Close this worksheet? Its drafts stay in My Questions.".into() };
                ui.add(egui::Label::new(theme::muted(ask).size(13.0)).wrap());
                ui.horizontal(|ui| {
                    if crate::kit::secondary(ui, "Close It", 32.0).clicked() {
                        st.library.worksheet = None;
                        st.confirm_close_worksheet = false;
                        st.worksheet_mode = false;
                        save_worksheet(st);
                    }
                    if crate::kit::secondary(ui, "Keep It", 32.0).clicked() { st.confirm_close_worksheet = false; }
                });
            } else if crate::kit::link(ui, "I'm done with this worksheet", false).clicked() {
                st.confirm_close_worksheet = true;
            }
            if let Some(saved) = open { open_worksheet_draft(st, saved); }
            if let Some(saved) = finished_open { open_saved(st, saved); st.step = Step::Copy; }
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

/// Ids of saved questions that a receipt shows attached to a MyOpenMath assessment.
#[cfg(not(target_arch = "wasm32"))]
fn published_ids() -> std::collections::HashSet<String> {
    use crate::mom::poster::PublishState;
    crate::store::list_dir("mom/publications").into_iter().filter_map(|file| {
        let state: PublishState = crate::store::load_json(&format!("mom/publications/{file}"))?;
        matches!(state, PublishState::Attached(_)).then(|| file.split('-').next().unwrap_or_default().to_string())
    }).collect()
}

/// The web build publishes through the helper server and keeps no receipts here.
#[cfg(target_arch = "wasm32")]
fn published_ids() -> std::collections::HashSet<String> { Default::default() }

/// Where a saved question went: its receipt's course, assessment and MyOpenMath question number.
#[cfg(not(target_arch = "wasm32"))]
fn receipt_for(id: &str) -> Option<crate::mom::poster::Publication> {
    use crate::mom::poster::PublishState;
    // In two assessments: the newest receipt is the one the teacher just made.
    let dir = crate::store::store_dir().join("mom/publications");
    let mut files: Vec<String> = crate::store::list_dir("mom/publications").into_iter().filter(|file| file.starts_with(&format!("{id}-"))).collect();
    files.sort_by_key(|file| std::cmp::Reverse(std::fs::metadata(dir.join(file)).and_then(|m| m.modified()).ok()));
    files.into_iter().find_map(|file| match crate::store::load_json::<PublishState>(&format!("mom/publications/{file}"))? {
        PublishState::Attached(p) => Some(p),
        _ => None,
    })
}

/// When a saved question was added to MyOpenMath: its newest receipt's time.
#[cfg(not(target_arch = "wasm32"))]
fn added_at(id: &str) -> Option<u64> {
    let dir = crate::store::store_dir().join("mom/publications");
    crate::store::list_dir("mom/publications").into_iter().filter(|file| file.starts_with(&format!("{id}-")))
        .filter_map(|file| std::fs::metadata(dir.join(file)).and_then(|m| m.modified()).ok())
        .filter_map(|time| time.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).max()
}

#[cfg(target_arch = "wasm32")]
fn added_at(_id: &str) -> Option<u64> { None }

/// The next worksheet draft not yet in MyOpenMath, after the one on screen.
#[cfg(not(target_arch = "wasm32"))]
fn next_worksheet_draft(st: &State) -> Option<Saved> {
    let id = st.current.as_ref()?.saved_id.as_deref()?;
    let batch = st.library.questions.iter().find(|s| s.id == id)?.worksheet_origin.as_ref()?.batch.clone();
    let published = st.published.clone().unwrap_or_default();
    worksheet_drafts(&st.library, &batch).into_iter().map(|(_, d)| d)
        .find(|d| d.id != id && !published.contains(&d.id)).cloned()
}

/// After a question is in MyOpenMath: the one next action, by what the teacher is working through.
#[cfg(not(target_arch = "wasm32"))]
fn finish_actions(ui: &mut Ui, st: &mut State) {
    if let Some(next) = next_worksheet_draft(st) {
        if pageshell::primary(ui, "Next Worksheet Draft").clicked() {
            st.add_elsewhere = false;
            if next.reviewed { open_saved(st, next); st.step = Step::Copy; } else { open_worksheet_draft(st, next); }
        }
    } else if from_worksheet(st) {
        // The worksheet's last draft: back to the worksheet, where it says it is all done.
        if pageshell::primary(ui, "Back to the Worksheet").clicked() {
            st.add_elsewhere = false;
            st.worksheet_mode = true;
            st.step = Step::Describe;
        }
        ui.add_space(8.0);
        if crate::kit::link(ui, "Write Another Question", false).clicked() { start_new_question(st); }
    } else if pageshell::primary(ui, "Write Another Question").clicked() {
        start_new_question(st);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn from_worksheet(st: &State) -> bool {
    let Some(id) = st.current.as_ref().and_then(|c| c.saved_id.as_deref()) else { return false };
    st.library.questions.iter().find(|s| s.id == id).is_some_and(|s| s.worksheet_origin.is_some())
}

fn saved_status(saved: &Saved) -> &'static str {
    if saved.needs_check { "Needs checking" }
    else if !saved.passed { "Needs fixing" }
    else if saved.reviewed { "Approved" }
    else { "Needs review" }
}


/// The AIs a teacher can pick without IT, in the order the card and Settings → AI both list them:
/// (provider, plain name, what it needs, the model a pick starts on). Settings holds the model menus.
pub(crate) fn plain_ais(local: &[&str]) -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    let mut list = Vec::new();
    // D.A.D AI first wherever it is offered (signed in, or a release with the D.A.D server built
    // in): no key, no install, one sign-in.
    if dad_ai_offered() {
        let need = if crate::ai::account_ai_ready() { "No setup. Uses credits from your account." } else { "No key or install. Sign in once; it uses credits." };
        list.push(("account", "D.A.D AI", need, ""));
    }
    if local.contains(&"claude-cli") { list.push(("claude-cli", "Claude on this computer", "Uses your Claude subscription. No key needed.", "sonnet")); }
    if local.contains(&"codex-cli") { list.push(("codex-cli", "Codex on this computer", "Uses your ChatGPT subscription. No key needed.", "")); }
    list.push(("cloud", "Ollama Cloud", "Needs a free Ollama key.", ""));
    list.push(("openrouter", "OpenRouter", "Needs an OpenRouter key and a few dollars of credit.", "deepseek/deepseek-v4.1-flash"));
    // (D.A.D AI is offered only signed in or with the server built in; offered otherwise, it led
    // teachers to a server address they do not have.)
    list
}

/// D.A.D AI can be offered: signed in already, or this build knows its server (sign-in only).
pub(crate) fn dad_ai_offered() -> bool {
    crate::ai::account_ai_ready() || crate::share::ShareSettings::built_in().is_some()
}

/// The AI can write now: a key where it needs one, or a signed-in account for D.A.D AI.
fn ai_ready(cfg: &crate::ai::AiCfg) -> bool {
    if cfg.provider == "account" { crate::ai::account_ai_ready() } else { cfg.has_key_or_needs_none() }
}

/// The AI's plain name, without the model id (the model lives in Settings).
fn ai_name(cfg: &crate::ai::AiCfg) -> String {
    match cfg.provider.as_str() {
        "claude-cli" => "Claude on this computer".into(),
        "codex-cli" => "Codex on this computer".into(),
        "openrouter" => "OpenRouter".into(),
        "ollama" => "Ollama on this computer".into(),
        "cloud" => "Ollama Cloud".into(),
        "account" => "D.A.D AI".into(),
        "workers" => "D.A.D relay".into(),
        _ => "Your own AI server".into(),
    }
}

/// "Your AI": which AI writes the questions. One line when it works; open, it lists the same AIs as
/// Settings → AI, and an AI with no key shows its key field (the one way forward).
fn ai_card(ui: &mut Ui, st: &mut State, cfg: &crate::ai::AiCfg, chosen: bool) {
    #[cfg(not(target_arch = "wasm32"))]
    let local = crate::ai::cli::installed();
    #[cfg(target_arch = "wasm32")]
    let local: Vec<&'static str> = Vec::new();
    // A saved key is tried once: a key the AI turns down is taken back off, so "ready" is not a
    // promise the first question breaks.
    if let Some((provider, key, result)) = st.key_check.as_ref().and_then(|(p, k, c)| c.lock().ok().and_then(|mut g| g.take()).map(|r| (p.clone(), k.clone(), r))) {
        st.key_check = None;
        let name = plain_ais(&local).into_iter().find(|a| a.0 == provider).map_or(provider.clone(), |a| a.1.to_string());
        let (msg, bad) = match result {
            Ok(()) => (format!("✔ {name} accepted the key."), false),
            Err(e) if e.starts_with("Key rejected") => {
                let _ = crate::ai::set_key(&provider, None);
                // Given back, masked (a classroom screen may be projected), with "Show key" beside it:
                // the teacher can check what was cut off instead of finding the key again.
                if provider == cfg.provider && st.ai_card_key.is_empty() { st.ai_card_key = key; st.ai_key_shown = false; }
                (format!("{name} turned that key down, so D.A.D took it off. Check it below (often a letter is missing at the end), or make a new one."), true)
            }
            Err(e) => (format!("Key saved. D.A.D couldn't check it just now ({e})"), false),
        };
        st.key_check_msg = Some((provider, msg, bad));
    }
    // A result belongs to the AI it was for: switching AI clears it.
    if st.key_check_msg.as_ref().is_some_and(|(p, _, _)| *p != cfg.provider) { st.key_check_msg = None; }
    let ready = ai_ready(cfg);
    let open = st.ai_open || !chosen || !ready;
    let choosing = !ready && !chosen;
    pageshell::card(ui, |ui| {
        control_edges(ui);
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::strong("Your AI").size(15.0));
            if choosing {
                ui.label(theme::muted("not chosen yet").size(13.0));
                return;
            }
            // Readiness and a plain name: a model id is one more thing to ignore here.
            ui.label(theme::text(ai_name(cfg)).size(14.0));
            let change = chosen && ready && !open;
            if !ready {
                // A setup step, not an error: the warning colour, as Write's gate line uses.
                // Once a key is typed in, the card no longer says it needs one: it isn't saved yet.
                let need = if cfg.provider == "account" { "needs you to sign in" } else if !st.ai_card_key.trim().is_empty() { "key not saved yet" } else { "needs a key" };
                ui.label(RichText::new(need).color(theme::on_attention()).size(13.0));
            }
            // A real button at the card's edge, not a small link: it changes every question written.
            if change && ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| crate::kit::secondary(ui, "Change", 32.0)).inner.clicked() {
                st.ai_open = true;
            }
        });
        // A key turned down that also looks like another site's key is said once, under the key
        // field, with that cause first; the top line would only repeat it in other words.
        let shape_problem = (crate::ai::clean_key(&st.ai_card_key).len() >= 8)
            .then(|| crate::ai::key_shape_problem(&cfg.provider, &st.ai_card_key)).flatten();
        let turned_down = st.key_check_msg.as_ref().is_some_and(|(_, _, bad)| *bad);
        if st.key_check.as_ref().is_some_and(|(p, _, _)| *p == cfg.provider) {
            ui.label(theme::muted("Checking the key…").size(12.0));
        } else if let Some((_, msg, bad)) = &st.key_check_msg
            && !(*bad && shape_problem.is_some())
        {
            let color = if *bad { theme::red() } else { theme::green() };
            ui.add(egui::Label::new(RichText::new(msg).color(color).size(12.0)).wrap());
        }
        // On Convert, which AI reads the worksheet, said where the AI is named (Claude can't read
        // pages as pictures, so Codex reads them when it is installed).
        if st.worksheet_mode && st.step == Step::Describe && cfg.provider == "claude-cli" && ready {
            let line = if local.contains(&"codex-cli") { "Worksheets are read by Codex on this computer." } else { "Claude can't read PDFs or photos: upload Word or text, or change AI." };
            ui.add(egui::Label::new(theme::muted(line).size(12.5)).wrap());
        }
        if !open { return; }
        ui.add_space(4.0);
        // Nothing ready and nothing chosen yet (a new computer): the teacher picks how D.A.D writes,
        // from the same plain list as Settings, instead of meeting a key form for an AI the app
        // chose for them. Picking one shows that AI's own setup (its key, or a sign-in) here.
        if choosing {
            ui.add(egui::Label::new(theme::text("Pick how D.A.D writes your questions. You can change it later.").size(13.0)).wrap());
            ui.add_space(8.0);
            // One recommendation, filled: D.A.D AI where it is offered (no key), else Ollama Cloud
            // (a free key). The others stay as quieter choices.
            let all = plain_ais(&local);
            let recommended = all.iter().position(|a| a.0 == "account").or_else(|| all.iter().position(|a| a.0 == "cloud"));
            for (i, (provider, name, need, model)) in all.into_iter().enumerate() {
                let pick = if Some(i) == recommended {
                    let r = pageshell::primary(ui, name);
                    ui.add(egui::Label::new(theme::muted(format!("Recommended. {need}")).size(12.0)).wrap());
                    r
                } else {
                    let r = crate::kit::secondary(ui, name, 36.0);
                    ui.add(egui::Label::new(theme::muted(need).size(12.0)).wrap());
                    r
                };
                if pick.clicked() { st.ai_pick = Some(AiPick { provider, model: model.into(), key: None }); }
                ui.add_space(8.0);
            }
            return;
        }
        if !ready && cfg.provider == "account" {
            ui.add(egui::Label::new(theme::muted("D.A.D AI needs no key: sign in once and it uses your account's credits.").size(12.0)).wrap());
            if crate::kit::secondary(ui, "Sign In to D.A.D", 36.0).clicked() { st.open_account = true; }
            ui.add_space(8.0);
        } else if !ready && cfg.uses_key() {
            // The fix for the AI in use, right here: its key, not a trip to Settings.
            let rejected = st.key_check_msg.as_ref().is_some_and(|(_, _, bad)| *bad) && !st.ai_card_key.is_empty();
            // First where to get a key (a teacher can't paste one they don't have yet), then the field.
            // The page where the key is made is one press away; an OpenRouter key already pasted
            // here needs no help getting an Ollama key (the switch is the way).
            let key_page = match cfg.provider.as_str() {
                "cloud" => Some(("Get a free key on ollama.com", "https://ollama.com/settings/keys")),
                "openrouter" => Some(("Get a key on openrouter.ai", "https://openrouter.ai/settings/keys")),
                _ => None,
            };
            let foreign = cfg.provider != "openrouter" && crate::ai::key_owner(&crate::ai::clean_key(&st.ai_card_key)) == Some("OpenRouter");
            if let Some((label, url)) = key_page && !foreign {
                ui.add_space(6.0);
                if crate::kit::link(ui, label, true).clicked() { crate::platform::open_url(url); }
            }
            // The label row carries Show key, right beside the field it reveals.
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(theme::strong(if rejected { "Check the key" } else if cfg.provider == "cloud" { "Paste your Ollama key here" } else if cfg.provider == "openrouter" { "Paste your OpenRouter key here" } else { "Paste its key" }).size(14.0));
                if !st.ai_card_key.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::kit::link(ui, if st.ai_key_shown { "Hide key" } else { "Show key" }, false).clicked() {
                            st.ai_key_shown = !st.ai_key_shown;
                        }
                    });
                }
            });
            ui.add_space(2.0);
            let need_line = plain_ais(&local).into_iter().find(|a| a.0 == cfg.provider).map(|a| a.2);
            // Stacked: the side column is narrow, and a key row side by side ran past its edge.
            // A fixed id: Show key appears above the field once a key is in, and an id taken from the
            // field's position would change then, dropping focus (and with it Enter) mid-paste.
            let r = ui.add(egui::TextEdit::singleline(&mut st.ai_card_key).id_salt("mom_ai_card_key").password(!st.ai_key_shown).margin(egui::Margin::symmetric(10, 8))
                .desired_width(ui.available_width()).hint_text("paste a key"));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "AI key"));
            // What the key is, under the field while it is empty (the label sits right on the field
            // it names; once a key is in, what matters is whether it fits).
            if let Some(need) = need_line && st.ai_card_key.is_empty() && key_page.is_none() {
                ui.add(egui::Label::new(theme::muted(need).size(12.0)).wrap());
            }
            // Whatever came with the paste (a "Key:" label, quotes, a line break) is dropped on save;
            // a key from another site is named before it is saved, not when writing fails.
            // Named once there is enough to judge, not after the first character.
            let switch_to_openrouter = cfg.provider != "openrouter"
                && crate::ai::key_owner(&crate::ai::clean_key(&st.ai_card_key)) == Some("OpenRouter");
            if let Some(problem) = &shape_problem {
                let name = ai_name(cfg);
                // With the switch on offer, one line naming the cause; the button carries the fix.
                let text = if switch_to_openrouter { format!("This is an OpenRouter key, not an {name} key.").replace("an Ollama Cloud", "an Ollama") }
                    else if turned_down { format!("{name} turned this key down. {problem}") } else { problem.clone() };
                // Understood and fixable (the switch below) is a warning, not an error: the error red
                // is kept for a key that was turned down with no way forward.
                let color = if switch_to_openrouter { theme::on_attention() } else { theme::red() };
                ui.add(egui::Label::new(RichText::new(text).color(color).size(12.0)).wrap());
                // An OpenRouter key pasted here can go where it works in one press: switch the AI
                // and keep the key, instead of a sentence that sends the teacher to find OpenRouter.
                let key = crate::ai::clean_key(&st.ai_card_key);
                // The switch is then the next step, so it is the filled button and Save Key steps back.
                // Enter in the key box presses the one action on offer, as Enter does in a form.
                let enter_here = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let switch_pressed = switch_to_openrouter && {
                    // The shortcut beside the action it presses.
                    // On its own line: beside the button it ran past the narrow column.
                    let pressed = pageshell::primary(ui, "Use This Key With OpenRouter").clicked();
                    ui.label(theme::muted("or press Enter in the key box").size(12.0));
                    pressed || enter_here
                };
                if switch_to_openrouter {
                    ui.add(egui::Label::new(theme::muted("OpenRouter needs a few dollars of credit on the account. Meant to use Ollama Cloud? Paste an Ollama key instead.").size(12.0)).wrap());
                }
                if switch_pressed
                {
                    let model = plain_ais(&local).into_iter().find(|a| a.0 == "openrouter").map_or("", |a| a.3);
                    st.ai_pick = Some(AiPick { provider: "openrouter", model: model.into(), key: Some(key) });
                    st.ai_card_key.clear();
                    // Said where the key was pasted, so the press (or Enter) visibly did something.
                    st.key_check_msg = Some(("openrouter".into(), "✔ Now using OpenRouter with the key you pasted (its label and spaces removed).".into(), false));
                }
            }
            // After the cause (which sits right under the field): showing the key, and what cleaning did
            // (a label, quotes or spaces pasted with it are dropped on save, not stored as the key).
            let cleaned = crate::ai::clean_key(&st.ai_card_key);
            if !cleaned.is_empty() && cleaned != st.ai_card_key.trim() {
                ui.add(egui::Label::new(theme::muted("D.A.D drops the label and spaces that came with the key when it saves.").size(12.0)).wrap());
            }
            let ok = !crate::ai::clean_key(&st.ai_card_key).is_empty();
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            // With a key in the box, saving it is the next step (Write waits on it), so Save Key is
            // the page's one filled button; empty, it is the quiet disabled one.
            // A key that belongs to OpenRouter isn't saved here: the switch above is its one action.
            let save = if switch_to_openrouter { None } else if ok { Some(pageshell::primary(ui, "Save Key")) } else { Some(ui.add_enabled_ui(false, |ui| crate::kit::secondary(ui, "Save Key", 36.0)).inner) };
            let enter = enter && !switch_to_openrouter;
            if (save.is_some_and(|s| s.clicked()) || enter) && ok {
                match crate::ai::set_key(&cfg.provider, Some(&st.ai_card_key)) {
                    Ok(()) => {
                        let sent = crate::ai::clean_key(&st.ai_card_key);
                        st.ai_card_key.clear();
                        st.ai_keep = true;
                        st.key_check_msg = None;
                        let check: Arc<Mutex<Option<Result<(), String>>>> = Default::default();
                        st.key_check = Some((cfg.provider.clone(), sent, check.clone()));
                        let (cfg2, ctx2) = (cfg.clone(), ui.ctx().clone());
                        #[cfg(not(target_arch = "wasm32"))]
                        std::thread::spawn(move || {
                            let r = crate::ai::test_connection(&cfg2);
                            if let Ok(mut g) = check.lock() { *g = Some(r); }
                            ctx2.request_repaint();
                        });
                        #[cfg(target_arch = "wasm32")]
                        wasm_bindgen_futures::spawn_local(async move {
                            let r = crate::ai::test_connection_async(&cfg2).await;
                            if let Ok(mut g) = check.lock() { *g = Some(r); }
                            ctx2.request_repaint();
                        });
                    }
                    Err(error) => st.error = Some(format!("Could not save the key: {error}")),
                }
            }
            if !switch_to_openrouter && crate::platform::help_clip_name(&cfg.provider).is_some() && crate::kit::link(ui, "Watch how to get a key", false).clicked() {
                crate::platform::open_help_video(&cfg.provider);
            }
            ui.add_space(8.0);
            // Where D.A.D AI needs only a sign-in, it is the way with no key at all.
            if dad_ai_offered() {
                if crate::kit::link(ui, "No key? Use D.A.D AI instead", true).clicked() {
                    st.ai_pick = Some(AiPick { provider: "account", model: String::new(), key: None });
                }
                ui.add(egui::Label::new(theme::muted("Sign in once; it uses your account's credits.").size(12.0)).wrap());
                ui.add_space(8.0);
            }
        }
        // One list, the same as Settings → AI: the AI in use first, then the others. While the AI in
        // use is being set up, they wait one level down so the visible path is one.
        let mut others_ui = |ui: &mut Ui| {
            let others: Vec<_> = plain_ais(&local).into_iter().filter(|a| a.0 != cfg.provider).collect();
            // While this AI isn't set up, each other AI is one press, with what it needs: the same
            // list as the first-open choice, not a menu plus a second confirm.
            if !ready {
                for (provider, name, need, model) in others {
                    if crate::kit::secondary(ui, name, 36.0).clicked() {
                        st.ai_pick = Some(AiPick { provider, model: model.into(), key: None });
                        st.ai_choice = 0;
                    }
                    ui.add(egui::Label::new(theme::muted(need).size(12.0)).wrap());
                    ui.add_space(6.0);
                }
                return;
            }
            if others.is_empty() {
                if ready && chosen && crate::kit::secondary(ui, "Done", 36.0).clicked() { st.ai_open = false; }
            } else {
                let mut labels = vec![format!("{} (current)", ai_name(cfg))];
                labels.extend(others.iter().map(|a| a.1.to_string()));
                // With no key for the AI in use, keeping it is no choice: start on the first other AI.
                let mut k = if !ready && st.ai_choice == 0 { 1 } else { st.ai_choice }.min(labels.len() - 1);
                field_label(ui, if ready { "Switch to" } else { "Or use another AI" });
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                if dropdown(ui, "mom_ai_choice", &refs, &mut k, ui.available_width().min(380.0)) { st.ai_choice = k; }
                if k > 0 {
                    ui.add(egui::Label::new(theme::muted(others[k - 1].2).size(12.0)).wrap());
                }
                ui.add_space(6.0);
                // "(current)" picked: the button closes the card; another AI: it switches (an AI that
                // needs a key then shows its key field above).
                let label = if k == 0 { "Done" } else { "Use This AI" };
                if ui.add_enabled_ui(k > 0 || ready, |ui| crate::kit::secondary(ui, label, 36.0)).inner.clicked() {
                    if k == 0 {
                        st.ai_keep = true;
                        st.ai_open = false;
                    } else {
                        let (provider, _, _, model) = others[k - 1];
                        st.ai_pick = Some(AiPick { provider, model: model.into(), key: None });
                        st.ai_choice = 0;
                    }
                }
            }
        };
        if ready {
            others_ui(ui);
        } else {
            egui::CollapsingHeader::new(theme::muted("Use a different AI instead").size(13.0)).id_salt("mom_ai_others").show(ui, |ui| others_ui(ui));
        }
        // Settings is the next stop once this AI works; while it is being set up, the fix is here.
        if ready {
            ui.add_space(8.0);
            ui.label(theme::muted("Models and other AIs: Settings, then AI.").size(12.0));
        }
    });
}

/// What the first open of the Questions screen does about the AI, before anyone is asked.
enum AutoAi { Keep, Pick(AiPick) }

/// The AI set up already when it can run (has its key, or needs none); else a signed-in D.A.D AI;
/// else Claude, then Codex, on this computer; else D.A.D AI when the build has its server built in.
/// None: nothing is ready, so the card asks.
fn auto_ai(cfg: &crate::ai::AiCfg) -> Option<AutoAi> {
    if ai_ready(cfg) { return Some(AutoAi::Keep); }
    if crate::ai::account_ai_ready() {
        return Some(AutoAi::Pick(AiPick { provider: "account", model: String::new(), key: None }));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let local = crate::ai::cli::installed();
        if local.contains(&"claude-cli") {
            return Some(AutoAi::Pick(AiPick { provider: "claude-cli", model: "sonnet".into(), key: None }));
        }
        if local.contains(&"codex-cli") {
            return Some(AutoAi::Pick(AiPick { provider: "codex-cli", model: String::new(), key: None }));
        }
    }
    // A build with its school server built in: D.A.D AI needs only a sign-in, which beats asking a
    // teacher to go and buy a key. The card then shows one action, Sign In to D.A.D.
    if crate::share::ShareSettings::built_in().is_some() {
        return Some(AutoAi::Pick(AiPick { provider: "account", model: String::new(), key: None }));
    }
    None
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

/// The answer-type menu: "Let the AI choose", the common types, and the rest one level down.
/// MyOpenMath's type codes show only with the advanced tools on.
fn answer_type_picker(ui: &mut Ui, part: usize, answer: &mut Option<Answer>, codes: bool) {
    let label = move |answer: Answer| {
        let (code, name, _) = answer.details();
        if codes { return format!("{name} ({code})"); }
        // MyOpenMath's own menu name beside the plain one where they differ ("Fill in the blank"
        // is "String" there), so the word matches what the teacher sees in MyOpenMath's editor.
        let flat = |t: &str| t.to_ascii_lowercase().chars().filter(char::is_ascii_alphanumeric).collect::<String>();
        let mom = form::type_menu_label(code);
        let (a, b) = (flat(name), flat(mom));
        if mom.is_empty() || answer == Answer::Dropdown || a.contains(&b) || b.contains(&a) { name.to_string() }
        else { format!("{name} (MyOpenMath: {mom})") }
    };
    let selected = answer.map_or_else(|| AI_CHOOSES.to_string(), label);
    egui::ComboBox::from_id_salt(("mom_answer_type", part))
        .selected_text(selected)
        .width(ui.available_width().min(300.0))
        .height(470.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(answer, None, AI_CHOOSES)
                .on_hover_text("Choose an answer format that suits the question.");
            for option in Answer::COMMON {
                ui.selectable_value(answer, Some(option), label(option)).on_hover_text(option.details().2);
            }
            ui.separator();
            egui::CollapsingHeader::new(theme::muted("More question types").size(13.0)).id_salt(("mom_answer_more", part)).show(ui, |ui| {
                for option in Answer::ALL.into_iter().filter(|a| !Answer::COMMON.contains(a)) {
                    ui.selectable_value(answer, Some(option), label(option)).on_hover_text(option.details().2);
                }
            });
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
        // Enter stays a new line (it is a paragraph box); Ctrl+Enter (⌘Enter on a Mac) sends it, as
        // in chat and mail apps, and says so under the box, where the hands already are.
        let send_key = description.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
        let shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.label(theme::muted(format!("{shortcut} writes it")).size(11.5));
        });
        // Question code pasted here would go to the AI as a description; Import is where it belongs.
        if matches!(not_a_question(&f.topic_text), NotAQuestion::Link(_)) {
            ui.add(egui::Label::new(theme::muted("That's a link: the AI can't open web pages. Describe the question in words, or bring a MyOpenMath question in with Import.").size(12.0)).wrap());
        }
        let code_here = f.topic_text.trim().len() > 12 && looks_like_question_code(&f.topic_text);
        if code_here && crate::kit::link(ui, "This looks like MyOpenMath code. Bring it in with Import instead", true).clicked() {
            let code = std::mem::take(&mut f.topic_text);
            st.import_source = code;
            st.import_hint = None;
            st.tool_page = Some(Destination::Import);
            request_destination(ui.ctx(), Destination::Import);
            return;
        }
        let f = &mut st.form;

        // Course and Topic side by side: two choices that narrow the same thing.
        let pair_w = ((ui.available_width() - 16.0) / 2.0).clamp(140.0, 320.0);
        ui.horizontal_wrapped(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                // A level, not the MyOpenMath course picked in step 3.
                field_label(ui, "Course level");
                let names: Vec<&str> = COURSES.iter().map(|c| c.0).collect();
                // Two pathways in one list: each under its own heading, so a teacher scans only theirs.
                // (Indexes into the full list, after "Let the AI choose".)
                let integrated = names.iter().position(|n| n.starts_with("Integrated")).map(|i| i + 1);
                let traditional = names.iter().position(|n| !n.starts_with("Integrated")).map(|i| i + 1);
                let heads: Vec<(usize, &str)> = integrated.map(|i| (i, "Integrated Math"))
                    .into_iter().chain(traditional.map(|i| (i, "Traditional courses"))).collect();
                let mut all = vec![AI_CHOOSES];
                all.extend_from_slice(&names);
                let mut k = f.course.map(|i| i + 1).unwrap_or(0);
                if dropdown_grouped(ui, "mom_course", &all, &mut k, pair_w, &heads) {
                    f.course = k.checked_sub(1);
                    f.topic = Topic::Ai;
                    f.fix();
                }
            });
            // Without a course the Topic menu has nothing to list: it appears once a course is picked.
            if f.course.is_none() { return; }
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
        let worksheet_job = st.job.lock().map(|j| j.busy && matches!(j.what, "Reading worksheet" | "Converting worksheet")).unwrap_or(false);
        let missing = if worksheet_job {
            // Write waits for a worksheet job; say which, and where it is.
            Some("D.A.D is working on your worksheet. Write is free when it finishes; see Convert.")
        } else if !ai_ready(cfg) {
            // Nothing chosen yet, there is nothing to finish: the card asks for a pick.
            Some(if !st.ai_chosen { "Pick an AI in the Your AI card first." }
                // The card already offers the way forward for an OpenRouter key: point at it.
                else if cfg.provider != "openrouter" && crate::ai::key_owner(&crate::ai::clean_key(&st.ai_card_key)) == Some("OpenRouter") {
                    "Use your pasted key with OpenRouter in the Your AI card to start writing."
                }
                else if cfg.provider == "account" { "Sign in to D.A.D in the Your AI card first." }
                else if cfg.provider == "cloud" { "Paste your Ollama Cloud key in the Your AI card first." }
                else if cfg.provider == "openrouter" { "Paste your OpenRouter key in the Your AI card first." }
                else { "Your AI isn't ready yet: finish it in the Your AI card." })
        } else if st.topic_image.is_some() {
            picture_blocked.then_some("Choose an AI that can read uploaded pictures, or describe the topic in words.")
        } else {
            st.form.missing()
        };
        // Write, then the optional picture beside it, quieter: attach sits next to send.
        let write = ui.horizontal_wrapped(|ui| {
            let shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter));
            let now = std::mem::take(&mut st.write_now);
            let write = (pageshell::primary_on(ui, "Write My Question", missing.is_none() && !working)
                .on_hover_text(format!("Or press {shortcut} in the description")).clicked() || send_key || now)
                && missing.is_none() && !working;
            ui.add_space(12.0);
            // Why it can't be used is on the button itself (hover), not a sentence in the main line
            // of sight on every visit.
            if ui.add_enabled_ui(!working && !picture_blocked, |ui| crate::kit::tertiary(ui, egui_phosphor::regular::IMAGE, "Add a reference picture", 36.0)).inner
                .on_disabled_hover_text("Claude on this computer can't read pictures. To use a picture, change Your AI to one that can.")
                .clicked() {
                match crate::platform::pick_question_image() {
                    Ok(file) => picked = file,
                    Err(error) => st.topic_image_error = Some(error),
                }
            }
            // A blocked action says why in one short line (the hover has the full sentence).
            if picture_blocked && st.topic_image.is_none() {
                ui.label(theme::muted("This AI can't read pictures").size(12.0));
            }
            if st.topic_image.is_none() && !picture_blocked && !working && ai_ready(cfg) {
                // As Convert and Import say: a photo can be dropped here too.
                ui.label(theme::muted("or drop a photo on this window").size(12.0));
            }
            if let Some(image) = &st.topic_image {
                ui.label(theme::muted(&image.name));
                if ui.link("Remove picture").clicked() {
                    st.topic_image = None;
                    st.topic_image_error = None;
                }
            }
            write
        }).inner;
        if let Some(help) = picked.as_ref().and_then(|image| worksheet::iphone_photo_help(&image.name)) {
            st.topic_image_error = Some(help);
        } else if let Some(image) = picked {
            match worksheet::image_as_png(&image.bytes) {
                Ok(bytes) => {
                    st.topic_image = Some(PickedFile { name: image.name, bytes });
                    st.topic_image_error = None;
                }
                Err(error) => st.topic_image_error = Some(error),
            }
        }
        if let Some(error) = &st.topic_image_error { ui.add(egui::Label::new(error).wrap()); }
        if st.topic_image.is_none() { st.topic_image_note = None; }
        if let Some(note) = &st.topic_image_note { ui.add(egui::Label::new(theme::muted(note).size(12.0)).wrap()); }
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
        // A busy AI is a wait, not a failure: said calmly, with the retry and the way round it.
        if st.error.as_deref().is_some_and(|e| form::plain_error(e).starts_with("The AI is busy")) {
            let raw = st.error.clone().unwrap_or_default();
            ui.add_space(8.0);
            // Write My Question above is the retry (one button for it, not two), named here; the way
            // round a busy AI is the other choice, as a button.
            crate::kit::gated_reason(ui, "The AI is busy right now. Wait a minute and press Write My Question again, or use a different AI.");
            if crate::kit::secondary(ui, "Use a Different AI", 32.0).clicked() {
                st.error = None;
                st.ai_open = true;
            }
            egui::CollapsingHeader::new(theme::muted("Show details").size(12.0)).id_salt("mom_busy_details").show(ui, |ui| {
                ui.add(egui::Label::new(theme::muted(&raw).size(12.0)).wrap());
            });
        } else {
            error_box(ui, st);
        }

        ui.add_space(12.0);
        let set = more_options_set(&st.form);
        let more = if set == 0 { "More options".to_string() } else { format!("More options ({set} set)") };
        // A 32 px row to press, not just its text.
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 32.0;
            egui::CollapsingHeader::new(theme::strong(more).size(14.0)).id_salt("mom_more_options").show(ui, |ui| {
                more_options(ui, st);
            });
        });
    });
}

/// How many of the "More options" differ from their defaults, so a closed section still says it holds choices.
fn more_options_set(f: &Form) -> usize {
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
    // Hover reads as "this one", by its edge (the theme keeps the card fill under the pointer).
    w.hovered.bg_stroke = egui::Stroke::new(1.0, theme::accent_col());
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
    let advanced = st.advanced;
    let f = &mut st.form;
    part_count_picker(ui, f);
    let per_part = f.part_range.map_or(f.part_count, |[_, max]| Some(max)).is_some_and(|n| n > 1);
    field_label(ui, if per_part { "Question type for each part" } else { "Question type" });
    for i in 0..f.answers.len() {
        ui.horizontal_wrapped(|ui| {
            if per_part {
                let optional = if f.part_range.is_some_and(|[min, _]| i >= min) { " (optional)" } else { "" };
                ui.label(theme::text(format!("Part ({}){optional}", (b'a' + i as u8) as char)));
            }
            answer_type_picker(ui, i, &mut f.answers[i], advanced);
        });
        if let Some(answer) = f.answers[i] {
            ui.add(egui::Label::new(theme::muted(answer.details().2).size(13.0)).wrap());
        }
    }
    if f.answers.iter().any(Option::is_some) {
        ui.checkbox(&mut f.prefer_answer_types, "Let AI choose a different type if it fits better");
    }
    if advanced { egui::CollapsingHeader::new("Compare MyOpenMath question types")
        .id_salt("mom_answer_types_help")
        .show(ui, |ui| {
            for answer in Answer::ALL {
                let (code, name, description) = answer.details();
                ui.label(theme::strong(format!("{name} ({code})")));
                ui.add(egui::Label::new(description).wrap());
                ui.add_space(4.0);
            }
            ui.hyperlink_to("MyOpenMath question-type documentation", "https://dev.imathas.com/help.html#questiontypes");
        }); }

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
    // A help video is added in one place: the Help video card once the question is written.

    let f = &mut st.form;
    field_label(ui, "Anything else? (optional)");
    let r = ui.add(egui::TextEdit::multiline(&mut f.extra).desired_rows(2).desired_width(f32::INFINITY)
        .hint_text("For example: use money amounts, or ask for the y-intercept too"));
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Anything else (optional)"));
    // The saved writing preference shapes every question written: it sits with the options it
    // joins, where it can be read before writing, not only in the revise card afterwards.
    ui.add_space(6.0);
    let rule = if st.writing_preference.trim().is_empty() { "Save a rule for every question" } else { "Your saved rule for every question" };
    egui::CollapsingHeader::new(rule).id_salt("mom_writing_preference").show(ui, |ui| {
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
    // Dropdowns, like every other count on this form: a drag-number gave no sign it could be typed.
    if let Some([min, max]) = &mut form.part_range {
        ui.horizontal(|ui| {
            let (lo, hi) = (*min, *max);
            count_dropdown(ui, "mom_parts_min", "Minimum", min, 1..=hi);
            ui.add_space(12.0);
            count_dropdown(ui, "mom_parts_max", "Maximum", max, lo..=MAX_PARTS);
        });
    } else if let Some(count) = &mut form.part_count {
        ui.horizontal(|ui| count_dropdown(ui, "mom_parts", "Parts", count, 1..=MAX_PARTS));
    }
    form.fix();
}

fn count_dropdown(ui: &mut Ui, id: &str, name: &str, value: &mut usize, range: std::ops::RangeInclusive<usize>) {
    let label = ui.label(name);
    let words = |n: usize| if n == 1 { "1 part".to_string() } else { format!("{n} parts") };
    egui::ComboBox::from_id_salt(id).selected_text(words(*value)).width(110.0).show_ui(ui, |ui| {
        for n in range { ui.selectable_value(value, n, words(n)); }
    }).response.labelled_by(label.id);
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

/// How far a write has got, from its log: 0 writing, 1 testing (or fixing), 2 passed.
fn write_stage(st: &State) -> usize {
    let said: Vec<String> = st.log.iter().filter_map(|line| form::plain_progress(line, MAX_TRIES)).collect();
    if said.iter().any(|p| p.starts_with("It works")) { 2 }
    else if said.iter().any(|p| p.starts_with("Testing") || p.starts_with("Found a problem")) { 1 }
    else { 0 }
}

/// True when the teacher pressed Stop.
fn writing(ui: &mut Ui, st: &State) -> bool {
    let mut stop = false;
    let revising = st.job.lock().map(|j| j.what == "Revising").unwrap_or(false);
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, if revising { "Rewriting your question" } else { "Writing your question" });
        // What was asked stays on screen while the AI works, so nobody has to remember it (the form
        // is put away during a write).
        let asked = if revising { st.current.as_ref().map(|c| c.follow_up_pending.trim().to_string()).unwrap_or_default() } else { st.form.topic_text.trim().to_string() };
        if !asked.is_empty() {
            let short: String = asked.chars().take(160).collect();
            let quote = if short.len() < asked.len() { format!("“{}…”", short.trim_end()) } else { format!("“{short}”") };
            ui.add(egui::Label::new(theme::muted(if revising { format!("Change: {quote}") } else { quote }).size(13.0)).wrap());
        }
        ui.add_space(6.0);
        let mut said: Vec<String> = Vec::new();
        for line in &st.log {
            if let Some(p) = form::plain_progress(line, MAX_TRIES) {
                if said.last() != Some(&p) { said.push(p); }
            }
        }
        // Every stage from the start, the done ones ticked and the next ones still to come, so the
        // teacher sees how far along it is and what is left, not one line that changes.
        let stage = write_stage(st);
        let names = [if revising { "Rewrite it" } else { "Write it" }, "Test it with 3 sets of random numbers", "Ready to try"];
        // A bar for how much is done (not "Step 1 of 3", which the page's own steps already say).
        // Within a stage the bar creeps on with time (never past 85 % of the stage), so a long first
        // stage doesn't sit at one dot looking stalled; the words stay exact.
        let elapsed = (ui.input(|i| i.time) - st.write_started).max(0.0) as f32;
        let within = if stage < names.len() { (1.0 - (-elapsed / 12.0).exp()).min(0.85) } else { 0.0 };
        ui.add(egui::ProgressBar::new((stage as f32 + within) / names.len() as f32).desired_width(320.0).desired_height(8.0)
            .fill(theme::accent_col()).corner_radius(3.0));
        // Which stage, and the time so far, right by the bar (the bar's fill is the stage under way).
        let el = elapsed as u64;
        ui.label(theme::muted(format!("Stage {} of {} · {}:{:02} so far", (stage + 1).min(names.len()), names.len(), el / 60, el % 60)).size(12.5));
        for (i, name) in names.iter().enumerate() {
            ui.horizontal(|ui| {
                // One 18 px slot for the mark, so the three names share a left edge.
                let (slot, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                if i < stage {
                    ui.painter().text(slot.center(), egui::Align2::CENTER_CENTER, "✔", egui::FontId::proportional(14.0), theme::green());
                    ui.label(theme::muted(*name).size(14.0));
                } else if i == stage {
                    egui::Spinner::new().size(18.0).paint_at(ui, slot.expand(1.0));
                    ui.label(theme::strong(*name).size(15.0));
                } else {
                    ui.painter().circle_stroke(slot.center(), 4.5, egui::Stroke::new(1.2, theme::muted_col()));
                    ui.label(theme::muted(*name).size(14.0));
                }
            });
        }
        // What is happening inside the current stage, when it says more than the stage's name.
        if let Some(detail) = said.last()
            && !detail.starts_with("Writing your question") && !detail.starts_with("Testing it with")
        {
            ui.add(egui::Label::new(theme::muted(detail).size(13.0)).wrap());
        }
        ui.add_space(8.0);
        // Past the usual minute, the line says so with the time, instead of repeating the estimate.
        let secs = (ui.input(|i| i.time) - st.write_started).max(0.0) as u64;
        let timing = if secs >= 60 {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
            format!("Taking longer than usual ({}:{:02}). The AI is still working; it keeps going while you use Import, Share, Figures or Settings.", secs / 60, secs % 60)
        } else {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
            "This usually takes between 10 seconds and a minute. It keeps going while you use Import, Share, Figures or Settings.".to_string()
        };
        ui.add(egui::Label::new(theme::muted(timing).size(13.0)).wrap());
        if let Some(note) = &st.wait_note {
            ui.add(egui::Label::new(theme::muted(note).size(13.0)).wrap());
        }
        ui.add_space(6.0);
        // Spotted a wrong description or level a few seconds in: no need to wait out the answer.
        let why = if revising { "Stop and keep the question as it was" } else { "Stop writing and go back to the description" };
        stop = crate::kit::secondary(ui, "Stop", 32.0).on_hover_text(why).clicked();
    });
    stop
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
        // A failed rewrite says so at the top, above the question it left unchanged, so the old
        // version is never approved by mistake.
        if c.follow_up_failed && let Some(why) = &c.follow_up_why {
            ui.add(egui::Label::new(RichText::new(why).color(theme::red()).size(14.0)).wrap());
            ui.add_space(6.0);
        }
        // The short name as the heading, the description as a quieter line: MyOpenMath's whole
        // "Name - Description" made a two-line heading to read before anything else.
        let (title, about) = c.q.name.split_once(" - ").map_or((c.q.name.as_str(), ""), |(t, a)| (t.trim(), a.trim()));
        ui.add(egui::Label::new(theme::serif(if title.is_empty() { "Your question" } else { title }).size(20.0)).wrap());
        if !about.is_empty() { ui.add(egui::Label::new(theme::muted(about).size(13.0)).wrap()); }
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
            // One short line before the one button (the preview itself shows the rest).
            "Answer it as a student would."
        } else {
            "Answer it as a student would, then check the worked solution."
        };
        ui.add(egui::Label::new(theme::muted(preview_help).size(13.0)).wrap());
        ui.add_space(6.0);
        let ready = !stale && !working && st.current.as_ref().is_some_and(|c| c.passed == Some(true));
        if !working && stale && st.error.is_some() && st.current.as_ref().is_some_and(|c| c.passed == Some(true)) {
            ui.horizontal_wrapped(|ui| {
                if pageshell::outline(ui, "Retry student preview").clicked() {
                    start_preview(st, ctx);
                }
                // The fallback belongs where the preview failed, not beside it every time.
                #[cfg(not(target_arch = "wasm32"))]
                if crate::kit::link(ui, "Try it as text instead", false).clicked() {
                    open_sandbox(st, ctx, true);
                }
            });
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
                if crate::kit::link(ui, "Try with text controls", false)
                    .on_hover_text("Open the student preview as native text and form controls").clicked()
                {
                    open_sandbox(st, ctx, true);
                }
            }));
        }

        ui.add_space(14.0);
        let (seen, reviewed) = st.current.as_ref().map(|c| (c.preview_opened, c.reviewed)).unwrap_or_default();
        if reviewed {
            // Already published: say so and lead to its finish, not to adding it a second time.
            if st.published.is_none() { st.published = Some(published_ids()); }
            let in_mom = st.current.as_ref().and_then(|c| c.saved_id.as_ref())
                .is_some_and(|id| st.published.as_ref().is_some_and(|p| p.contains(id)));
            let (done, next) = if in_mom { ("✔ In MyOpenMath", "See Where It Is") } else { ("✔ Approved and saved", "Add It to MyOpenMath") };
            ui.horizontal(|ui| {
                ui.label(RichText::new(done).color(theme::green()));
                if pageshell::primary(ui, next).clicked() { st.step = Step::Copy; }
            });
        } else {
            ui.label(theme::strong("Happy with it?").size(15.0));
            ui.add_space(4.0);
            // One press approves: opening the student preview is the check (approve() refuses
            // without it), so a checkbox restating it was a second decision for the same thing.
            if pageshell::primary_on(ui, "Looks Good", seen && !working).clicked() && seen && !working {
                if let Some(c) = st.current.as_mut() { c.preview_confirmed = true; }
                approve(st);
            }
            // Names the section above (either of its buttons counts), so the reason points at
            // something on screen.
            if !seen { crate::kit::gated_reason(ui, "Try it as a student first, with either button above."); }
        }

        error_box(ui, st);

    });
    // A question that failed its check already offers "Rewrite with AI" and "Change My Choices":
    // the revise card would be a second way to do the same thing.
    let failed = st.current.as_ref().is_some_and(|c| c.passed == Some(false));
    if st.step == Step::Try && !failed {
        ui.add_space(12.0);
        question_follow_ups(ui, ctx, st, cfg);
        ui.add_space(12.0);
        question_video(ui, ctx, st, cfg);
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

/// The original request plus the recent follow-ups, so a revision keeps what was already asked.
fn follow_up_context(current: &Current) -> String {
    let mut context = current.request.clone();
    if !current.follow_ups.is_empty() {
        context.push_str("\n\nRecent follow-ups already reflected in the current draft:");
        for earlier in current.follow_ups.iter().rev().take(8).rev() {
            context.push_str("\n- ");
            context.push_str(earlier);
        }
    }
    context
}

fn question_follow_ups(ui: &mut Ui, ctx: &egui::Context, st: &mut State, cfg: &crate::ai::AiCfg) {
    let working = busy(st);
    let revising = st.job.lock().is_ok_and(|job| job.busy && job.what == "Revising");
    let Some(current) = st.current.as_mut() else { return };
    let mut send = None;
    pageshell::card(ui, |ui| {
        control_edges(ui);
        // Heading, then the changes: short enough that the card's controls finish in view.
        pageshell::heading(ui, "Not quite right?");
        ui.add_space(4.0);
        // The three changes asked for most, then Rewrite It on the same row so it is in view without
        // scrolling; the other changes one level down; your own words in the box under them. A
        // change writes itself into the box; Rewrite It sends it, so a stray click never replaces
        // the draft. Drawn as real buttons (the 3:1 control edge): pale chips read as disabled.
        let chip = |ui: &mut Ui, change: Change, current: &mut Current| {
            let on = current.follow_up_draft.trim() == change.label();
            let r = if on { crate::kit::secondary_accent(ui, change.label(), 32.0) } else { crate::kit::secondary(ui, change.label(), 32.0) };
            if r.clicked() {
                current.follow_up_draft = if on { String::new() } else { change.label().to_string() };
            }
        };
        let mut rewrite = false;
        ui.add_enabled_ui(!working, |ui| {
            ui.horizontal_wrapped(|ui| {
                for change in [Change::Easier, Change::Harder, Change::Numbers] { chip(ui, change, current); }
                ui.add_space(8.0);
                let can_send = !current.follow_up_draft.trim().is_empty();
                let shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter));
                rewrite = ui.add_enabled_ui(can_send, |ui| pageshell::outline(ui, "Rewrite It")).inner
                    .on_hover_text(format!("Or press {shortcut} in the box below")).clicked() && can_send;
            });
            // A disabled button says what it waits for.
            if current.follow_up_draft.trim().is_empty() {
                crate::kit::gated_reason(ui, "Pick a change above, or type one below, then press Rewrite It.");
            }
            egui::CollapsingHeader::new(theme::muted("More changes").size(13.0)).id_salt("mom_more_changes").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for change in [Change::Story, Change::Wording, Change::Solution, Change::AddPart] { chip(ui, change, current); }
                });
            });
            let input = ui.add(egui::TextEdit::multiline(&mut current.follow_up_draft)
                .desired_width(f32::INFINITY).desired_rows(2)
                .hint_text(format!("Or type a change, then press Rewrite It ({}). For example: “Keep the graph and add a second part.”",
                    ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter)))));
            input.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, !working, "Question follow-up"));
            rewrite |= input.has_focus() && !current.follow_up_draft.trim().is_empty()
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
            if rewrite {
                let message = std::mem::take(&mut current.follow_up_draft).trim().to_string();
                current.follow_up_pending = message.clone();
                // A change's own text sends that change's full instruction.
                let change = Change::ALL.into_iter().find(|c| *c != Change::Other && c.label() == message).unwrap_or(Change::Other);
                let request = form::change_request(&follow_up_context(current), &current.q.emit(), change, &message);
                current.follow_up_failed = false;
                send = Some((request, message));
            }
        });
        if !current.follow_ups.is_empty() {
            ui.add_space(8.0);
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
        if revising {
            ui.horizontal(|ui| { ui.spinner(); ui.label("Revising and checking your question…"); });
        } else if current.follow_up_failed {
            ui.add(egui::Label::new("Your change is back in the box: edit it or send it again.").wrap());
        }

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
        let adding = !student && working && st.job.lock().map(|j| j.what == "MyOpenMath").unwrap_or(false);
        let label = if student { "Back to review" } else if adding { "Stop adding" } else { "Back to transfer" };
        let r = pageshell::outline(ui, label);
        back = if adding { r.on_hover_text(STOP_ADDING_HOVER) } else { r }.clicked();
        // A real stop: the add pauses just before Save and checks this through the pause.
        if back && adding { crate::mom::poster::STOP_BEFORE_SAVE.store(true, std::sync::atomic::Ordering::SeqCst); }
        ui.label(theme::serif(surface.title()).size(22.0));
        if adding {
            // The pause before Save says so in plain sight, next to the button that cancels it.
            match st.log.last().filter(|l| crate::mom::poster::is_save_hold(l)) {
                Some(line) => { ui.label(theme::strong(line).size(14.0)); }
                None => { ui.label(theme::muted("This usually takes under a minute.").size(13.0)); }
            }
        }
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
    // Already in MyOpenMath: the step is done. Show the finish and the next thing to do, not the
    // pickers, the sign-in and the by-hand fallback again; adding it somewhere else is one link.
    if st.published.is_none() { st.published = Some(published_ids()); }
    let in_mom = c.saved_id.as_ref().is_some_and(|id| st.published.as_ref().is_some_and(|p| p.contains(id)));
    #[cfg(not(target_arch = "wasm32"))]
    if in_mom && !st.add_elsewhere {
        let receipt = c.saved_id.as_deref().and_then(receipt_for);
        // The short name: the full "Name - Description" pushed the place to the end of two lines.
        let name = c.q.name.trim().split_once(" - ").map_or(c.q.name.trim(), |(t, _)| t.trim()).to_string();
        let revision = c.revised_from;
        st.poster.load();
        pageshell::card(ui, |ui| {
            ui.label(RichText::new("✔ It's in MyOpenMath").color(theme::green()).family(theme::bold_family()).size(20.0));
            let place = receipt.as_ref().and_then(|p| st.poster.place_name(p.course, p.assessment));
            let line = match (&receipt, place) {
                (Some(_), Some(place)) => format!("“{name}” is in {place}. Students see it the next time they open the assessment."),
                (Some(_), None) => format!("“{name}” is in your MyOpenMath assessment. Students see it the next time they open it."),
                _ => format!("“{name}” is in your MyOpenMath assessment."),
            };
            ui.add(egui::Label::new(theme::text(line).size(14.0)).wrap());
            if let Some(revision) = &revision { ui.label(theme::muted(revision.line(timestamp())).size(12.0)); }
            if let Some(p) = &receipt { ui.label(theme::muted(format!("MyOpenMath question ID {}", p.question)).size(12.0)); }
            // Where else it is: the end says the work is kept, not only where it went.
            ui.label(theme::muted("It's also kept in My Questions, marked In MyOpenMath.").size(12.0));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                finish_actions(ui, st);
                if let Some(p) = &receipt {
                    ui.add_space(8.0);
                    if crate::kit::link(ui, "See it in MyOpenMath", true).clicked() { autoadd::see_in_mom(st, ui.ctx(), p.course, p.assessment); }
                }
            });
            ui.add_space(4.0);
            if crate::kit::link(ui, "Add it to a different assessment", false).clicked() { st.add_elsewhere = true; }
        });
        return;
    }
    let _ = in_mom;
    // Which question this is, and that this is the short last step.
    let name = c.q.name.trim();
    // One short sentence, then the question's short name on its own quieter line: the name
    // wrapped inside the sentence made it hard to read.
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("✔ Approved and saved.").color(theme::green()).size(14.0));
        ui.label(theme::muted("Last step: choose where it goes.").size(14.0));
    });
    let short = name.split_once(" - ").map_or(name, |(t, _)| t.trim());
    if !short.is_empty() { ui.label(theme::strong(short).size(14.0)); }
    ui.add_space(6.0);
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
    // On the desktop the automatic add above is the way; copying by hand (and the bookmark that
    // fills the boxes) is one closed fallback, not two more cards to weigh. The web build has no
    // automatic add, so there the steps are the card.
    pageshell::card(ui, |ui| {
        if cfg!(target_arch = "wasm32") {
            pageshell::heading(ui, "Add it to MyOpenMath");
            ui.add_space(8.0);
            manual_copy(ui, st, &fields, &menu, &mut copied);
        } else {
            egui::CollapsingHeader::new(theme::strong("Can't connect? Add it by hand or with the bookmark").size(15.0)).id_salt("mom_copy_fallback").show(ui, |ui| {
                manual_copy(ui, st, &fields, &menu, &mut copied);
            });
        }
    });
    if let Some(n) = copied { st.note = Some(n); }
}

/// The bookmark that fills every box in one click (for the teacher who uses it each time), then
/// the four MyOpenMath boxes by hand.
fn manual_copy(ui: &mut Ui, st: &mut State, fields: &[(&str, String, &str)], menu: &str, copied: &mut Option<String>) {
    ui.label(theme::strong("Fastest: fill every box with one click").size(14.0));
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
    ui.add_space(12.0);
    ui.label(theme::strong("Or copy each box").size(14.0));
    ui.add_space(6.0);
    ui.label(theme::strong("1.  In MyOpenMath, open your course, click Manage Question Set, then Add New Question.").size(14.0));
    ui.add_space(10.0);
    ui.label(theme::strong(format!("2.  Set the Question type menu to \"{menu}\".")).size(14.0));
    ui.add_space(10.0);
    ui.label(theme::strong("3.  Copy each item and paste it into the box with the same name.").size(14.0));
    ui.add_space(6.0);
    for (name, value, place) in fields {
        if value.trim().is_empty() { continue; }
        // Indented to the steps' text (after "3.  "), not past it.
        ui.horizontal(|ui| {
            ui.add_space(17.0);
            if pageshell::outline(ui, &format!("Copy {name}")).clicked() {
                ui.ctx().copy_text(value.clone());
                *copied = Some(format!("Copied. Now paste it into {place} in MyOpenMath."));
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

}

fn open_saved(st: &mut State, saved: Saved) {
    // The list goes back to its top, so its heading and done count are in view at the question's
    // finish (it stayed wherever the teacher had scrolled to find the row).
    st.side_to_top = true;
    st.tool_page = None;
    st.add_elsewhere = false;
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
        revised_from: saved.revised_from,
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
            ui.label(theme::muted("Questions you write or convert appear here, saved as soon as they are written.").size(13.0));
        }
        let mut open: Option<Saved> = None;
        let mut delete: Option<String> = None;
        if st.published.is_none() { st.published = Some(published_ids()); }
        let published = st.published.clone().unwrap_or_default();
        // Done first: what is finished, then what is waiting on the teacher.
        let (mut done, mut ready, mut review, mut fix) = (0, 0, 0, 0);
        for s in &st.library.questions {
            if published.contains(&s.id) { done += 1 }
            else if !s.passed && !s.needs_check { fix += 1 }
            else if s.reviewed { ready += 1 }
            else { review += 1 }
        }
        // Finished on its own line, then what is left, so neither breaks mid-phrase.
        // Counted against the whole list, so the progress has an end in sight.
        let total = st.library.questions.len();
        if done > 0 { ui.label(RichText::new(format!("✔ {done} of {total} in MyOpenMath")).color(theme::green()).size(12.5)); }
        // In the rows' own order (to fix, to review, ready), so the counts map straight onto them.
        // The same totals, pinned over the column's top while it is scrolled (see `column`).
        {
            // The list's own words, on two lines so they fit the narrow column.
            let mut rest: Vec<String> = Vec::new();
            for (n, what) in [(fix, "to fix"), (review, "to review"), (ready, "ready to add")] { if n > 0 { rest.push(format!("{n} {what}")); } }
            let summary = format!("✔ {done} of {} in MyOpenMath\n{}", st.library.questions.len(), rest.join(" · "));
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("mom_side", "sticky")), summary));
        }
        // Each count is a link to its first question, so picking up the next one isn't a scan.
        let not_written = (!busy(st) && st.current.is_none() && st.error.is_some() && !st.form.topic_text.trim().is_empty()) as usize;
        let mut parts: Vec<(u8, String)> = [(0u8, fix, "to fix"), (1, review, "to review"), (2, ready, "ready to add")]
            .into_iter().filter(|(_, n, _)| *n > 0).map(|(g, n, what)| (g, format!("{n} {what}"))).collect();
        // A write that failed counts as unfinished work too (it heads the list; the link goes there).
        if not_written > 0 { parts.insert(0, (9, format!("{not_written} not written yet"))); }
        if !parts.is_empty() && st.library.questions.len() > 1 {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (i, (group, text)) in parts.iter().enumerate() {
                    if i > 0 { ui.label(theme::muted("·").size(12.5)); }
                    if crate::kit::link(ui, text, true).on_hover_text("Go to the first one").clicked() { st.jump_to_group = Some(*group); }
                }
            });
        }
        // A write that failed (a busy AI, a dropped connection) is unfinished work too: its request
        // heads the list with Try Again, so it isn't only on the form that happens to be open.
        if !busy(st) && st.current.is_none() && st.error.is_some() && !st.form.topic_text.trim().is_empty() {
            if st.jump_to_group == Some(9) { st.jump_to_group = None; ui.scroll_to_cursor(Some(egui::Align::Min)); }
            ui.add_space(10.0);
            let asked = st.form.topic_text.trim();
            let title: String = asked.chars().take(60).collect::<String>() + if asked.chars().count() > 60 { "…" } else { "" };
            ui.add(egui::Label::new(theme::strong(title).size(14.0)).wrap());
            // Why it stopped, in the same plain words as the message on the form.
            if let Some(e) = &st.error {
                let why = form::plain_error(e);
                let first = why.split(". ").next().unwrap_or(&why).trim_end_matches('.').to_string();
                ui.add(egui::Label::new(theme::muted(format!("{first}.")).size(12.0)).wrap());
            }
            ui.horizontal(|ui| {
                crate::kit::status_tag_w(ui, "Not written yet", theme::on_attention(), 112.0);
                ui.add_space(8.0);
                // Back to the description, where Write My Question is the one retry.
                if crate::kit::secondary(ui, "Show", 32.0).on_hover_text("Go back to the description; press Write My Question to try again").clicked() {
                    request_destination(ui.ctx(), Destination::Questions);
                }
            });
        }
        // A question being written is unfinished work too: it heads the list, with the way back to it.
        let writing_now = st.job.lock().map(|j| j.busy && matches!(j.what, "Writing" | "Revising")).unwrap_or(false);
        if writing_now {
            ui.add_space(10.0);
            let asked = st.form.topic_text.trim();
            let title: String = if asked.is_empty() { "Your new question".into() } else { asked.chars().take(60).collect::<String>() + if asked.chars().count() > 60 { "…" } else { "" } };
            ui.add(egui::Label::new(theme::strong(title).size(14.0)).wrap());
            ui.horizontal(|ui| {
                // Where it has got and for how long, so from another page it doesn't look frozen.
                let stage = ["Writing it", "Testing it", "Almost ready"][write_stage(st).min(2)];
                let secs = (ui.input(|i| i.time) - st.write_started).max(0.0) as u64;
                ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                crate::kit::status_tag(ui, &format!("{stage} · {}:{:02}", secs / 60, secs % 60), theme::accent_col());
                ui.add_space(8.0);
                if crate::kit::secondary(ui, "Show", 32.0).on_hover_text("Go to its progress, with Stop").clicked() {
                    request_destination(ui.ctx(), Destination::Questions);
                }
            });
        }
        // The rows below grey out during a job: say why, so they don't look broken.
        if busy(st) && !st.library.questions.is_empty() {
            ui.add_space(4.0);
            let what = st.job.lock().map(|j| j.what).unwrap_or("");
            let job = match what { "Writing" => "the question being written is done", "Revising" => "the rewrite is done", "MyOpenMath" => "the add to MyOpenMath is done", _ => "the worksheet job is done" };
            ui.add(egui::Label::new(theme::muted(format!("Open and Delete wait until {job}. To stop it, press Show, then Stop.")).size(12.0)).wrap());
        }
        // What needs the teacher comes first (to fix, to review, ready to add), finished work last:
        // the question most in need of a look is never the one under the fold. Saved order within each.
        let rank = |s: &Saved| if published.contains(&s.id) { 3 } else if !s.passed && !s.needs_check { 0 } else if s.reviewed { 2 } else { 1 };
        let mut order: Vec<usize> = (0..st.library.questions.len()).collect();
        order.sort_by_key(|&i| rank(&st.library.questions[i]));
        let on_screen = st.current.as_ref().and_then(|c| c.saved_id.clone());
        for s in order.iter().map(|&i| &st.library.questions[i]) {
            // A count above was pressed: bring the first question of its group into view.
            if st.jump_to_group == Some(rank(s)) {
                st.jump_to_group = None;
                ui.scroll_to_cursor(Some(egui::Align::Min));
            }
            let q = MomQuestion::parse(&s.src);
            let name = if q.name.is_empty() { "Untitled question".to_string() } else { q.name.clone() };
            // MyOpenMath's "Name - Description": the name is what tells rows apart, so it leads and
            // the description follows, quieter, on its own line.
            let (title, about) = name.split_once(" - ").map_or((name.as_str(), ""), |(t, a)| (t.trim(), a.trim()));
            // Entries apart from each other by clearly more than an entry's own lines are, with a
            // hairline between them, so each row reads as one unit.
            ui.add_space(10.0);
            let (line, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().hline(line.x_range(), line.center().y, egui::Stroke::new(1.0, theme::border()));
            ui.add_space(10.0);
            ui.add(egui::Label::new(theme::strong(title).size(14.0)).wrap());
            if !about.is_empty() {
                ui.add(egui::Label::new(theme::muted(about).size(12.0)).wrap());
            }
            if let Some(revision) = &s.revised_from {
                ui.add(egui::Label::new(theme::muted(revision.line(timestamp())).size(12.0)).wrap());
            }
            if let Some(source) = &s.worksheet_source {
                ui.add(egui::Label::new(theme::muted(source).size(12.0)).wrap());
            }
            // What failed, on the row, so the teacher can choose which to take on without opening each.
            // What is left on a question that waits for review, as the fix rows say what failed.
            if s.passed && !s.reviewed && !published.contains(&s.id) {
                ui.add(egui::Label::new(theme::muted("Try it as a student, then approve it.").size(12.0)).wrap());
            }
            if !s.passed && !s.needs_check && let Some(problem) = s.problems.first() {
                let problem = plain_problem(problem);
                let short: String = problem.chars().take(160).collect();
                let text = if short.len() < problem.len() { format!("{}…", short.trim_end()) } else { short };
                // Quiet text: the red dot already marks the row; a red sentence pulled the eye from
                // the page's own job (Convert, Import).
                ui.add(egui::Label::new(theme::muted(text).size(12.0)).wrap());
            }
            let paused = busy(st);
            ui.horizontal(|ui| {
                // Finished work reads as finished: a question already in MyOpenMath says so.
                // A dot and a word, not a filled chip: the status is read, Open is pressed, and a
                // chip as tall as Open drew the eye from it.
                let (status, fg) = if published.contains(&s.id) {
                    ("In MyOpenMath", theme::green())
                } else if !s.passed && !s.needs_check {
                    (saved_status(s), theme::red())
                } else if s.reviewed {
                    ("Ready to add", theme::accent_col())
                } else {
                    (saved_status(s), theme::on_attention())
                };
                // One slot for the word, so every row's Open starts on the same edge.
                // The status stays at full strength during a job (it is still true); only the
                // controls after it pause.
                // Finished work wears a tick, not the same dot as work still waiting.
                if published.contains(&s.id) { crate::kit::status_done_w(ui, status, fg, 112.0); } else { crate::kit::status_tag_w(ui, status, fg, 112.0); }
                ui.add_space(8.0);
                ui.add_enabled_ui(!paused, |ui| {
                // The question already on screen says so, instead of offering to open it again.
                if on_screen.as_deref() == Some(s.id.as_str()) {
                    ui.add_sized(egui::vec2(0.0, 32.0), egui::Label::new(theme::muted("On screen").size(12.5)));
                } else if crate::kit::secondary(ui, "Open", 32.0).clicked() { open = Some(s.clone()); }
                // Delete sits at the far end of the row, away from Open.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if st.confirm_delete.as_deref() == Some(s.id.as_str()) {
                        if crate::kit::secondary(ui, "Keep it", 32.0).clicked() { st.confirm_delete = None; }
                        if crate::kit::danger_text_button(ui, "Yes, delete").clicked() { delete = Some(s.id.clone()); }
                    // A real 32 px target with its icon, quiet and at the far end from Open; it still asks
                    // before anything is removed.
                    } else if {
                        // A 32 x 32 icon button drawn like Open (same height, corners and edge, and the
                        // one unavailable look when paused), kept small so it sits well apart from
                        // Open in the narrow column. Named "Delete" for screen readers and on hover.
                        let enabled = ui.is_enabled();
                        let (rect, del) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
                        let (fill, edge, fg) = if enabled {
                            (if del.hovered() { theme::hover_bg() } else { theme::card() }, theme::edge(), theme::ink())
                        } else {
                            let (f, t, e) = pageshell::unavailable_colors();
                            (f, e, t)
                        };
                        ui.painter().rect(rect, egui::CornerRadius::same(12), fill, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, egui_phosphor::regular::TRASH, theme::icon_font(16.0), fg);
                        del.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Delete"));
                        let del = if enabled { del.on_hover_cursor(egui::CursorIcon::PointingHand) } else { del };
                        del.on_hover_text("Delete this question (D.A.D asks first)").clicked() && enabled
                    } {
                        st.confirm_delete = Some(s.id.clone());
                    }
                });
                });
            });
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
    fn question_code_copied_from_an_ai_chat_imports() {
        let reply = "Here is your question:\n```\n// === NAME - DESCRIPTION: Add one ===\n// === SET QUESTION TYPE TO: number ===\n\n// === COMMON CONTROL ===\n$a = rand(2,9)\n$answer = $a + 1\n\n// === QUESTION TEXT ===\nWhat is `$a + 1`? $answerbox\n```\nLet me know if you want changes.";
        let file = PickedFile { name: "Pasted code".into(), bytes: reply.as_bytes().to_vec() };
        let q = MomQuestion::parse(&imported_sources(&file).unwrap()[0]);
        assert_eq!(q.name, "Add one");
        assert!(q.qtext.contains("$answerbox") && !q.qtext.contains("```") && !q.qtext.contains("Let me know"), "{q:?}");
        // A fenced block with no section markers is still the code, not the sentence before it.
        assert_eq!(unwrap_reply("Here is the code:\n```php\n$a = rand(2,9)\n$answer = $a + 1\n```\nEnjoy!"), "$a = rand(2,9)\n$answer = $a + 1");
        assert_eq!(unwrap_reply("Here:\n```\n$a = 1"), "$a = 1", "an unclosed fence runs to the end");
        let two = "Common Control:\n```\n$a = rand(2,9)\n$answer = $a + 1\n```\nQuestion Text:\n```\nWhat is `$a + 1`? $answerbox\n```";
        let q = MomQuestion::parse(&unwrap_reply(two));
        assert!(q.control.contains("rand(2,9)") && q.qtext.contains("$answerbox"), "{q:?}");
        // Both boxes copied as plain text: split where the code ends.
        let q = MomQuestion::parse(&unwrap_reply("$a = rand(2,9)\n$answer = $a + 1\nWhat is `$a + 1`? $answerbox"));
        assert!(q.control.contains("$answer = $a + 1") && q.qtext.starts_with("What is"), "{q:?}");
        // Or at a heading.
        let q = MomQuestion::parse(&unwrap_reply("// Common Control\n$a = rand(2,9)\n// Question Text\nWhat is $a? $answerbox"));
        assert!(q.control.contains("rand(2,9)") && q.qtext.contains("$answerbox") && !q.control.contains("Common Control"), "{q:?}");
        // Two blocks that are not the two boxes: the first one only.
        assert_eq!(unwrap_reply("V1:\n```\n$a = 1\n```\nV2:\n```\n$a = 2\n```"), "$a = 1");
    }

    #[test]
    fn an_iphone_photo_gets_the_way_to_fix_it() {
        assert!(worksheet::iphone_photo_help("IMG_0042.HEIC").is_some_and(|m| m.contains("JPEG")));
        assert!(worksheet::iphone_photo_help("page.jpg").is_none());
    }

    #[test]
    fn a_paste_that_is_not_a_question_says_what_it_is() {
        assert_eq!(not_a_question("https://www.myopenmath.com/course/moddataset.php?cid=1&id=12345"),
            NotAQuestion::Link("https://www.myopenmath.com/course/moddataset.php?cid=1&id=12345".into()));
        assert!(matches!(not_a_question("A shirt costs $25 and is 20% off. What is the sale price?"), NotAQuestion::Words(_)));
        assert!(matches!(not_a_question("What is `$a + 1`? $answerbox"), NotAQuestion::QtextOnly(_)));
        assert!(matches!(not_a_question("$5,000 is invested at 4% interest. How much is there after 3 years?"), NotAQuestion::Words(_)));
        assert!(matches!(not_a_question("$f(x) = 3x + 2$. Find f(4)."), NotAQuestion::Words(_)));
        assert!(matches!(not_a_question("$y = 2x + 3$. What is the slope?"), NotAQuestion::Words(_)));
        for words in ["Factor (x^2 - 9)", "Simplify (x+2)(x-3)", "Evaluate (2/3) + (1/4)"] {
            assert!(matches!(not_a_question(words), NotAQuestion::Words(_)), "{words}");
        }
        assert!(matches!(not_a_question("loadlibrary(\"stats\")\n$a = 1"), NotAQuestion::ControlOnly(_)));
        assert!(looks_like_question_code("if ($a > 1) {\n  $b = 2\n}"));
        assert_eq!(not_a_question("Find the slope of the line through (1, 2) and (3, 8)."),
            NotAQuestion::Words("Find the slope of the line through (1, 2) and (3, 8).".into()));
        assert!(matches!(not_a_question("$a = rand(2,9)\n$answer = $a + 1"), NotAQuestion::ControlOnly(_)));
    }

    #[test]
    fn common_control_and_question_text_pasted_apart_import_as_one_question() {
        // The first press finds no Question Text and asks for it; nothing is saved yet.
        let mut st = State::default();
        st.import_source = "$a = rand(2,9)\n$answer = $a + 1".into();
        import_drafts(&mut st);
        assert!(matches!(st.import_hint, Some(NotAQuestion::ControlOnly(_))));
        assert!(st.lib_note.is_none() && st.library.questions.is_empty(), "no 'Imported 0' beside the hint");
        let q = MomQuestion::parse(&joined_paste("$a = rand(2,9)\n$answer = $a + 1", "What is `$a + 1`? $answerbox"));
        assert!(q.control.contains("rand(2,9)") && q.qtext.contains("$answerbox"));
    }

    #[test]
    fn a_backup_comes_back_approved_and_a_share_package_does_not_count_as_one() {
        let backup = Library { questions: vec![Saved { id: "a".into(), request: "Slope".into(), src: "// x".into(), passed: true, reviewed: true, saved_at: 1, ..Default::default() }], worksheet: None };
        let file = PickedFile { name: "dad-my-questions.json".into(), bytes: serde_json::to_vec(&backup).unwrap() };
        let restored = backup_records(&file).expect("a backup file");
        assert!(restored[0].reviewed && restored[0].passed && restored[0].request == "Slope");
        let share = PickedFile { name: "package.json".into(), bytes: br#"{"questions":[{"src":"// x"}]}"#.to_vec() };
        assert!(backup_records(&share).is_none());
        assert_eq!(imported_sources(&share).unwrap(), vec!["// x".to_string()]);
    }

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

    /// The AI's answer to "revise this question", as `take` receives it.
    fn revision_arrives(st: &mut State, name: &str) {
        let q = MomQuestion { name: name.into(), qtype: "number".into(), qtext: "T".into(), ..Default::default() };
        take(st, Outcome::Authored { follow_up: Some("make it harder".into()), q: Some(q), passed: true, problems: vec![], untested: vec![], error: None });
    }

    /// A published question the teacher opened to revise.
    fn published_question(st: &mut State, id: &str, name: &str, saved_at: u64, revised_from: Option<Revision>) {
        st.library.questions.insert(0, Saved { id: id.into(), src: MomQuestion { name: name.into(), ..Default::default() }.emit(), passed: true, reviewed: true, saved_at, revised_from, ..Default::default() });
        st.published.get_or_insert_with(Default::default).insert(id.into());
        st.current = Some(Current { saved_id: Some(id.into()), reviewed: true, revised_from, ..Default::default() });
    }

    #[test]
    fn a_revision_is_shown_apart_from_its_original() {
        let mut st = State::default();
        let first = 1_700_000_000; // Nov 14, 2023
        published_question(&mut st, "orig", "Solve a two-step equation", first, None);
        revision_arrives(&mut st, "Solve a two-step equation");
        let id = st.current.as_ref().unwrap().saved_id.clone().unwrap();
        assert_ne!(id, "orig", "the revision is a new question");
        let (original, revision) = (st.library.questions.iter().find(|s| s.id == "orig").unwrap(), st.library.questions.iter().find(|s| s.id == id).unwrap());
        assert_eq!(original.revised_from, None, "the earlier version is as it was");
        assert_eq!(revision.revised_from, Some(Revision { added_at: first, number: 1 }));
        // The name, which MyOpenMath takes as the Description, is the same; the second line is what differs.
        assert_eq!(MomQuestion::parse(&original.src).name, MomQuestion::parse(&revision.src).name);
        assert_eq!(revision.revised_from.unwrap().line(first), "Revised from the question added on Nov 14");
        assert!(st.current.as_ref().unwrap().revised_from.is_some(), "the finish shows it too");
        assert!(st.lib_note.as_deref().is_some_and(|n| n.starts_with("Saved as a new question")));
    }

    #[test]
    fn a_question_never_published_is_unchanged_by_a_rewrite() {
        let mut st = State::default();
        st.published = Some(Default::default());
        st.library.questions.insert(0, Saved { id: "draft".into(), src: MomQuestion { name: "Slope".into(), ..Default::default() }.emit(), passed: true, saved_at: 5, ..Default::default() });
        st.current = Some(Current { saved_id: Some("draft".into()), ..Default::default() });
        revision_arrives(&mut st, "Slope");
        assert_eq!(st.library.questions.len(), 1, "rewritten in place");
        assert_eq!(st.library.questions[0].id, "draft");
        assert_eq!(st.library.questions[0].revised_from, None);
        assert_eq!(st.current.as_ref().unwrap().revised_from, None);
        assert!(st.lib_note.as_deref().is_none_or(|n| !n.starts_with("Saved as a new question")));
    }

    #[test]
    fn a_second_revision_stays_distinct() {
        let mut st = State::default();
        published_question(&mut st, "orig", "Slope", 1_700_000_000, None);
        revision_arrives(&mut st, "Slope");
        // Another rewrite before the revision is added: still the one revision, in place.
        revision_arrives(&mut st, "Slope");
        let rev1 = st.current.as_ref().unwrap().saved_id.clone().unwrap();
        assert_eq!(st.library.questions.len(), 2);
        assert_eq!(st.library.questions.iter().find(|s| s.id == rev1).unwrap().revised_from.unwrap().number, 1);
        // The revision is added to MyOpenMath, and revised again, the same day.
        st.published.as_mut().unwrap().insert(rev1.clone());
        revision_arrives(&mut st, "Slope");
        let rev2 = st.current.as_ref().unwrap().saved_id.clone().unwrap();
        assert!(rev2 != rev1 && rev2 != "orig");
        assert_eq!(st.library.questions.len(), 3);
        let lines: Vec<String> = ["orig", rev1.as_str(), rev2.as_str()].iter()
            .map(|id| st.library.questions.iter().find(|s| s.id == *id).unwrap().revised_from.map(|r| r.line(1_700_000_000)).unwrap_or_default())
            .collect();
        assert_eq!(lines[0], "");
        assert_eq!(lines[1], "Revised from the question added on Nov 14");
        assert!(lines[2].starts_with("Revision 2, from the question added on"), "{}", lines[2]);
        assert_ne!(lines[1], lines[2], "two revisions made on one day still read differently");
    }

    #[test]
    fn a_revision_line_names_the_year_only_when_it_is_not_this_year() {
        let nov_25_2026 = 1_795_564_800;
        assert_eq!(short_date(nov_25_2026, nov_25_2026 + 5), "Nov 25");
        assert_eq!(short_date(nov_25_2026, nov_25_2026 + 30 * 86_400), "Nov 25");
        assert_eq!(short_date(nov_25_2026, nov_25_2026 + 90 * 86_400), "Nov 25, 2026", "Feb 2027");
        assert_eq!(short_date(nov_25_2026, nov_25_2026 + 400 * 86_400), "Nov 25, 2026");
    }

    #[test]
    fn a_saved_library_from_before_revisions_still_opens() {
        let saved: Saved = serde_json::from_str(r#"{"id":"a","request":"x","src":"","passed":true,"saved_at":1}"#).unwrap();
        assert_eq!(saved.revised_from, None);
        let json = serde_json::to_string(&Saved { revised_from: Some(Revision { added_at: 9, number: 2 }), ..saved.clone() }).unwrap();
        assert_eq!(serde_json::from_str::<Saved>(&json).unwrap().revised_from, Some(Revision { added_at: 9, number: 2 }));
        assert!(!serde_json::to_string(&saved).unwrap().contains("revised_from"));
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
    struct Prefs {
        course: Option<u64>,
        assessment: Option<u64>,
        /// The last place published to (files from before `places`).
        place: Option<(u64, u64, String)>,
        /// Every place published to, by name, so each finish can name its own place.
        places: Vec<(u64, u64, String)>,
    }

    impl Prefs {
        fn remember(&mut self, cid: u64, aid: u64, name: String) {
            self.places.retain(|(c, a, _)| (*c, *a) != (cid, aid));
            self.places.push((cid, aid, name.clone()));
            self.place = Some((cid, aid, name));
        }

        fn name_of(&self, cid: u64, aid: u64) -> Option<String> {
            self.places.iter().chain(self.place.as_ref())
                .find(|(c, a, _)| (*c, *a) == (cid, aid)).map(|(_, _, name)| name.clone())
        }
    }

    #[cfg(test)]
    mod place_tests {
        use super::Prefs;

        #[test]
        fn each_place_keeps_its_name_after_publishing_elsewhere() {
            let mut prefs = Prefs::default();
            prefs.remember(1, 10, "Algebra · Quiz 1".into());
            prefs.remember(2, 20, "Geometry · Test".into());
            prefs.remember(1, 10, "Algebra · Quiz 1 (renamed)".into());
            assert_eq!(prefs.name_of(1, 10).as_deref(), Some("Algebra · Quiz 1 (renamed)"));
            assert_eq!(prefs.name_of(2, 20).as_deref(), Some("Geometry · Test"));
            assert_eq!(prefs.name_of(3, 30), None);
            assert_eq!(prefs.places.len(), 2);
        }

        #[test]
        fn a_file_from_before_the_list_still_names_its_place() {
            let prefs: Prefs = serde_json::from_str(r#"{"course":5,"assessment":6,"place":[5,6,"Stats · Review"]}"#).unwrap();
            assert_eq!(prefs.name_of(5, 6).as_deref(), Some("Stats · Review"));
        }
    }

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

        /// The saved picks and place names. The finish needs them without the step-3 card having
        /// been drawn (a published question opened after a restart shows only the finish).
        pub(super) fn load(&mut self) {
            if std::mem::replace(&mut self.loaded, true) { return; }
            self.prefs = crate::store::load_json(PREFS).unwrap_or_default();
        }

        pub(super) fn message(&self) -> Option<&str> {
            self.result.as_ref().map(|result| match result { Ok(message) | Err(message) => message.as_str() })
        }

        /// "Course · Assessment" by name, when D.A.D has read that course's assessments.
        /// The place last published to, by name (remembered across restarts).
        pub(super) fn last_place(&self) -> Option<String> {
            self.prefs.place.as_ref().map(|(_, _, name)| name.clone())
        }

        pub(super) fn place_name(&self, cid: u64, aid: u64) -> Option<String> {
            let live = (|| {
                let course = self.courses.iter().find(|course| course.id == cid)?;
                let assessment = self.assessments.iter().find(|assessment| assessment.id == aid && self.assessments_for == Some(cid))?;
                Some(format!("{} · {}", course.name, assessment.name))
            })();
            // Not connected this session: the names saved when the teacher published there.
            live.or_else(|| self.prefs.name_of(cid, aid))
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
        poster::STOP_BEFORE_SAVE.store(false, std::sync::atomic::Ordering::SeqCst);
        // The names the pickers show now, kept with the ids so the finish can name the place later.
        if let Some(place) = st.poster.place_name(cid, aid) { st.poster.prefs.remember(cid, aid, place); let _ = crate::store::save_json(PREFS, &st.poster.prefs); }
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

    /// Open the signed-in MyOpenMath panel on the assessment's question list.
    pub(super) fn see_in_mom(st: &mut State, ctx: &egui::Context, cid: u64, aid: u64) {
        st.browser.open(Surface::Transfer);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            if let Ok(t) = tab(&ctx) { let _ = t.navigate(&format!("{}/course/addquestions2.php?cid={cid}&aid={aid}", poster::MOM)); }
            ctx.request_repaint();
        });
    }

    pub fn courses_in(st: &mut State, r: Result<Vec<Course>, String>) {
        match r {
            Ok(c) => {
                st.poster.courses = c;
                if st.poster.courses.is_empty() {
                    st.poster.result = Some(Err("No courses you teach were found on MyOpenMath.".into()));
                } else {
                    // Connected: back to the card, where the course and assessment are picked. The
                    // panel was left on MyOpenMath's home page with only "Back to transfer" to press.
                    st.browser.close();
                }
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
        st.published = None;
        // Done: back to the card, where the finish is said in full (the panel showed a plain line).
        if matches!(r, Ok(Added::Saved(_))) { st.browser.close(); }
        let place = st.poster.destination();
        st.poster.result = Some(match r {
            // The card's finish says it in full; no second green line under it.
            Ok(Added::Saved(_)) => { let _ = place; st.poster.result = None; return; }
            Ok(Added::SavedToSet(p, error)) => Err(format!("Question {} is in your MyOpenMath question set. Attachment is unconfirmed. {error}", p.question)),
            Ok(Added::Filled) => Ok("Save is disabled in this preview. Return to transfer to publish to the selected assessment.".into()),
            Err(e) => Err(e),
        });
    }

    pub fn card(ui: &mut Ui, ctx: &egui::Context, st: &mut State) {
        st.poster.load();
        let working = busy(st);
        let mine = st.job.lock().map(|j| j.what == "MyOpenMath").unwrap_or(false);
        pageshell::card(ui, |ui| {
            // Keep the destination controls visible against the card in both themes.
            ui.style_mut().visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, theme::edge());
            pageshell::heading(ui, "Add it to MyOpenMath for me");
            ui.add_space(4.0);
            if st.poster.courses.is_empty() {
                // With a remembered place, that is all there is to say before the button.
                if st.poster.last_place().is_none() {
                    ui.add(egui::Label::new(theme::muted("Connect to MyOpenMath in D.A.D. Pick a course and assessment to preview or publish the question.").size(13.0)).wrap());
                }
                // Where the last one went, so the last step looks as short as it is.
                if let Some(place) = st.poster.last_place() {
                    // Connecting picks the same place again (the pickers start on the remembered one).
                    ui.add(egui::Label::new(theme::strong(format!("Last added to {place}.")).size(13.5)).wrap());
                    ui.add(egui::Label::new(theme::muted("Connect, and D.A.D picks it again.").size(12.5)).wrap());
                }
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
                    let mut finished: Option<poster::Publication> = None;
                    ui.add_enabled_ui(!working, |ui| {
                        ui.horizontal_wrapped(|ui| {
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
                                    // The end of the job: say it is done, where, and offer the next one.
                                    finished = Some(p);
                                }
                                Some(PublishState::AwaitingConfirmation) => {
                                    ui.label(theme::muted("Save unconfirmed. Check the assessment in MyOpenMath."));
                                    if crate::kit::link(ui, "Open the assessment in MyOpenMath", true).clicked() { see_in_mom(st, ctx, cid, aid); }
                                }
                            }
                            // Step 2 already tried the question as a student: seeing it in MyOpenMath's
                            // own editor first is optional, so it is a quiet link after Publish.
                            if finished.is_none() {
                                ui.add_space(8.0);
                                if crate::kit::link(ui, "Preview in MyOpenMath first", false).clicked() { start_add(st, ctx, cid, aid, false); }
                            }
                        });
                        if let Some(p) = &finished {
                            ui.label(RichText::new("✔ It's in MyOpenMath").color(theme::green()).family(theme::bold_family()).size(18.0));
                            let place = st.poster.destination().unwrap_or_else(|| format!("assessment {}", p.assessment));
                            ui.add(egui::Label::new(theme::text(format!("It's published in {place}. Students see it the next time they open the assessment.")).size(14.0)).wrap());
                            ui.label(theme::muted(format!("MyOpenMath question ID {}", p.question)).size(12.0));
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                finish_actions(ui, st);
                                ui.add_space(8.0);
                                // In the panel that is already signed in, not a browser that may not be.
                                if crate::kit::link(ui, "See it in MyOpenMath", true).clicked() {
                                    see_in_mom(st, ui.ctx(), p.course, p.assessment);
                                }
                            });
                        }
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
                ui.label(theme::muted("This usually takes under a minute. You can watch it in the browser panel.").size(13.0));
                for line in st.log.iter().rev().take(3).collect::<Vec<_>>().into_iter().rev() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.add(egui::Label::new(theme::text(line).size(13.0)).wrap());
                    });
                }
            }
            match &st.poster.result {
                Some(Ok(m)) => { ui.add_space(6.0); ui.add(egui::Label::new(RichText::new(format!("✔ {m}")).color(theme::green()).size(14.0)).wrap()); }
                // A stop the teacher asked for is not a fault: said plainly, not in red.
                Some(Err(e)) if e.starts_with("Stopped before saving") => { ui.add_space(6.0); ui.add(egui::Label::new(theme::muted(e).size(14.0)).wrap()); }
                Some(Err(e)) => { ui.add_space(6.0); ui.add(egui::Label::new(RichText::new(e).color(theme::red()).size(14.0)).wrap()); }
                None => {}
            }
            ui.add_space(8.0);
            login_box(ui, st, false);
        });
    }

    pub(super) fn connection(ui: &mut Ui, st: &mut State) {
        pageshell::card(ui, |ui| {
            pageshell::heading(ui, "MyOpenMath connection");
            ui.add(egui::Label::new(theme::muted("Save your instructor login here, or sign in through the browser panel when adding a question.").size(13.0)).wrap());
            // The Settings section exists for this one control: open, not one more click away.
            login_box(ui, st, true);
        });
    }

    /// The optional saved MyOpenMath sign-in, kept in the computer's keychain.
    fn login_box(ui: &mut Ui, st: &mut State, open: bool) {
        let p = &mut st.poster;
        if p.has_login.is_none() { p.has_login = Some(poster::saved_login().is_some()); }
        egui::CollapsingHeader::new(theme::muted("Sign in automatically (optional)").size(13.0)).id_salt(("mom_post_login", open)).default_open(open).show(ui, |ui| {
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
                // Enter in the username goes on to the password, as sign-in forms do.
                let next = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let r = ui.add(egui::TextEdit::singleline(&mut p.pass).password(true).desired_width(180.0).hint_text("Password"));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "MyOpenMath password"));
                if next { r.request_focus(); }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let ok = !p.user.trim().is_empty() && !p.pass.is_empty();
                if (pageshell::primary_on(ui, "Save Login", ok).clicked() || enter) && ok {
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
