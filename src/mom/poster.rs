//! Adding a question to MyOpenMath in a real browser: check the sign-in (typing a saved password
//! when the teacher stored one), list the teacher's courses and assessments, open the assessment's
//! Add New Question editor, fill every box, tick "Uses random variables" for the worked solution,
//! and press Save when the teacher chose that. Otherwise the filled editor is left for them.
//!
//! The steps follow MyOpenMath's pages as read on 2026-11-21: the home page lists taught courses
//! (each has a "Transfer ownership" link), a course page links its assessments as
//! `../assess2/?cid=&aid=`, and an assessment's Add New Question is
//! `course/moddataset.php?aid=&cid=&from=addq2`, whose Save buttons are plain buttons labelled Save.
//! The password lives in the OS keychain and only ever goes into the sign-in form.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::chrome::Tab;
use super::MomQuestion;

pub const MOM: &str = "https://www.myopenmath.com";
const KEYRING_SERVICE: &str = "dad-myopenmath";

/// What the steps need from a browser tab (the real one is [`Tab`]; tests use a fake).
pub trait Page {
    fn navigate(&mut self, url: &str) -> Result<(), String>;
    fn eval(&mut self, js: &str) -> Result<Value, String>;
    fn wait_for(&mut self, cond: &str, limit: Duration) -> Result<bool, String>;
    fn url(&mut self) -> String;
}

impl Page for Tab {
    fn navigate(&mut self, url: &str) -> Result<(), String> { Tab::navigate(self, url) }
    fn eval(&mut self, js: &str) -> Result<Value, String> { Tab::eval(self, js) }
    fn wait_for(&mut self, cond: &str, limit: Duration) -> Result<bool, String> { Tab::wait_for(self, cond, limit) }
    fn url(&mut self) -> String { Tab::url(self) }
}

impl Page for super::livecdp::LiveTab {
    fn navigate(&mut self, url: &str) -> Result<(), String> { super::livecdp::LiveTab::navigate(self, url) }
    fn eval(&mut self, js: &str) -> Result<Value, String> { super::livecdp::LiveTab::eval(self, js) }
    fn wait_for(&mut self, cond: &str, limit: Duration) -> Result<bool, String> { super::livecdp::LiveTab::wait_for(self, cond, limit) }
    fn url(&mut self) -> String { super::livecdp::LiveTab::url(self) }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Course { pub id: u64, pub name: String }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Assessment { pub id: u64, pub name: String }

/// How an add ended.
#[derive(Debug, PartialEq)]
pub enum Added {
    Saved(Publication),
    SavedToSet(Publication, String),
    Filled,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Publication {
    pub course: u64,
    pub assessment: u64,
    pub question: u64,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum PublishState {
    ReadyToSave,
    AwaitingConfirmation,
    SavedToSet(Publication),
    Attached(Publication),
}

/// Set by the teacher's "Stop adding": the add stops at the last safe point, just before
/// MyOpenMath's Save is clicked. After that click the question stays, and the receipt says where.
pub static STOP_BEFORE_SAVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// How long a publish waits just before Save. With a saved login the whole add takes 1-2 s, so
/// without this pause a press of "Stop adding" nearly always arrives after Save. Long enough to read
/// the countdown and press Stop; short enough not to feel like a stall.
pub const SAVE_HOLD: Duration = Duration::from_millis(2500);

/// How often the hold looks for a press of Stop.
const HOLD_TICK: Duration = Duration::from_millis(50);

/// What the countdown line starts with; the window shows lines that start this way more boldly.
pub const HOLD_LINE: &str = "Adding it to MyOpenMath in ";

/// True for a progress line written by the pause before Save.
pub fn is_save_hold(line: &str) -> bool { line.starts_with(HOLD_LINE) }

/// Waits up to `hold` for Stop, saying how long is left once a second. True when Stop was pressed
/// (the press is consumed). A zero `hold` only checks once.
fn held_for_stop(hold: Duration, stop: &std::sync::atomic::AtomicBool, log: &mut impl FnMut(&str)) -> bool {
    use std::sync::atomic::Ordering::SeqCst;
    let start = std::time::Instant::now();
    let mut said = u64::MAX;
    loop {
        if stop.swap(false, SeqCst) { return true; }
        let left = hold.saturating_sub(start.elapsed());
        if left.is_zero() { return false; }
        let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
        if secs != said {
            said = secs;
            log(&format!("{HOLD_LINE}{secs}… Press Stop adding to cancel."));
        }
        std::thread::sleep(HOLD_TICK.min(left));
    }
}

/// Keep the state on disk before each remote change so a retry can reconcile the same question.
pub fn publish_question(
    p: &mut impl Page, cid: u64, aid: u64, q: &MomQuestion, key: &str, log: &mut impl FnMut(&str),
) -> Result<Added, String> {
    let receipt_path = crate::store::store_dir().join(key);
    let lock_path = receipt_path.with_extension("lock");
    let lock_dir = lock_path.parent().ok_or("Invalid publication record path.")?;
    let lock = (|| {
        std::fs::create_dir_all(lock_dir)?;
        std::fs::OpenOptions::new().write(true).create(true).truncate(false).open(&lock_path)
    })().map_err(|e| format!("Could not open the publication lock: {e}"))?;
    lock.try_lock().map_err(|e| format!("This publication could not be locked: {e}"))?;

    let record = |state| crate::store::save_json(key, &state)
        .map_err(|e| format!("Could not record publication status: {e}"));
    let save = |p: &mut _, log: &mut _| add_question_gated(p, cid, aid, q, true, log, &record, SAVE_HOLD, &STOP_BEFORE_SAVE);
    let result = match std::fs::read(&receipt_path) {
        Ok(bytes) => match serde_json::from_slice::<PublishState>(&bytes) {
            Ok(PublishState::Attached(publication)) if publication.course == cid && publication.assessment == aid => {
                return Ok(Added::Saved(publication));
            }
            Ok(PublishState::SavedToSet(publication)) if publication.course == cid && publication.assessment == aid => {
                resume_attachment(p, publication, log)
            }
            Ok(PublishState::ReadyToSave) => save(p, log)?,
            _ => return Err("The previous save could not be confirmed. Check your MyOpenMath question set before publishing again.".into()),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => save(p, log)?,
        Err(error) => return Err(format!("Could not read the previous publication status. Nothing was saved: {error}")),
    };
    if let Added::Saved(publication) = &result {
        record(PublishState::Attached(*publication))?;
    }
    Ok(result)
}

const SIGNED_IN: &str = "!!document.querySelector('a[href*=\"action=logout\"]')";

/// Whether this browser is signed in to MyOpenMath (opens the home page).
pub fn signed_in(p: &mut impl Page) -> Result<bool, String> {
    p.navigate(&format!("{MOM}/index.php"))?;
    Ok(p.eval(SIGNED_IN)? == Value::Bool(true))
}

/// Waits, without leaving the page, until the teacher has signed in in the window.
pub fn wait_signed_in(p: &mut impl Page, limit: Duration) -> Result<bool, String> {
    p.wait_for(SIGNED_IN, limit)
}

/// Types the saved username and password into MyOpenMath's sign-in form and submits it; true when
/// that signed in. The values are passed as JSON strings into the page and never logged.
pub fn sign_in_with(p: &mut impl Page, username: &str, password: &str) -> Result<bool, String> {
    p.navigate(&format!("{MOM}/index.php"))?;
    let js = format!(
        "(() => {{ const u = document.querySelector('input[name=username], #username'); \
           const w = document.querySelector('input[type=password]'); if (!u || !w) return false; \
           const set = (el, v) => {{ el.focus(); el.value = v; el.dispatchEvent(new Event('input', {{bubbles: true}})); }}; \
           set(u, {}); set(w, {}); const f = w.form; \
           const b = f && f.querySelector('input[type=submit], button[type=submit]'); if (b) b.click(); else if (f) f.submit(); return true; }})()",
        json!(username), json!(password));
    if p.eval(&js)? != Value::Bool(true) {
        return Ok(false);
    }
    p.wait_for(SIGNED_IN, Duration::from_secs(20))
}

/// The courses this teacher teaches, from the home page.
pub fn courses(p: &mut impl Page) -> Result<Vec<Course>, String> {
    p.navigate(&format!("{MOM}/index.php"))?;
    let v = p.eval(
        "(() => { const taught = new Set([...document.querySelectorAll('a[href*=\"transfercourse.php\"]')]\
           .map(a => (a.getAttribute('href').match(/[?&]id=(\\d+)/) || [])[1]).filter(Boolean)); const seen = new Set(); \
           return [...document.querySelectorAll('a[href*=\"course/course.php\"]')].map(a => { \
             const id = (a.getAttribute('href').match(/[?&]cid=(\\d+)/) || [])[1]; return {id, name: a.textContent.trim()}; }) \
           .filter(c => c.id && taught.has(c.id) && c.name && !seen.has(c.id) && seen.add(c.id)).map(c => ({id: +c.id, name: c.name})); })()")?;
    serde_json::from_value(v).map_err(|e| format!("Could not read the course list: {e}"))
}

/// A course's assessments, from its course page.
pub fn assessments(p: &mut impl Page, cid: u64) -> Result<Vec<Assessment>, String> {
    p.navigate(&format!("{MOM}/course/course.php?folder=0&cid={cid}"))?;
    let course = url::Url::parse(&p.url()).ok();
    if !course.is_some_and(|url| {
        url.origin().ascii_serialization() == MOM
            && url.path() == "/course/course.php"
            && number_param(&url, "cid") == Some(cid)
    }) {
        return Err("MyOpenMath did not open the selected course. Check that you are still signed in.".into());
    }
    let v = p.eval(
        "(() => { const seen = new Set(); return [...document.querySelectorAll('a[href*=\"assess2/?cid=\"]')].map(a => { \
           const id = (a.getAttribute('href').match(/[?&]aid=(\\d+)/) || [])[1]; return {id, name: a.textContent.trim()}; }) \
           .filter(x => x.id && x.name && !seen.has(x.id) && seen.add(x.id)).map(x => ({id: +x.id, name: x.name})); })()")?;
    serde_json::from_value(v).map_err(|e| format!("Could not read the assessments: {e}"))
}

/// The editor's fields for `q`, as the fill script takes them.
pub fn package(q: &MomQuestion) -> Value {
    json!({
        "description": q.name, "qtype": q.qtype.trim(), "control": q.common_control_field(),
        "qtext": q.qtext, "solution": q.detailed_solution_field(),
    })
}

/// Fills MyOpenMath's question editor from a package (the bookmarklet's logic): the type through
/// its own menu, each code box through its CodeMirror editor when there is one, and "Uses random
/// variables" when there is a worked solution. Returns what it could not find.
const FILL_JS: &str = "(q => { const missing = []; \
  const set = (id, v) => { const el = document.getElementById(id); if (!el) { missing.push(id); return; } \
    if (window.tinymce?.get(id)) { if (typeof window.toggleeditor !== 'function') { missing.push(id); return; } window.toggleeditor(id); if (window.tinymce.get(id)) { missing.push(id); return; } } \
    const w = el.nextElementSibling, cm = w && w.CodeMirror; \
    if (cm) { cm.setValue(v); if (cm.save) cm.save(); } \
    else { el.value = v; el.dispatchEvent(new Event('input', {bubbles: true})); el.dispatchEvent(new Event('change', {bubbles: true})); } \
    const normalize = s => s.replace(/\\r\\n/g, '\\n'); if (normalize(el.value) !== normalize(v)) missing.push(id); }; \
  const a = [...document.querySelectorAll('a[data-sn]')].find(el => el.dataset.sn === q.qtype); \
  if (a) a.click(); if (document.getElementById('qtype')?.value !== q.qtype) missing.push('qtype'); \
  set('description', q.description || ''); set('control', q.control || ''); set('qtext', q.qtext || ''); \
  if (q.solution) { set('solution', q.solution); const r = document.querySelector('input[name=\"usesrand\"]'); if (r && !r.checked) r.click(); if (!r?.checked) missing.push('usesrand'); } \
  return missing; })";

/// Opens the assessment's Add New Question editor, fills it, and saves it when `save`.
pub fn add_question(p: &mut impl Page, cid: u64, aid: u64, q: &MomQuestion, save: bool, log: &mut impl FnMut(&str)) -> Result<Added, String> {
    add_question_recorded(p, cid, aid, q, save, log, |_| Ok(()))
}

/// The in-app editor preview keeps Save disabled. Publishing opens a fresh editor.
pub fn preview_question(
    p: &mut impl Page, cid: u64, aid: u64, q: &MomQuestion, log: &mut impl FnMut(&str),
) -> Result<Added, String> {
    let filled = add_question(p, cid, aid, q, false, log)?;
    let locked = p.eval(&format!(r#"(() => {{
        const url = new URL(location.href);
        if (url.origin !== '{MOM}' || url.pathname !== '/course/moddataset.php'
            || url.searchParams.get('cid') !== '{cid}' || url.searchParams.get('aid') !== '{aid}') return false;
        const form = document.getElementById('control')?.form;
        if (!form) return false;
        form.addEventListener('submit', event => event.preventDefault(), true);
        for (const element of form.querySelectorAll('button, input[type=submit], input[type=image]')) {{
            if (element.type === 'submit' || element.type === 'image' ||
                element.textContent.trim() === 'Save' || element.classList.contains('quickSaveButton')) {{
                element.disabled = true;
            }}
        }}
        return true;
    }})()"#))?;
    if locked != Value::Bool(true) {
        return Err("The editor changed before D.A.D could prepare its preview. Return to transfer and try again.".into());
    }
    Ok(filled)
}

/// [`add_question_gated`] without the pause, for callers that have no Stop button to press.
pub fn add_question_recorded(
    p: &mut impl Page, cid: u64, aid: u64, q: &MomQuestion, save: bool,
    log: &mut impl FnMut(&str), record: impl FnMut(PublishState) -> Result<(), String>,
) -> Result<Added, String> {
    add_question_gated(p, cid, aid, q, save, log, record, Duration::ZERO, &STOP_BEFORE_SAVE)
}

/// Fills the editor and, when `save`, waits `hold` just before Save so the teacher can press Stop
/// (setting `stop`), then clicks Save. A Stop seen at any point up to the click saves nothing.
pub fn add_question_gated(
    p: &mut impl Page, cid: u64, aid: u64, q: &MomQuestion, save: bool,
    log: &mut impl FnMut(&str), mut record: impl FnMut(PublishState) -> Result<(), String>,
    hold: Duration, stop: &std::sync::atomic::AtomicBool,
) -> Result<Added, String> {
    if q.name.trim().is_empty() || q.name.chars().count() > 254 {
        return Err("Give this question a description of 1 to 254 characters before opening it in MyOpenMath.".into());
    }
    log("Opening the question editor in MyOpenMath…");
    p.navigate(&format!("{MOM}/course/moddataset.php?aid={aid}&cid={cid}&from=addq2"))?;
    if !p.wait_for("!!document.getElementById('control') && !!document.getElementById('qtext')", Duration::from_secs(20))? {
        return Err("MyOpenMath's question editor didn't open. Are you still signed in?".into());
    }
    if !matches_destination(&p.url(), "/course/moddataset.php", cid, aid) {
        return Err("MyOpenMath did not open the selected assessment's editor.".into());
    }
    log("Filling in the question…");
    let missing = p.eval(&format!("{FILL_JS}({})", package(q)))?;
    if !missing.as_array().is_some_and(Vec::is_empty) {
        return Err(format!("Some boxes weren't found in MyOpenMath's editor: {missing}"));
    }
    if !save {
        log("The editor is filled. Nothing was saved or added.");
        return Ok(Added::Filled);
    }
    if !matches_destination(&p.url(), "/course/moddataset.php", cid, aid) {
        return Err("The editor is no longer open for the selected assessment. Nothing was saved.".into());
    }
    // The live editor changes Save from type=submit to type=button when its script wires up form validation.
    let button = "[...(document.getElementById('control')?.form?.querySelectorAll('button') ?? [])].find(b => b.form === document.getElementById('control').form && b.textContent.trim() === 'Save' && !b.disabled && b.getClientRects().length > 0)";
    if p.eval(&format!("!!({button})"))? != Value::Bool(true) {
        return Err("Couldn't find MyOpenMath's Save button. Nothing was saved automatically.".into());
    }
    // The last safe point: the editor is filled and Save is found, nothing is saved yet.
    if held_for_stop(hold, stop, log) {
        return Err("Stopped before saving. Nothing was added to MyOpenMath.".into());
    }
    log("Saving to the MyOpenMath question set…");
    record(PublishState::AwaitingConfirmation)?;
    let clicked = p.eval(&format!(
        "(() => {{ const url = new URL(location.href); if (url.origin !== '{MOM}' || url.pathname !== '/course/moddataset.php' || url.searchParams.get('cid') !== '{cid}' || url.searchParams.get('aid') !== '{aid}') return false; const b = {button}; if (b) b.click(); return !!b; }})()"
    ))?;
    if clicked == Value::Bool(false) {
        record(PublishState::ReadyToSave)?;
        return Err("Save was not clicked. You can try publishing again.".into());
    }
    if clicked != Value::Bool(true) {
        return Err("MyOpenMath did not confirm the Save click. Check your question set before publishing again.".into());
    }
    if !p.wait_for(SAVED, Duration::from_secs(30))? {
        return Err("MyOpenMath did not confirm the save. Check your question set before publishing again.".into());
    }
    let publication = saved_publication(p)?;
    if publication.course != cid || publication.assessment != aid {
        return Err(format!("MyOpenMath returned a different destination for question {}. Nothing was attached.", publication.question));
    }
    record(PublishState::SavedToSet(publication))?;
    Ok(resume_attachment(p, publication, log))
}

const SAVED: &str = "!!document.querySelector('a[href*=\"usedef=true\"]')";

fn matches_destination(raw: &str, path: &str, course: u64, assessment: u64) -> bool {
    let Ok(url) = url::Url::parse(raw) else { return false };
    url.origin().ascii_serialization() == MOM && url.path() == path
        && number_param(&url, "cid") == Some(course) && number_param(&url, "aid") == Some(assessment)
}

fn number_param(url: &url::Url, key: &str) -> Option<u64> {
    let mut values = url.query_pairs().filter(|(name, _)| name == key);
    let value: u64 = values.next()?.1.parse().ok()?;
    (value > 0 && values.next().is_none()).then_some(value)
}

fn saved_publication(p: &mut impl Page) -> Result<Publication, String> {
    let href = p.eval("(() => { const a = document.querySelector('a[href*=\"usedef=true\"]'); return a ? a.href : null; })()")?;
    let url = href.as_str().and_then(|href| url::Url::parse(href).ok());
    let publication = url.as_ref().filter(|url| {
        url.origin().ascii_serialization() == MOM
            && url.path() == "/course/modquestion2.php"
            && url.query_pairs().any(|(key, value)| key == "usedef" && value == "true")
    }).and_then(|url| Some(Publication {
        course: number_param(url, "cid")?,
        assessment: number_param(url, "aid")?,
        question: number_param(url, "qsetid")?,
    }));
    publication.ok_or_else(|| "MyOpenMath did not return a valid saved question ID. Check your question set before publishing again.".into())
}

fn attachment_count(p: &mut impl Page, publication: Publication) -> Result<u64, String> {
    let Publication { course, assessment, question } = publication;
    let expected = "/course/addquestions2.php";
    if !matches_destination(&p.url(), expected, course, assessment) {
        return Err("MyOpenMath did not open the intended assessment's question list.".into());
    }
    // IMathAS addquestions2.php exposes the current assessment as itemarray. Question rows
    // hold the question-set ID at index 1. A grouped row holds its member rows at index 2.
    fn count(rows: &[Value], question: u64) -> Option<u64> {
        rows.iter().try_fold(0, |total, row| {
            if row[0] == "text" {
                return Some(total);
            }
            if let Some(children) = row[2].as_array() {
                return count(children, question).map(|n| total + n);
            }
            let _assessment_row_id = row[0].as_u64()?;
            let question_id = row[1].as_u64()?;
            let _description = row[2].as_str()?;
            let _kind = row[3].as_str()?;
            Some(total + u64::from(question_id == question))
        })
    }
    let rows = p.eval("Array.isArray(window.itemarray) ? window.itemarray : null")?;
    rows.as_array().and_then(|rows| count(rows, question))
        .ok_or_else(|| "MyOpenMath's assessment question list could not be read.".into())
}

/// Reconcile first so retrying an attachment reuses the same question-set ID.
pub fn resume_attachment(p: &mut impl Page, publication: Publication, log: &mut impl FnMut(&str)) -> Added {
    let Publication { course, assessment, question } = publication;
    let result = (|| {
        let list = format!("{MOM}/course/addquestions2.php?cid={course}&aid={assessment}");
        p.navigate(&list)?;
        match attachment_count(p, publication)? {
            1 => return Ok(()),
            0 => {}
            _ => return Err("The question appears more than once in the assessment. Check the question list.".into()),
        }
        log("Attaching the saved question to the assessment…");
        p.navigate(&format!("{MOM}/course/modquestion2.php?qsetid={question}&cid={course}&aid={assessment}&from=addq2&process=true&usedef=true"))?;
        if attachment_count(p, publication)? != 1 {
            return Err("MyOpenMath did not show exactly one copy in the assessment.".into());
        }
        Ok(())
    })();
    match result {
        Ok(()) => Added::Saved(publication),
        Err(error) => Added::SavedToSet(publication, error),
    }
}

/// Recover a save already visible in the browser without creating a second question.
pub fn attach_after_save(p: &mut impl Page, log: &mut impl FnMut(&str)) -> Result<Added, String> {
    let publication = saved_publication(p)?;
    Ok(resume_attachment(p, publication, log))
}

/// The saved MyOpenMath sign-in, if any: (username, password) from the OS keychain.
pub fn saved_login() -> Option<(String, String)> {
    let user = keyring::Entry::new(KEYRING_SERVICE, "username").ok()?.get_password().ok()?;
    let pass = keyring::Entry::new(KEYRING_SERVICE, "password").ok()?.get_password().ok()?;
    Some((user, pass))
}

/// Saves (Some) or forgets (None) the MyOpenMath sign-in in the OS keychain.
pub fn store_login(login: Option<(&str, &str)>) -> Result<(), String> {
    let entry = |k: &str| keyring::Entry::new(KEYRING_SERVICE, k).map_err(|e| format!("The keychain isn't available: {e}"));
    match login {
        Some((user, pass)) => {
            entry("username")?.set_password(user).map_err(|e| format!("Could not save to the keychain: {e}"))?;
            entry("password")?.set_password(pass).map_err(|e| format!("Could not save to the keychain: {e}"))
        }
        None => {
            for k in ["username", "password"] {
                match entry(k)?.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => {}
                    Err(e) => return Err(format!("Could not remove it from the keychain: {e}")),
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records navigations and answers scripts from a list of (substring of the script, answer).
    struct Fake { visited: Vec<String>, answers: Vec<(&'static str, Value)>, scripts: Vec<String>, at: String, lists: std::collections::VecDeque<Value>, wrong_landing: bool }
    impl Page for Fake {
        fn navigate(&mut self, url: &str) -> Result<(), String> { self.visited.push(url.into());
            self.at = if url.contains("usedef=true") {
                let aid = if self.wrong_landing { 3 } else { 2 };
                format!("{MOM}/course/addquestions2.php?cid=1&aid={aid}")
            } else { url.into() };
            Ok(()) }
        fn eval(&mut self, js: &str) -> Result<Value, String> {
            self.scripts.push(js.into());
            if js.contains("itemarray") { return Ok(self.lists.pop_front().unwrap_or(Value::Null)); }
            Ok(self.answers.iter().find(|(k, _)| js.contains(k)).map(|(_, v)| v.clone()).unwrap_or(Value::Null))
        }
        fn wait_for(&mut self, cond: &str, _: Duration) -> Result<bool, String> {
            // Saving lands on "Question Added to QuestionSet"; the "using Defaults" link then goes
            // to the assessment's list.
            if cond.contains("usedef") { self.at = format!("{MOM}/course/moddataset.php?process=true&id=9&frompot=1"); }
            else if cond.contains("addquestions") && self.at.contains("modquestion2") { self.at = format!("{MOM}/course/addquestions2.php?cid=1&aid=2"); }
            Ok(true)
        }
        fn url(&mut self) -> String { self.at.clone() }
    }
    fn fake(answers: Vec<(&'static str, Value)>) -> Fake { Fake { visited: vec![], answers, scripts: vec![], at: String::new(), lists: Default::default(), wrong_landing: false } }

    fn q() -> MomQuestion {
        MomQuestion { name: "Add one".into(), qtype: "number".into(), control: "$a = rand(1,9)\n$answer = $a+1".into(),
            qtext: "What is `$a+1`? $answerbox".into(), answer_code: "$solutionguide = 'add one'".into(), ..Default::default() }
    }

    const PUBLICATION: Publication = Publication { course: 1, assessment: 2, question: 9 };

    #[test]
    fn preview_disables_editor_submissions_without_saving() {
        let mut page = fake(vec![("missing", json!([])), ("event.preventDefault()", json!(true))]);
        let result = preview_question(&mut page, 1, 2, &q(), &mut |_| {}).unwrap();
        assert!(matches!(result, Added::Filled));
        assert_eq!(page.visited.len(), 1);
        assert!(page.scripts.iter().any(|script| script.contains("element.disabled = true")));
        assert!(!page.scripts.iter().any(|script| script.contains("b.click()")));
    }

    fn saved_link() -> String {
        format!("{MOM}/course/modquestion2.php?qsetid=9&cid=1&aid=2&from=addq2&process=true&usedef=true")
    }

    #[test]
    fn publishing_checks_the_exact_assessment_and_records_before_attachment() {
        let mut p = fake(vec![("missing", json!([])), ("=== 'Save'", json!(true)), ("a ? a.href", json!(saved_link()))]);
        p.lists.extend([json!([]), json!([[1, 0, [[25, 9, "Add one", "number"]], 1]])]);
        let mut journal = Vec::new();
        let r = add_question_recorded(&mut p, 1, 2, &q(), true, &mut |_| {}, |publication| {
            journal.push(publication);
            Ok(())
        });
        assert_eq!(r, Ok(Added::Saved(PUBLICATION)));
        assert_eq!(journal, vec![PublishState::AwaitingConfirmation, PublishState::SavedToSet(PUBLICATION)]);
        assert_eq!(p.visited.last(), Some(&saved_link()));
        assert!(p.scripts.iter().any(|s| s.contains("window.itemarray")));
    }

    use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

    fn ready_to_save() -> Fake {
        let mut p = fake(vec![("missing", json!([])), ("=== 'Save'", json!(true)), ("a ? a.href", json!(saved_link()))]);
        p.lists.extend([json!([]), json!([[1, 0, [[25, 9, "Add one", "number"]], 1]])]);
        p
    }

    fn clicked_save(p: &Fake) -> bool { p.scripts.iter().any(|s| s.contains("b.click()")) }

    #[test]
    fn the_hold_waits_before_save_and_counts_down_where_the_teacher_can_see_it() {
        let (stop, mut p, mut lines) = (AtomicBool::new(false), ready_to_save(), Vec::<String>::new());
        let began = std::time::Instant::now();
        let r = add_question_gated(&mut p, 1, 2, &q(), true, &mut |l| lines.push(l.into()), |_| Ok(()),
            Duration::from_millis(1200), &stop);
        assert_eq!(r, Ok(Added::Saved(PUBLICATION)));
        assert!(began.elapsed() >= Duration::from_millis(1200), "Save came before the hold ended");
        assert!(clicked_save(&p));
        let counts: Vec<&String> = lines.iter().filter(|l| is_save_hold(l)).collect();
        assert_eq!(counts.len(), 2, "one line per second left: {lines:?}");
        assert!(counts[0].contains("2…") && counts[1].contains("1…"));
        assert!(counts.iter().all(|l| l.contains("Stop adding")));
        // The countdown comes first and "Saving…" only once the hold is over.
        let saving = lines.iter().position(|l| l.starts_with("Saving")).unwrap();
        assert!(lines.iter().position(|l| is_save_hold(l)).unwrap() < saving);
        assert!(lines[saving..].iter().all(|l| !is_save_hold(l)));
    }

    #[test]
    fn stop_pressed_during_the_hold_saves_nothing() {
        let (stop, mut p, mut journal) = (AtomicBool::new(false), ready_to_save(), Vec::new());
        let began = std::time::Instant::now();
        let r = std::thread::scope(|s| {
            s.spawn(|| { std::thread::sleep(Duration::from_millis(300)); stop.store(true, SeqCst); });
            add_question_gated(&mut p, 1, 2, &q(), true, &mut |_| {}, |state| { journal.push(state); Ok(()) },
                Duration::from_secs(10), &stop)
        });
        assert_eq!(r, Err("Stopped before saving. Nothing was added to MyOpenMath.".into()));
        assert!(began.elapsed() < Duration::from_secs(5), "Stop did not end the hold");
        assert!(!clicked_save(&p), "Save was clicked after Stop");
        assert!(journal.is_empty(), "nothing is recorded as sent: {journal:?}");
        assert_eq!(p.visited.len(), 1, "no attachment page was opened");
        assert!(!stop.load(SeqCst), "the press is used up, so the next add is not stopped");
    }

    #[test]
    fn stop_pressed_before_the_hold_begins_still_lands() {
        let (stop, mut p) = (AtomicBool::new(true), ready_to_save());
        let r = add_question_gated(&mut p, 1, 2, &q(), true, &mut |_| {}, |_| Ok(()), SAVE_HOLD, &stop);
        assert!(r.unwrap_err().starts_with("Stopped before saving"));
        assert!(!clicked_save(&p));
    }

    #[test]
    fn without_a_press_the_hold_ends_and_publishes_with_the_finish_unchanged() {
        let (stop, mut p, mut journal) = (AtomicBool::new(false), ready_to_save(), Vec::new());
        let r = add_question_gated(&mut p, 1, 2, &q(), true, &mut |_| {}, |state| { journal.push(state); Ok(()) },
            Duration::from_millis(150), &stop);
        assert_eq!(r, Ok(Added::Saved(PUBLICATION)));
        assert_eq!(journal, vec![PublishState::AwaitingConfirmation, PublishState::SavedToSet(PUBLICATION)]);
        assert_eq!(p.visited.last(), Some(&saved_link()));
    }

    #[test]
    fn the_hold_is_long_enough_to_press_stop_but_not_a_stall() {
        assert!(SAVE_HOLD >= Duration::from_millis(2000) && SAVE_HOLD <= Duration::from_secs(4));
        let mut lines = Vec::<String>::new();
        assert!(!held_for_stop(Duration::ZERO, &AtomicBool::new(false), &mut |l| lines.push(l.into())));
        assert!(lines.is_empty(), "a zero hold says nothing");
    }

    #[test]
    fn preview_ends_after_filling_even_when_the_page_could_offer_an_attachment() {
        let mut p = fake(vec![("missing", json!([])), ("a ? a.href", json!(saved_link()))]);
        assert_eq!(add_question(&mut p, 1, 2, &q(), false, &mut |_| {}), Ok(Added::Filled));
        assert!(!p.scripts.iter().any(|s| s.contains("=== 'Save'")));
        assert_eq!(p.visited.len(), 1);
    }

    #[test]
    fn confirmed_no_click_allows_retry_but_an_uncertain_click_does_not() {
        for (reply, can_retry) in [(json!(false), true), (Value::Null, false)] {
            let mut p = fake(vec![("missing", json!([])), ("return !!b", reply), ("=== 'Save'", json!(true))]);
            let mut journal = Vec::new();
            assert!(add_question_recorded(&mut p, 1, 2, &q(), true, &mut |_| {}, |state| {
                journal.push(state);
                Ok(())
            }).is_err());
            assert_eq!(journal.first(), Some(&PublishState::AwaitingConfirmation));
            assert_eq!(journal.last() == Some(&PublishState::ReadyToSave), can_retry);
        }
    }

    #[test]
    fn missing_or_wrong_destination_after_attachment_is_unconfirmed() {
        for wrong_landing in [false, true] {
            let mut p = fake(vec![]);
            p.lists.extend([json!([]), json!([[26, 90, "Different question", "number"]])]);
            p.wrong_landing = wrong_landing;
            assert!(matches!(resume_attachment(&mut p, PUBLICATION, &mut |_| {}), Added::SavedToSet(PUBLICATION, _)));
        }
    }

    #[test]
    fn retry_does_not_attach_a_question_already_present_or_duplicated() {
        for (count, expected) in [(1, true), (2, false)] {
            let mut p = fake(vec![]);
            p.lists.push_back(json!(vec![json!([25, 9, "Add one", "number"]); count]));
            assert_eq!(matches!(resume_attachment(&mut p, PUBLICATION, &mut |_| {}), Added::Saved(PUBLICATION)), expected);
            assert_eq!(p.visited.len(), 1);
        }
    }

    #[test]
    fn different_save_destination_does_not_attach() {
        let link = saved_link().replace("&aid=2", "&aid=3");
        let mut p = fake(vec![("missing", json!([])), ("=== 'Save'", json!(true)), ("a ? a.href", json!(link))]);
        assert!(add_question(&mut p, 1, 2, &q(), true, &mut |_| {}).unwrap_err().contains("different destination"));
        assert_eq!(p.visited.len(), 1);
    }

    #[test]
    fn a_missing_box_is_reported() {
        let mut p = fake(vec![("missing", json!(["solution"]))]);
        let e = add_question(&mut p, 1, 2, &q(), true, &mut |_| {}).unwrap_err();
        assert!(e.contains("solution"));
    }

    #[test]
    fn lists_parse() {
        let mut p = fake(vec![("transfercourse", json!([{"id": 334437, "name": "Global Question Class"}]))]);
        assert_eq!(courses(&mut p).unwrap(), vec![Course { id: 334437, name: "Global Question Class".into() }]);
        let mut p = fake(vec![("assess2/?cid=", json!([{"id": 24473576, "name": "D.A.D test"}]))]);
        assert_eq!(assessments(&mut p, 334437).unwrap()[0].name, "D.A.D test");
        assert!(p.visited[0].ends_with("cid=334437"));
    }

    #[test]
    fn the_password_goes_in_as_a_json_string() {
        let mut p = fake(vec![("input[type=password]", json!(true)), ("logout", json!(true))]);
        assert!(sign_in_with(&mut p, "t\"eacher", "p'a\"ss").unwrap());
        let s = p.scripts.iter().find(|s| s.contains("input[type=password]")).unwrap();
        assert!(s.contains(r#""t\"eacher""#) && s.contains(r#""p'a\"ss""#));
    }
}
