//! Static checks on a mom question, for what the engine cannot see.
//!
//! Ported from steve-desktop `src/integrations/mom/health.ts`. The engine catches anything that
//! errors or scores wrong; these catch what renders "fine" and is still wrong for a student:
//! a variable the question text prints but the control never sets (IMathAS silently prints
//! nothing), nested indexing that prints a literal `Array[1]`, and a marker left unsubstituted.

use super::format::MomQuestion;
use std::collections::BTreeSet;

/// Names IMathAS supplies, so their absence from the control is not a defect.
const BUILTINS: &[&str] = &["answerbox", "answer", "anstypes", "questions", "solutionguide", "ansprompt", "previewloc", "showanswerloc", "hintloc"];

/// Variables the question text prints that the control never assigns. They render as nothing,
/// with no diagnostic: the sentence just quietly loses a number.
pub fn undefined_vars(q: &MomQuestion) -> Vec<String> {
    let control = q.common_control_field();
    let mut assigned = BTreeSet::new();
    for line in control.lines() {
        // Every statement on the line: `$a = 1; $b = 2`, and the body of `if (...) { $x = ... }`.
        for stmt in line.split([';', '{', '}']) {
            // Left-hand sides, including the multi-assignment `$a,$b = diffrands(1,5,2)`.
            if let Some(eq) = find_assignment(stmt) {
                for name in var_names(&stmt[..eq]) { assigned.insert(name); }
            }
        }
        // foreach ($list as $item) / as $k => $v
        let mut rest = line;
        while let Some(i) = rest.find(" as ") {
            rest = &rest[i + 4..];
            for name in var_names(rest.split(')').next().unwrap_or("")) { assigned.insert(name); }
        }
        // for ($i=0..2)
        if let Some(i) = line.find("for (") {
            for name in var_names(line[i..].split('=').next().unwrap_or("")) { assigned.insert(name); }
        }
    }
    let mut missing = BTreeSet::new();
    for name in var_names(&q.qtext) {
        if !assigned.contains(&name) && !BUILTINS.contains(&name.as_str()) { missing.insert(name); }
    }
    missing.into_iter().collect()
}

/// End of the left-hand side of an assignment on this line (`=`, or a compound `.=` / `+=` ...),
/// if there is one and that side is only variables, indexes and commas. `==`, `<=`, `>=`, `!=`
/// and `=>` are not assignments.
fn find_assignment(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    for i in 0..b.len() {
        if b[i] != b'=' { continue; }
        let prev = if i > 0 { b[i - 1] } else { b' ' };
        let next = b.get(i + 1).copied().unwrap_or(b' ');
        if matches!(prev, b'=' | b'<' | b'>' | b'!') || next == b'=' || next == b'>' { continue; }
        let end = if matches!(prev, b'.' | b'+' | b'-' | b'*' | b'/') { i - 1 } else { i };
        let lhs = line[..end].trim();
        let ok = lhs.starts_with('$')
            && lhs.chars().all(|c| c.is_ascii_alphanumeric() || "$_,[] '\"".contains(c));
        return ok.then_some(end);
    }
    None
}

/// Every `$name` in the text, without the `$`.
fn var_names(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' && i + 1 < b.len() && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_') {
            let start = i + 1;
            let mut j = start;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') { j += 1; }
            out.push(s[start..j].to_string());
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Problems visible in rendered question HTML that the engine does not report as errors.
pub fn render_problems(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    if html.contains("Array[") {
        found.push("Nested array indexing in the question text prints a literal \"Array[…]\". Precompute a plain variable in the control.".to_string());
    }
    for marker in ["$answerbox", "$answer[", "$solutionguide"] {
        if html.contains(marker) {
            found.push(format!("\"{marker}\" was printed literally instead of being substituted."));
            break;
        }
    }
    let text = strip_tags(html);
    if !html.trim().is_empty() && text.trim().is_empty() {
        found.push("The question renders no text at all.".to_string());
    }
    found
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let lower = html.to_ascii_lowercase();
    let mut skip_until: Option<&str> = None;
    let mut i = 0;
    while i < html.len() {
        if let Some(end) = skip_until {
            match lower[i..].find(end) { Some(j) => { i += j + end.len(); skip_until = None; continue } None => break }
        }
        if lower[i..].starts_with("<script") { skip_until = Some("</script>"); continue; }
        if lower[i..].starts_with("<style") { skip_until = Some("</style>"); continue; }
        let c = html[i..].chars().next().unwrap();
        if c == '<' { in_tag = true } else if c == '>' { in_tag = false } else if !in_tag { out.push(c) }
        i += c.len_utf8();
    }
    out.replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(control: &str, qtext: &str) -> MomQuestion {
        MomQuestion { qtype: "number".into(), control: control.into(), qtext: qtext.into(), ..Default::default() }
    }

    #[test]
    fn finds_a_variable_the_text_prints_but_never_sets() {
        assert_eq!(undefined_vars(&q("$m = 2", "`y = $m x + $b`")), vec!["b"]);
    }

    #[test]
    fn multi_assignment_and_loops_count_as_set() {
        assert!(undefined_vars(&q("$a,$b = diffrands(1,5,2)\nforeach ($xs as $x) {", "$a $b $x $answerbox")).is_empty());
        assert!(undefined_vars(&q("for ($i=0..2) {\n$t[$i] = $i\n}", "$t $answerbox[0]")).is_empty());
    }

    #[test]
    fn assignments_inside_a_branch_or_after_a_semicolon_count() {
        assert!(undefined_vars(&q("if ($k > 0) { $shown = \"x\" }\n$xa = 1; $ya = 2;", "$shown $xa $ya")).is_empty());
        assert!(undefined_vars(&q("}  $im[$i] = 3", "$im[0]")).is_empty());
    }

    #[test]
    fn imathas_placement_markers_are_builtins() {
        assert!(undefined_vars(&q("$answer = 1", "$answerbox $previewloc $hintloc $showanswerloc")).is_empty());
    }

    #[test]
    fn comparisons_are_not_assignments() {
        assert_eq!(undefined_vars(&q("if ($a == 2) {", "$a")), vec!["a"]);
    }

    #[test]
    fn render_problems_spots_literal_arrays_and_markers() {
        assert_eq!(render_problems("<p>Pick Array[1]</p>").len(), 1);
        assert_eq!(render_problems("<p>Type $answerbox here</p>").len(), 1);
        assert_eq!(render_problems("<div><script>x</script></div>"), vec!["The question renders no text at all."]);
        assert!(render_problems("<p>Fine</p>").is_empty());
    }
}
