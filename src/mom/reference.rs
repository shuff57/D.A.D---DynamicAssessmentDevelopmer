//! What a MyOpenMath (IMathAS) question may use, as data the writer can be given and can look up.
//!
//! `reference.json` is built by `tools/build_mom_reference.py` from IMathAS itself (its allowed
//! functions, libraries, reserved names and answer types, read from the engine and the pinned source)
//! and from `docs/dad/mom-reference` (MyOpenMath's help page). Three uses:
//!
//! - [`cheat_sheet`]: a short block for the system prompt, so the first draft already uses the right
//!   answer type codes and avoids the PHP functions IMathAS rejects.
//! - [`lookup`]: the writer's tools. A reply of `LOOKUP: answer_type numfunc`, `LOOKUP: macro diffrands`,
//!   `LOOKUP: check shuffle, normalcdf`, `LOOKUP: topic syntax` or `LOOKUP: search rounding money`
//!   lines is answered with the docs instead of being tested. Plain text, so any model can use it.
//! - [`preflight`]: the same facts checked against a question's code before the engine runs it:
//!   forbidden functions with what to use instead, a missing loadlibrary, a reserved variable name,
//!   an answer type IMathAS does not have.

use super::format::MomQuestion;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

#[derive(Deserialize, Default)]
pub struct Reference {
    pub allowed_functions: Vec<String>,
    pub libraries: BTreeMap<String, Vec<String>>,
    pub disallowed_vars: Vec<String>,
    pub answer_types: Vec<String>,
    #[serde(default)]
    pub answer_type_aliases: BTreeMap<String, String>,
    pub answer_type_docs: BTreeMap<String, TypeDoc>,
    pub macro_docs: BTreeMap<String, MacroDoc>,
    pub topics: BTreeMap<String, String>,
    pub php_traps: BTreeMap<String, Trap>,
}

#[derive(Deserialize, Default, Clone)]
pub struct TypeDoc { pub title: String, pub summary: String, pub doc: String }

#[derive(Deserialize, Default, Clone)]
pub struct MacroDoc { pub signature: String, pub summary: String, pub doc: String, pub library: Option<String> }

#[derive(Deserialize, Default, Clone)]
pub struct Trap { #[serde(rename = "use")] pub instead: Vec<String>, pub needs_library: Option<String> }

/// The reference built into the app.
pub fn reference() -> &'static Reference {
    static R: OnceLock<Reference> = OnceLock::new();
    R.get_or_init(|| serde_json::from_str(include_str!("reference.json")).expect("src/mom/reference.json is valid"))
}

/// Lookups answered per conversation before the writer must write.
pub const MAX_LOOKUPS: usize = 6;
/// The longest answer to one lookup line.
const LOOKUP_CHARS: usize = 3000;

/// The block for the system prompt.
pub fn cheat_sheet() -> String {
    let r = reference();
    let mut s = String::from("MYOPENMATH REFERENCE (from IMathAS itself; trust it over memory):\n\n");
    s.push_str("Answer type codes, exactly as written in $anstypes and SET QUESTION TYPE TO:\n");
    for t in &r.answer_types {
        let summary = r.answer_type_docs.get(t).map(|d| d.title.split(" (").next().unwrap_or("").to_string()).unwrap_or_default();
        s.push_str(&format!("- {t}{}\n", if summary.is_empty() { String::new() } else { format!(": {summary}") }));
    }
    s.push_str("- multipart: a question with several parts; set $anstypes to one code per part (only as the question type)\n");
    s.push_str("Never write these names, which MyOpenMath's help page uses but IMathAS rejects: ");
    s.push_str(&r.answer_type_aliases.iter().map(|(a, b)| format!("{a} (use {b})")).collect::<Vec<_>>().join(", "));
    s.push_str("\n\nPHP functions question code may NOT call, and what to use instead:\n");
    for (f, t) in &r.php_traps {
        let what = match (&t.needs_library, t.instead.is_empty()) {
            (Some(lib), _) => format!("allowed only after loadlibrary(\"{lib}\")"),
            (None, false) => format!("use {}", t.instead.join(" or ")),
            (None, true) => "no replacement; write it another way".into(),
        };
        s.push_str(&format!("- {f}: {what}\n"));
    }
    s.push_str("\nLibraries add functions: put loadlibrary(\"stats\") (or another) as the first line of the control. ");
    s.push_str("stats has normalcdf, invnormalcdf, tcdf, invtcdf, binomialpdf, mean, stdev, median, ...\n");
    s.push_str(&format!("\nReserved variable names (IMathAS rejects them): {}\n", r.disallowed_vars.iter().map(|v| format!("${v}")).collect::<Vec<_>>().join(", ")));
    s.push_str("\nTOOLS: before writing, you may reply with only lookup lines instead of a question, and the \
        docs come back. Lookups do not use up attempts. One per line, up to 3 per reply:\n\
        LOOKUP: answer_type <code>        the options and answer format of one answer type\n\
        LOOKUP: macro <name>              what a function does and its arguments\n\
        LOOKUP: check <name>, <name>      whether functions are allowed, and in which library\n\
        LOOKUP: topic <syntax|display|common-options|question-structure|dialect-rules>\n\
        LOOKUP: search <words>            find answer types and functions by keyword\n");
    s
}

/// Lookup lines in a reply, if the reply is a lookup request (and not a question file).
pub fn lookup_lines(reply: &str) -> Vec<String> {
    if reply.contains("// === ") {
        return Vec::new();
    }
    reply.lines()
        .filter_map(|l| l.trim().trim_start_matches(['-', '*', '`', ' ']).strip_prefix("LOOKUP:").map(|r| r.trim().trim_end_matches('`').to_string()))
        .filter(|l| !l.is_empty())
        .take(3)
        .collect()
}

fn cap(s: &str) -> String {
    if s.chars().count() <= LOOKUP_CHARS { s.to_string() } else { s.chars().take(LOOKUP_CHARS).collect::<String>() + "\n…(cut)" }
}

/// Answer one lookup line ("macro diffrands").
pub fn lookup(line: &str) -> String {
    let r = reference();
    let (tool, arg) = line.split_once(char::is_whitespace).map(|(a, b)| (a.trim(), b.trim())).unwrap_or((line.trim(), ""));
    match tool.to_ascii_lowercase().as_str() {
        "answer_type" | "type" | "anstype" => {
            let name = arg.trim_matches('`').to_ascii_lowercase();
            let code = r.answer_type_aliases.get(&name).cloned().unwrap_or(name.clone());
            match r.answer_type_docs.get(&code) {
                Some(d) => cap(&format!("{}{}: {}\n{}", if code != name { format!("\"{name}\" is not an IMathAS code; it is `{code}`.\n") } else { String::new() }, code, d.title, d.doc)),
                None if r.answer_types.contains(&code) => format!("`{code}` is an answer type IMathAS accepts; no further docs."),
                None => format!("`{name}` is not an IMathAS answer type. The codes are: {}.", r.answer_types.join(", ")),
            }
        }
        "macro" | "function" | "fn" => {
            let name = arg.trim_matches(['`', '(', ')']).to_string();
            match r.macro_docs.get(&name).or_else(|| r.macro_docs.iter().find(|(k, _)| k.eq_ignore_ascii_case(&name)).map(|(_, v)| v)) {
                Some(d) => cap(&format!("{}{}\n{}", d.signature, d.library.as_ref().map(|l| format!("  (needs loadlibrary(\"{l}\"))")).unwrap_or_default(), d.doc)),
                None => check(&[name.as_str()]),
            }
        }
        "check" => check(&arg.split([',', ' ']).map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>()),
        "topic" => match r.topics.get(arg) {
            Some(t) => cap(t),
            None => format!("No topic \"{arg}\". Topics: {}.", r.topics.keys().cloned().collect::<Vec<_>>().join(", ")),
        },
        "search" => search(arg),
        _ => format!("Unknown lookup \"{tool}\". Use answer_type, macro, check, topic or search."),
    }
}

/// Whether functions may be called, and from which library.
pub fn check(names: &[&str]) -> String {
    let r = reference();
    names.iter().map(|n| {
        if r.allowed_functions.iter().any(|f| f == n) {
            format!("{n}: allowed")
        } else if let Some((lib, _)) = r.libraries.iter().find(|(_, fns)| fns.iter().any(|f| f == n)) {
            format!("{n}: allowed after loadlibrary(\"{lib}\")")
        } else if let Some(t) = r.php_traps.get(*n) {
            format!("{n}: NOT allowed; {}", if t.instead.is_empty() { "write it another way".to_string() } else { format!("use {}", t.instead.join(" or ")) })
        } else {
            // Similar allowed names: ones that contain the name, or that it contains (4+ letters).
            let ln = n.to_ascii_lowercase();
            let close: Vec<&String> = r.allowed_functions.iter().chain(r.libraries.values().flatten())
                .filter(|f| { let lf = f.to_ascii_lowercase(); ln.len() >= 3 && lf.contains(&ln) || lf.len() >= 4 && ln.contains(&lf) })
                .take(5).collect();
            format!("{n}: NOT allowed{}", if close.is_empty() { String::new() } else { format!("; similar allowed names: {}", close.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) })
        }
    }).collect::<Vec<_>>().join("\n")
}

/// Answer types and functions whose name or description contains the words, best first.
pub fn search(words: &str) -> String {
    let r = reference();
    let words: Vec<String> = words.split_whitespace().map(|w| w.to_ascii_lowercase()).filter(|w| w.len() > 2).collect();
    if words.is_empty() { return "Give some words to search for.".into(); }
    let score = |name: &str, text: &str| -> usize {
        let (name, text) = (name.to_ascii_lowercase(), text.to_ascii_lowercase());
        words.iter().map(|w| if name.contains(w.as_str()) { 3 } else if text.contains(w.as_str()) { 1 } else { 0 }).sum()
    };
    let mut hits: Vec<(usize, String)> = Vec::new();
    for (k, d) in &r.answer_type_docs {
        let s = score(k, &format!("{} {}", d.title, d.summary));
        if s > 0 { hits.push((s + 1, format!("answer_type {k}: {}", d.title.split(" (").next().unwrap_or("")))); }
    }
    for (k, d) in &r.macro_docs {
        let s = score(k, &d.summary);
        if s > 0 { hits.push((s, format!("macro {}{}: {}", d.signature, d.library.as_ref().map(|l| format!(" [loadlibrary(\"{l}\")]")).unwrap_or_default(), d.summary))); }
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0));
    if hits.is_empty() { return format!("Nothing found for \"{}\".", words.join(" ")); }
    hits.into_iter().take(10).map(|(_, h)| h).collect::<Vec<_>>().join("\n")
}

/// Words that look like calls but are language: never reported as functions.
const KEYWORDS: &[&str] = &["if", "elseif", "else", "for", "foreach", "while", "where", "array", "isset", "empty", "list", "return", "and", "or", "not"];

/// Problems IMathAS's code checker would raise, found before the engine runs, each with a fix.
pub fn preflight(q: &MomQuestion) -> Vec<String> {
    let r = reference();
    let code = format!("{}\n{}", q.control, q.answer_code);
    let stripped = strip_strings_and_comments(&code);
    // Libraries the code loads.
    let mut loaded: BTreeSet<String> = BTreeSet::new();
    let lower = code.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(i) = rest.find("loadlibrary(") {
        let after = &rest[i + 12..];
        let arg: String = after.chars().take_while(|c| *c != ')').collect();
        for lib in arg.trim_matches(['"', '\'', ' ']).split(',') { loaded.insert(lib.trim().to_string()); }
        rest = &after[arg.len().min(after.len())..];
    }
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for name in called_functions(&stripped) {
        if KEYWORDS.contains(&name.as_str()) || !seen.insert(name.clone()) { continue; }
        if name == "loadlibrary" || name == "includecodefrom" || r.allowed_functions.iter().any(|f| *f == name) { continue; }
        if let Some((lib, _)) = r.libraries.iter().find(|(_, fns)| fns.iter().any(|f| *f == name)) {
            if !loaded.contains(&lib.to_ascii_lowercase()) {
                problems.push(format!("The code calls {name}, which needs loadlibrary(\"{lib}\") as the first line of the control."));
            }
            continue;
        }
        problems.push(match r.php_traps.get(&name) {
            Some(t) if !t.instead.is_empty() => format!("The code calls {name}, which MyOpenMath does not allow (\"unallowed macro\"); use {} instead.", t.instead.join(" or ")),
            _ => format!("The code calls {name}, which MyOpenMath does not allow (\"unallowed macro\"). {}", check(&[name.as_str()]).split_once("; ").map(|(_, b)| format!("{b}.")).unwrap_or_else(|| "Write it another way.".into())),
        });
    }
    // Reserved variable names.
    for v in &r.disallowed_vars {
        if code.contains(&format!("${v} ")) || code.contains(&format!("${v}=")) || code.contains(&format!("${v}[")) {
            problems.push(format!("${v} is a name IMathAS reserves (\"unallowed var\"); rename it, e.g. ${v}Value."));
        }
    }
    // Answer types.
    let mut types: Vec<String> = vec![q.qtype.trim().to_string()];
    if let Some(i) = code.find("$anstypes") {
        let line = code[i..].lines().next().unwrap_or("");
        types.extend(line.split(['"', '\'', ',', '(', ')', '=']).map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && s != "$anstypes" && s != "array"
                && !r.allowed_functions.iter().any(|f| f == s)));
    }
    for t in types {
        let t = t.to_ascii_lowercase();
        if t.is_empty() || t == "multipart" || r.answer_types.contains(&t) { continue; }
        problems.push(match r.answer_type_aliases.get(&t) {
            Some(code) => format!("\"{t}\" is not an IMathAS answer type; the code is {code}."),
            None => format!("\"{t}\" is not an IMathAS answer type. The codes are: {}.", r.answer_types.join(", ")),
        });
    }
    problems
}

/// Names followed by "(" that are not $variables or method calls.
fn called_functions(code: &str) -> Vec<String> {
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') { i += 1; }
            let prev = if start > 0 { b[start - 1] } else { b' ' };
            let mut j = i;
            while j < b.len() && b[j] == b' ' { j += 1; }
            if j < b.len() && b[j] == b'(' && prev != b'$' && prev != b'>' && prev != b':' && !(prev.is_ascii_alphanumeric()) {
                out.push(code[start..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The code with string literals and comments blanked (newlines kept), so words inside them are not
/// taken for calls. One scan over the whole text: IMathAS strings span lines (a $solutionguide of
/// several paragraphs is one string).
fn strip_strings_and_comments(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut chars = code.chars().peekable();
    let mut quote: Option<char> = None;
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == '\\' {
                out.push(' ');
                if let Some(n) = chars.next() { out.push(blank(n)); }
                continue;
            }
            if c == q { quote = None; }
            out.push(blank(c));
            continue;
        }
        match c {
            '"' | '\'' => { quote = Some(c); out.push(' '); }
            '/' if chars.peek() == Some(&'/') => { while let Some(&n) = chars.peek() { if n == '\n' { break; } chars.next(); } }
            '#' => { while let Some(&n) = chars.peek() { if n == '\n' { break; } chars.next(); } }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() { out.push(blank(n)); if prev == '*' && n == '/' { break; } prev = n; }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(control: &str) -> MomQuestion {
        MomQuestion { qtype: "multipart".into(), control: control.into(), qtext: "$answerbox[0]".into(), ..Default::default() }
    }

    #[test]
    fn the_reference_loads_and_knows_the_codes() {
        let r = reference();
        assert!(r.answer_types.iter().any(|t| t == "choices") && r.answer_types.iter().any(|t| t == "numfunc"));
        assert!(r.allowed_functions.iter().any(|f| f == "diffrands"));
        assert!(!r.allowed_functions.iter().any(|f| f == "shuffle"));
        assert!(r.libraries["stats"].iter().any(|f| f == "normalcdf"));
    }

    #[test]
    fn preflight_names_the_fix() {
        let p = preflight(&q("$a = shuffle(array(1,2,3))\n$b = normalcdf(1)\n$anstypes = array(\"number\", \"multiple_choice\")"));
        assert!(p.iter().any(|x| x.contains("shuffle") && x.contains("jointshuffle")), "{p:?}");
        assert!(p.iter().any(|x| x.contains("normalcdf") && x.contains("loadlibrary(\"stats\")")), "{p:?}");
        assert!(p.iter().any(|x| x.contains("multiple_choice") && x.contains("choices")), "{p:?}");
    }

    #[test]
    fn preflight_accepts_good_code() {
        let good = q("loadlibrary(\"stats\")\n$a,$b = diffrands(1,9,2)\n$p = normalcdf($a)\n// shuffle( in a comment\n$s = \"implode( in a string\"\n$g = '<p>A long guide (with words like part (a)\nand size (n)) over lines</p>'\n/* sort( in a block */\n$anstypes = array(\"number\",\"choices\")\nif ($a > 1) { $c = round($a, 2) }");
        assert!(preflight(&good).is_empty(), "{:?}", preflight(&good));
    }

    #[test]
    fn preflight_catches_a_reserved_name() {
        assert!(preflight(&q("$score = 3")).iter().any(|x| x.contains("$score")));
    }

    #[test]
    fn lookups_answer_from_the_docs() {
        assert!(lookup("answer_type multiple_choice").contains("`choices`"));
        assert!(lookup("macro diffrands").contains("diffrands"));
        assert!(lookup("check shuffle, sortarray, normalcdf").contains("normalcdf: allowed after loadlibrary(\"stats\")"));
        assert!(lookup("search random integers").contains("rand"));
        assert!(lookup("topic syntax").len() > 200);
        assert_eq!(lookup_lines("LOOKUP: macro rand\n- LOOKUP: answer_type numfunc\nthanks"), vec!["macro rand", "answer_type numfunc"]);
        assert!(lookup_lines("// === COMMON CONTROL ===\nLOOKUP: macro x").is_empty());
    }

    #[test]
    fn the_cheat_sheet_is_short_and_has_the_codes() {
        let s = cheat_sheet();
        assert!(s.contains("- choices") && s.contains("multiple_choice (use choices)") && s.contains("shuffle: use"), "{s}");
        assert!(s.len() < 6000, "{} chars", s.len());
    }
}

/// Runs preflight over a whole bank when `DAD_MOM_BANK` is set; questions the engine passes (per
/// engine/baseline.json) must come out clean, or preflight would mislead the writer.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod bank {
    #[test]
    #[ignore]
    fn passing_bank_questions_preflight_clean() {
        let Ok(root) = std::env::var("DAD_MOM_BANK") else { return };
        let base: std::collections::BTreeMap<String, bool> =
            serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/engine/baseline.json")).unwrap()).unwrap();
        let (mut flagged_pass, mut flagged_fail) = (Vec::new(), 0);
        for (path, passes) in &base {
            let Ok(src) = std::fs::read_to_string(std::path::Path::new(&root).join(path)) else { continue };
            let p = super::preflight(&super::MomQuestion::parse(&src));
            if p.is_empty() { continue }
            if *passes { flagged_pass.push(format!("{path}: {}", p.join(" | "))) } else { flagged_fail += 1 }
        }
        println!("preflight flags {} of the failing questions; {} passing questions flagged", flagged_fail, flagged_pass.len());
        for f in flagged_pass.iter().take(25) { println!("  {f}"); }
        assert!(flagged_pass.is_empty());
    }
}
