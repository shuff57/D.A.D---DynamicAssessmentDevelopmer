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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Course { pub id: u64, pub name: String }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Assessment { pub id: u64, pub name: String }

/// How an add ended.
#[derive(Debug, PartialEq)]
pub enum Added {
    /// Saved; MyOpenMath is showing the assessment's question list.
    Saved,
    /// Filled in and waiting for the teacher to check it and press Save.
    Filled,
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
    const w = el.nextElementSibling, cm = w && w.CodeMirror; \
    if (cm) { cm.setValue(v); if (cm.save) cm.save(); } \
    else { el.value = v; el.dispatchEvent(new Event('input', {bubbles: true})); el.dispatchEvent(new Event('change', {bubbles: true})); } }; \
  const a = document.querySelector('a[data-sn=\"' + q.qtype + '\"]'); \
  if (a) a.click(); else { const h = document.getElementById('qtype'); if (h) h.value = q.qtype; else missing.push('qtype'); } \
  set('description', q.description || ''); set('control', q.control || ''); set('qtext', q.qtext || ''); \
  if (q.solution) { set('solution', q.solution); const r = document.querySelector('input[name=\"usesrand\"]'); if (r && !r.checked) r.click(); } \
  return missing; })";

/// Opens the assessment's Add New Question editor, fills it, and saves it when `save`.
pub fn add_question(p: &mut impl Page, cid: u64, aid: u64, q: &MomQuestion, save: bool, log: &mut impl FnMut(&str)) -> Result<Added, String> {
    log("Opening the question editor in MyOpenMath…");
    p.navigate(&format!("{MOM}/course/moddataset.php?aid={aid}&cid={cid}&from=addq2"))?;
    if !p.wait_for("!!document.getElementById('control') && !!document.getElementById('qtext')", Duration::from_secs(20))? {
        return Err("MyOpenMath's question editor didn't open. Are you still signed in?".into());
    }
    log("Filling in the question…");
    let missing = p.eval(&format!("{FILL_JS}({})", package(q)))?;
    if missing.as_array().is_some_and(|m| !m.is_empty()) {
        return Err(format!("Some boxes weren't found in MyOpenMath's editor: {missing}"));
    }
    if !save {
        // The teacher checks it and presses Save in the window; then it still has to be put in
        // the assessment, which D.A.D does as soon as it sees the save.
        log("It's filled in. Check it in the Chrome window and press Save there; D.A.D then puts it in the assessment.");
        if !p.wait_for(SAVED, Duration::from_secs(900))? {
            return Ok(Added::Filled);
        }
        return attach_after_save(p, log);
    }
    log("Saving…");
    let clicked = p.eval("(() => { const b = [...document.querySelectorAll('button')].find(b => b.textContent.trim() === 'Save'); if (b) b.click(); return !!b; })()")?;
    if clicked != Value::Bool(true) {
        return Err("Couldn't find MyOpenMath's Save button. The question is filled in; press Save yourself.".into());
    }
    if !p.wait_for(SAVED, Duration::from_secs(30))? {
        return Err("MyOpenMath didn't confirm the save. Check the browser window.".into());
    }
    attach_after_save(p, log)
}

/// After a save MyOpenMath shows "Question Added to QuestionSet" with "Add Question to Assessment
/// using Defaults" (course/moddataset.php, `frompot`), or is already back on the assessment's list.
const SAVED: &str = "!!document.querySelector('a[href*=\"usedef=true\"]') || location.href.indexOf('addquestions') !== -1";

/// Puts the just-saved question into the assessment (MyOpenMath's own "using Defaults" link).
pub fn attach_after_save(p: &mut impl Page, log: &mut impl FnMut(&str)) -> Result<Added, String> {
    if p.url().contains("addquestions") {
        return Ok(Added::Saved);
    }
    let href = p.eval("(() => { const a = document.querySelector('a[href*=\"usedef=true\"]'); return a ? a.href : null; })()")?;
    let Some(href) = href.as_str().map(str::to_string) else {
        return Err("The question was saved to your question set, but MyOpenMath didn't offer to add it to the assessment.".into());
    };
    log("Adding it to the assessment…");
    p.navigate(&href)?;
    if !p.wait_for("location.href.indexOf('addquestions') !== -1", Duration::from_secs(30))? {
        return Err("The question is saved in your question set, but adding it to the assessment didn't finish. Use \"Add Question to Assessment\" in the window.".into());
    }
    Ok(Added::Saved)
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
    struct Fake { visited: Vec<String>, answers: Vec<(&'static str, Value)>, scripts: Vec<String>, at: String }
    impl Page for Fake {
        fn navigate(&mut self, url: &str) -> Result<(), String> { self.visited.push(url.into()); self.at = url.into(); Ok(()) }
        fn eval(&mut self, js: &str) -> Result<Value, String> {
            self.scripts.push(js.into());
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
    fn fake(answers: Vec<(&'static str, Value)>) -> Fake { Fake { visited: vec![], answers, scripts: vec![], at: String::new() } }

    fn q() -> MomQuestion {
        MomQuestion { name: "Add one".into(), qtype: "number".into(), control: "$a = rand(1,9)\n$answer = $a+1".into(),
            qtext: "What is `$a+1`? $answerbox".into(), answer_code: "$solutionguide = 'add one'".into(), ..Default::default() }
    }

    #[test]
    fn saving_fills_then_presses_save() {
        let link = format!("{MOM}/course/modquestion2.php?qsetid=9&cid=1&aid=2&from=addq2&process=true&usedef=true");
        let mut p = fake(vec![("missing", json!([])), ("=== 'Save'", json!(true)), ("a ? a.href", json!(link.clone()))]);
        let mut log = vec![];
        let r = add_question(&mut p, 1, 2, &q(), true, &mut |l| log.push(l.to_string()));
        assert_eq!(r, Ok(Added::Saved));
        assert_eq!(p.visited, vec![format!("{MOM}/course/moddataset.php?aid=2&cid=1&from=addq2"), link]);
        let fill = p.scripts.iter().find(|s| s.contains("usesrand")).unwrap();
        assert!(fill.contains("\"solution\":\"$solutionguide\"") && fill.contains("\"qtype\":\"number\""), "{fill}");
        assert!(log.iter().any(|l| l.starts_with("Saving")));
    }

    #[test]
    fn without_save_it_waits_for_the_teachers_save_then_attaches() {
        let link = format!("{MOM}/course/modquestion2.php?qsetid=9&cid=1&aid=2&from=addq2&process=true&usedef=true");
        let mut p = fake(vec![("missing", json!([])), ("a ? a.href", json!(link.clone()))]);
        assert_eq!(add_question(&mut p, 1, 2, &q(), false, &mut |_| {}), Ok(Added::Saved));
        assert!(!p.scripts.iter().any(|s| s.contains("=== 'Save'")), "it never presses Save itself");
        assert_eq!(p.visited.last(), Some(&link));
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
