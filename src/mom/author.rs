//! The authoring loop: a teacher's request in, a MyOpenMath question that passes the engine out.
//!
//! The AI writes the question as JSON (the editor's fields), the engine renders it at several
//! seeds and types its correct answers like a student, the static checks look for what renders
//! "fine" and is still wrong, and every problem goes back to the AI with the seed it happened at.
//! Bounded: after `max_tries` the last attempt comes back with its problems listed, never hidden.
//!
//! The dialect rules are steve-desktop's `learned-rules.md`, each written after a real failure.

use super::engine::{SelftestResponse, DEFAULT_SEEDS};
use super::format::MomQuestion;
use super::health;
use crate::ai::Turn;
use crate::figure_agent::Chat;
use serde_json::Value;

/// Runs a question through the engine's self-test.
pub trait Engine {
    fn selftest(&mut self, src: &str, seeds: &[i64]) -> Result<SelftestResponse, String>;
}

/// The engine over HTTP (`engine/run.sh` serves it on 127.0.0.1:8791).
#[cfg(not(target_arch = "wasm32"))]
pub struct HttpEngine {
    pub base: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl Engine for HttpEngine {
    fn selftest(&mut self, src: &str, seeds: &[i64]) -> Result<SelftestResponse, String> {
        let url = format!("{}/api/selftest", self.base.trim_end_matches('/'));
        let body = serde_json::to_string(&super::engine::SelftestRequest { src, seeds }).map_err(|e| e.to_string())?;
        let mut resp = ureq::post(&url)
            .header("Content-Type", "application/json")
            .send(body)
            .map_err(|e| format!("Can't reach the engine at {url}: {e}"))?;
        let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from the engine: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("The engine did not answer with a self-test ({e})"))
    }
}

/// What a run produced.
#[derive(Debug, Default)]
pub struct AuthorRun {
    /// The last question the AI wrote (passing or not; check `passed`).
    pub question: Option<MomQuestion>,
    /// It passed the engine at every seed and the static checks found nothing.
    pub passed: bool,
    /// The engine's last report.
    pub report: Option<SelftestResponse>,
    /// Problems left on the last attempt (empty when `passed`).
    pub problems: Vec<String>,
    /// One line per step, for the chat panel and the headless hook.
    pub log: Vec<String>,
    pub tries: usize,
    /// Set when the run could not produce a question at all (AI or engine unreachable).
    pub error: Option<String>,
}

const REPLY_SHAPE: &str = r#"Reply with ONE JSON object and nothing else:
{"name": "<what the question assesses>",
 "qtype": "<IMathAS question type: number, numfunc, string, choices, multans, matching, calculated, interval, ntuple, matrix, essay, or multipart>",
 "control": "<Common Control code>",
 "qtext": "<Question Text, with $answerbox (or $answerbox[i] in multipart)>",
 "answer": "<code run after the control, e.g. $solutionguide = '...'; may be empty>"}"#;

/// The system prompt: what IMathAS is, the reply shape, and the learned rules.
pub fn system_prompt() -> String {
    format!(
        "You write questions for MyOpenMath, which runs IMathAS. A question is IMathAS question code: \
         a Common Control block (variables, randomization with rand/diffrands/randfrom, and the answer \
         in $answer or $answer[i]), and Question Text that shows the prompt with `backtick` math and \
         $answerbox placeholders. Multipart questions set $anstypes (one type per part) and index \
         $answer, $questions and $answerbox by part. Choices parts set $questions[i] to the options and \
         $answer[i] to the index of the right one.\n\n\
         Every question you write is rendered by a real IMathAS at several random seeds, its correct \
         answers are typed in like a student would, and any error or wrong score is sent back to you \
         to fix. Write questions that are correct at EVERY seed.\n\n{REPLY_SHAPE}\n\n\
         RULES for this dialect, each learned from a real failure:\n{}",
        include_str!("learned-rules.md")
    )
}

/// The question in the AI's JSON reply. Tolerates code fences and prose around the object.
pub fn parse_reply(reply: &str) -> Result<MomQuestion, String> {
    let start = reply.find('{').ok_or("The reply has no JSON object.")?;
    let end = reply.rfind('}').ok_or("The reply has no JSON object.")?;
    let v: Value = serde_json::from_str(&reply[start..=end]).map_err(|e| format!("The reply is not valid JSON: {e}"))?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let q = MomQuestion {
        name: s("name"),
        qtype: if s("qtype").is_empty() { "multipart".into() } else { s("qtype") },
        control: s("control"),
        qtext: s("qtext"),
        answer_code: s("answer"),
        solution: String::new(),
    };
    if q.control.is_empty() || q.qtext.is_empty() {
        return Err("The reply is missing \"control\" or \"qtext\".".into());
    }
    Ok(q)
}

/// Problems the static checks find. Only what a student would see: a variable the text prints but
/// nothing sets renders as a silent blank.
pub fn static_problems(q: &MomQuestion) -> Vec<String> {
    let undef = health::undefined_vars(q);
    if undef.is_empty() {
        return Vec::new();
    }
    vec![format!(
        "The question text prints {} but the control never sets {}; IMathAS shows nothing there.",
        undef.iter().map(|v| format!("${v}")).collect::<Vec<_>>().join(", "),
        if undef.len() == 1 { "it" } else { "them" }
    )]
}

/// Write a question for `request`, test it, and repair it until it passes or `max_tries` runs out.
pub fn author(request: &str, chat: &mut dyn Chat, engine: &mut dyn Engine, max_tries: usize) -> AuthorRun {
    let mut run = AuthorRun::default();
    let mut turns = vec![Turn::new("system", system_prompt()), Turn::new("user", request.to_string())];
    for attempt in 1..=max_tries.max(1) {
        run.tries = attempt;
        let reply = match chat.chat(&turns) {
            Ok(r) => r,
            Err(e) => {
                run.error = Some(format!("The AI did not answer: {e}"));
                run.log.push(format!("try {attempt}: the AI did not answer"));
                return run;
            }
        };
        turns.push(Turn::new("assistant", reply.clone()));
        let q = match parse_reply(&reply) {
            Ok(q) => q,
            Err(e) => {
                run.log.push(format!("try {attempt}: unreadable reply ({e})"));
                turns.push(Turn::new("user", format!("{e}\n\n{REPLY_SHAPE}")));
                continue;
            }
        };
        let src = q.emit();
        let report = match engine.selftest(&src, &DEFAULT_SEEDS) {
            Ok(r) => r,
            Err(e) => {
                run.error = Some(e);
                run.question = Some(q);
                run.log.push(format!("try {attempt}: the engine could not be reached"));
                return run;
            }
        };
        let mut problems = report.problems();
        problems.extend(static_problems(&q));
        run.log.push(if problems.is_empty() {
            format!("try {attempt}: \"{}\" passes at seeds {:?}", q.name, DEFAULT_SEEDS)
        } else {
            format!("try {attempt}: \"{}\" has {} problem(s)", q.name, problems.len())
        });
        run.log.extend(problems.iter().map(|p| format!("    {p}")));
        run.passed = report.pass && problems.is_empty();
        run.problems = problems.clone();
        run.report = Some(report);
        run.question = Some(q);
        if run.passed {
            return run;
        }
        turns.push(Turn::new(
            "user",
            format!(
                "IMathAS tested that question. Problems:\n- {}\n\nFix every one and reply with the whole \
                 question again in the same JSON shape.",
                problems.join("\n- ")
            ),
        ));
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mom::engine::{ScoreResponse, SelftestRun};

    struct Scripted(Vec<&'static str>, usize);
    impl Chat for Scripted {
        fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
            assert_eq!(turns[0].role, "system");
            self.1 += 1;
            Ok(self.0[self.1 - 1].to_string())
        }
    }

    /// Fails any question whose control lacks "$answer", like an engine that cannot score it.
    struct FakeEngine;
    impl Engine for FakeEngine {
        fn selftest(&mut self, src: &str, seeds: &[i64]) -> Result<SelftestResponse, String> {
            let ok = src.contains("$answer =");
            Ok(SelftestResponse {
                pass: ok,
                runs: seeds.iter().map(|&seed| SelftestRun {
                    seed, ok,
                    problems: if ok { vec![] } else { vec!["no correct answers exposed".into()] },
                    correct: ScoreResponse { total: if ok { 1.0 } else { 0.0 }, errors: vec![] },
                    ..Default::default()
                }).collect(),
                ..Default::default()
            })
        }
    }

    const BAD: &str = r#"```json
{"name": "Add one", "qtype": "number", "control": "$a = rand(2,9)", "qtext": "What is `$a+1`? $answerbox", "answer": ""}
```"#;
    const GOOD: &str = r#"{"name": "Add one", "qtype": "number", "control": "$a = rand(2,9)\n$answer = $a + 1", "qtext": "What is `$a+1`? $answerbox", "answer": ""}"#;

    #[test]
    fn repairs_until_the_engine_passes() {
        let mut chat = Scripted(vec![BAD, GOOD], 0);
        let run = author("A question about adding one", &mut chat, &mut FakeEngine, 4);
        assert!(run.passed, "{:?}", run.log);
        assert_eq!(run.tries, 2);
        assert_eq!(run.question.unwrap().control, "$a = rand(2,9)\n$answer = $a + 1");
    }

    #[test]
    fn gives_up_after_max_tries_with_the_problems_listed() {
        let mut chat = Scripted(vec![BAD, BAD], 0);
        let run = author("x", &mut chat, &mut FakeEngine, 2);
        assert!(!run.passed);
        assert_eq!(run.tries, 2);
        assert_eq!(run.problems, vec!["no correct answers exposed (seeds 4242, 7, 99)".to_string()]);
    }

    #[test]
    fn a_never_set_variable_is_a_problem_even_when_the_engine_passes() {
        let q = MomQuestion { control: "$answer = 2".into(), qtext: "`$m x` $answerbox".into(), ..Default::default() };
        assert_eq!(static_problems(&q).len(), 1);
    }

    #[test]
    fn reply_without_fields_is_rejected() {
        assert!(parse_reply(r#"{"name": "x"}"#).is_err());
        assert!(parse_reply("no json").is_err());
    }
}
