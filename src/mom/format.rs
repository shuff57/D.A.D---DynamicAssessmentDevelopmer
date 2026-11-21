//! The "mom" question file: one MyOpenMath (IMathAS) question as text, with section markers.
//!
//! ```text
//! // === NAME - DESCRIPTION: What the question assesses ===
//! // === SET QUESTION TYPE TO: multipart ===
//! // === COMMON CONTROL ===
//! $a = rand(2,9)
//! // === QUESTION TEXT ===
//! What is `$a + 1`? $answerbox
//! // === ANSWER ===
//! $solutionguide = '...'
//! ```
//!
//! Ported from steve-desktop (`src/integrations/mom/`, `mom-content/reference/transfer-rules.md`)
//! and kept in step with the engine's `engine/momformat.php`. Two layouts exist in the bank:
//! separate sections as above, and an older one where the whole IMathAS blob sits inside
//! `COMMON CONTROL (paste into Common Control)`, split by a `//question text` line and then a lone
//! `///` line before the solution. The `ANSWER` section is code when it looks like code (newer
//! files set `$solutionguide` there) and the worked solution otherwise (MyOpenMath's Detailed
//! Solution field). No I/O; builds for wasm32.

/// One question, in the fields MyOpenMath's question editor has.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MomQuestion {
    /// The NAME - DESCRIPTION line: what the question assesses.
    pub name: String,
    /// IMathAS question type: `multipart`, `number`, `choices`, `numfunc`, ...
    pub qtype: String,
    /// Common Control.
    pub control: String,
    /// Question Text.
    pub qtext: String,
    /// Code from the ANSWER section, run after the control (usually `$solutionguide = ...`).
    pub answer_code: String,
    /// A prose worked solution (MyOpenMath's Detailed Solution field).
    pub solution: String,
}

const MARK_OPEN: &str = "// === ";
const MARK_CLOSE: &str = " ===";

/// The text of a `// === X ===` marker line, or None.
fn marker(line: &str) -> Option<&str> {
    let t = line.trim_end();
    let inner = t.strip_prefix(MARK_OPEN)?.strip_suffix(MARK_CLOSE)?;
    Some(inner.trim())
}

impl MomQuestion {
    /// Parse a mom file. Never fails: a file with no markers is all control, as the engine reads it.
    pub fn parse(src: &str) -> MomQuestion {
        let mut q = MomQuestion { qtype: "multipart".into(), ..Default::default() };
        let mut sections: Vec<(String, String)> = Vec::new();
        for line in src.lines() {
            // Only the five real markers start a section; a comment banner in the same shape
            // ("// === Scenario picker ===") is content, or the code after it would be dropped.
            if let Some(m) = marker(line).filter(|m| is_section_marker(m) || sections.is_empty()) {
                sections.push((m.to_string(), String::new()));
            } else if let Some((_, body)) = sections.last_mut() {
                body.push_str(line);
                body.push('\n');
            }
        }
        let mut answer = String::new();
        for (head, body) in &sections {
            let body = body.trim().to_string();
            let upper = head.to_ascii_uppercase();
            if upper.starts_with("NAME - DESCRIPTION:") {
                q.name = head["NAME - DESCRIPTION:".len()..].trim().to_string();
            } else if let Some(i) = upper.find("SET QUESTION TYPE TO:") {
                q.qtype = head[i + "SET QUESTION TYPE TO:".len()..].trim().to_string();
            } else if upper.starts_with("COMMON CONTROL") && q.control.is_empty() {
                q.control = body;
            } else if upper == "QUESTION TEXT" {
                q.qtext = body;
            } else if upper == "ANSWER" {
                answer = body;
            }
        }
        if q.qtext.is_empty() {
            if let Some((control, after)) = split_on_line(&q.control, |l| l.trim().eq_ignore_ascii_case("//question text") || l.trim().eq_ignore_ascii_case("// question text")) {
                let (qtext, tail) = split_on_line(&after, |l| l.trim() == "///").unwrap_or((after.clone(), String::new()));
                q.control = control.trim_end().to_string();
                q.qtext = qtext.trim().to_string();
                if answer.is_empty() { answer = tail.trim().to_string(); }
            } else if let Some((control, qtext)) = split_on_line(&q.control, |l| l.trim() == "///") {
                q.control = control.trim_end().to_string();
                q.qtext = qtext.trim().to_string();
            }
        }
        if !answer.is_empty() {
            if looks_like_code(&answer) { q.answer_code = answer } else { q.solution = answer }
        }
        q
    }

    /// The standard layout. `parse(emit(q)) == q` for any question with no `// === ` lines inside
    /// its fields.
    pub fn emit(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("// === NAME - DESCRIPTION: {} ===\n", self.name.trim()));
        out.push_str(&format!("// === SET QUESTION TYPE TO: {} ===\n\n", self.qtype.trim()));
        out.push_str("// === COMMON CONTROL ===\n");
        out.push_str(self.control.trim());
        out.push_str("\n\n// === QUESTION TEXT ===\n");
        out.push_str(self.qtext.trim());
        out.push('\n');
        let answer = if !self.answer_code.trim().is_empty() { &self.answer_code } else { &self.solution };
        if !answer.trim().is_empty() {
            out.push_str("\n// === ANSWER ===\n");
            out.push_str(answer.trim());
            out.push('\n');
        }
        out
    }

    /// What goes in MyOpenMath's Common Control box: the control, then any ANSWER code.
    pub fn common_control_field(&self) -> String {
        if self.answer_code.trim().is_empty() {
            self.control.trim().to_string()
        } else {
            format!("{}\n\n{}", self.control.trim(), self.answer_code.trim())
        }
    }
}

/// The mom format's section markers; any other `// === X ===` line is a comment.
fn is_section_marker(head: &str) -> bool {
    let u = head.trim().to_ascii_uppercase();
    u.starts_with("NAME - DESCRIPTION:") || u.contains("SET QUESTION TYPE TO:")
        || u.starts_with("COMMON CONTROL") || u == "QUESTION TEXT" || u == "ANSWER"
}

/// Split at the first line matching `is_sep`, dropping that line.
fn split_on_line(s: &str, is_sep: impl Fn(&str) -> bool) -> Option<(String, String)> {
    let mut before = String::new();
    let mut lines = s.lines();
    for line in lines.by_ref() {
        if is_sep(line) {
            let after: Vec<&str> = lines.collect();
            return Some((before, after.join("\n")));
        }
        before.push_str(line);
        before.push('\n');
    }
    None
}

/// Whether an ANSWER section is code rather than prose: its first line that is not blank and not a
/// comment starts like a statement. Same rule as the engine's `dad_looks_like_code`.
pub fn looks_like_code(section: &str) -> bool {
    for line in section.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") { continue; }
        if t.starts_with('$') { return true; }
        for kw in ["if", "for", "foreach", "while", "loadlibrary", "includecodefrom"] {
            if let Some(rest) = t.strip_prefix(kw) {
                if rest.trim_start().starts_with('(') { return true; }
            }
        }
        // A bare macro call on its own line: name(...)
        let name_len = t.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
        if name_len > 0 && !t.as_bytes()[0].is_ascii_digit() {
            let rest = t[name_len..].trim_start();
            if rest.starts_with('(') && t.trim_end_matches(';').ends_with(')') { return true; }
        }
        return false;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const STANDARD: &str = "// === NAME - DESCRIPTION: Add one ===\n// === SET QUESTION TYPE TO: number ===\n\n// === COMMON CONTROL ===\n$a = rand(2,9)\n$answer = $a + 1\n\n// === QUESTION TEXT ===\nWhat is `$a + 1`? $answerbox\n\n// === ANSWER ===\n$solutionguide = 'Add one.'\n";

    #[test]
    fn standard_layout() {
        let q = MomQuestion::parse(STANDARD);
        assert_eq!(q.name, "Add one");
        assert_eq!(q.qtype, "number");
        assert_eq!(q.control, "$a = rand(2,9)\n$answer = $a + 1");
        assert_eq!(q.qtext, "What is `$a + 1`? $answerbox");
        assert_eq!(q.answer_code, "$solutionguide = 'Add one.'");
        assert!(q.solution.is_empty());
    }

    #[test]
    fn prose_answer_is_the_worked_solution() {
        let src = STANDARD.replace("$solutionguide = 'Add one.'", "The distributive property allows us to expand.");
        let q = MomQuestion::parse(&src);
        assert!(q.answer_code.is_empty());
        assert_eq!(q.solution, "The distributive property allows us to expand.");
        assert_eq!(q.common_control_field(), q.control);
    }

    #[test]
    fn older_paste_layout() {
        let src = "// === SET QUESTION TYPE TO: multipart ===\n// === COMMON CONTROL (paste into Common Control) ===\n$anstypes = \"essay\"\n//question text\n$questiontext\n$answerbox[0]\n///\n$rubricanswerbutton\n";
        let q = MomQuestion::parse(src);
        assert_eq!(q.control, "$anstypes = \"essay\"");
        assert_eq!(q.qtext, "$questiontext\n$answerbox[0]");
        assert_eq!(q.answer_code, "$rubricanswerbutton");
    }

    #[test]
    fn a_comment_banner_inside_the_code_is_not_a_section() {
        let src = STANDARD.replace("$a = rand(2,9)\n", "$a = rand(2,9)\n// === Scenario picker ===\n$label = \"x\"\n");
        let q = MomQuestion::parse(&src);
        assert_eq!(q.control, "$a = rand(2,9)\n// === Scenario picker ===\n$label = \"x\"\n$answer = $a + 1");
    }

    #[test]
    fn emit_round_trips() {
        let q = MomQuestion::parse(STANDARD);
        assert_eq!(MomQuestion::parse(&q.emit()), q);
    }

    #[test]
    fn code_detection() {
        assert!(looks_like_code("// note\n$x = 1"));
        assert!(looks_like_code("if ($a > 1) {"));
        assert!(looks_like_code("loadlibrary(\"stats\")"));
        assert!(!looks_like_code("So:\n<table>"));
        assert!(!looks_like_code("The answer is 4 (four)."));
    }
}

/// Runs over a whole bank when `DAD_MOM_BANK` points at a mom-content folder:
/// `DAD_MOM_BANK=~/…/mom-content cargo test --lib mom::format::bank -- --ignored --nocapture`
#[cfg(all(test, not(target_arch = "wasm32")))]
mod bank {
    use super::MomQuestion;

    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() { walk(&p, out) } else if p.extension().is_some_and(|x| x == "php") { out.push(p) }
        }
    }

    #[test]
    #[ignore]
    fn every_bank_question_round_trips() {
        let Ok(root) = std::env::var("DAD_MOM_BANK") else { return };
        let mut files = Vec::new();
        walk(&std::path::Path::new(&root).join("questions"), &mut files);
        let (mut bad, mut no_qtext, mut prose, mut undef) = (Vec::new(), 0, 0, 0);
        for f in &files {
            let src = std::fs::read_to_string(f).unwrap_or_default();
            let q = MomQuestion::parse(&src);
            if q.qtext.is_empty() { no_qtext += 1 }
            if !q.solution.is_empty() { prose += 1 }
            let u = super::super::health::undefined_vars(&q);
            if !u.is_empty() {
                undef += 1;
                println!("never set: {} {:?}", f.strip_prefix(&root).unwrap_or(f).display(), u);
            }
            if MomQuestion::parse(&q.emit()) != q { bad.push(f.display().to_string()) }
        }
        println!("{} files, {} without question text, {} with a prose solution, {} printing a never-set variable", files.len(), no_qtext, prose, undef);
        assert!(bad.is_empty(), "do not round-trip: {bad:#?}");
    }
}
