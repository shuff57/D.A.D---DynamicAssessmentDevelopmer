//! The authoring loop: a teacher's request in, a MyOpenMath question that passes the engine out.
//!
//! The AI writes the question as a mom file (the format in `format.rs`, plain text: code is never
//! escaped inside JSON, which cost whole tries when the model got the escaping wrong). The engine
//! renders it at several seeds, types its correct answers like a student and checks for what
//! renders "fine" and is still wrong (a printed variable the code never sets, duplicate options),
//! and every problem goes back to the AI with the seeds it happened at. Bounded: after `max_tries` the last attempt comes back with its problems listed.
//! A reply that is not a question file at all is asked for again without spending a try (at most
//! [`FORMAT_RETRIES`] times per run).
//!
//! One async loop serves both builds. The desktop drives it with [`block_on_ready`] (its chat and
//! engine calls are blocking, so every future is ready on first poll); the web build awaits fetch.
//! The dialect rules are steve-desktop's `learned-rules.md`, each written after a real failure.

use super::engine::{SelftestRequest, SelftestResponse, DEFAULT_SEEDS};
use super::format::MomQuestion;
use super::health;
use crate::ai::{AiCfg, Turn};

/// Unreadable replies asked for again, per run, without spending a try.
pub const FORMAT_RETRIES: usize = 2;

/// The model, as the loop sees it.
#[allow(async_fn_in_trait)]
pub trait AuthorChat {
    async fn chat(&mut self, turns: &[Turn]) -> Result<String, String>;
}

/// The engine's self-test, as the loop sees it.
#[allow(async_fn_in_trait)]
pub trait AuthorEngine {
    async fn selftest(&mut self, src: &str, seeds: &[i64]) -> Result<SelftestResponse, String>;
}

/// The configured provider, free-text replies.
pub struct ModelChat<'a> {
    pub cfg: &'a AiCfg,
}

impl AuthorChat for ModelChat<'_> {
    async fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
        #[cfg(not(target_arch = "wasm32"))]
        { crate::ai::chat_turns_text(self.cfg, turns, 0.2) }
        #[cfg(target_arch = "wasm32")]
        { crate::ai::chat_turns_async(self.cfg, turns, 0.2, false).await }
    }
}

/// The engine over HTTP (`engine/run.sh` serves it on 127.0.0.1:8791).
pub struct HttpEngine {
    pub base: String,
}

impl AuthorEngine for HttpEngine {
    async fn selftest(&mut self, src: &str, seeds: &[i64]) -> Result<SelftestResponse, String> {
        let url = format!("{}/api/selftest", self.base.trim_end_matches('/'));
        let body = serde_json::to_value(SelftestRequest { src, seeds }).map_err(|e| e.to_string())?;
        #[cfg(not(target_arch = "wasm32"))]
        let v: serde_json::Value = {
            let mut resp = ureq::post(&url)
                .header("Content-Type", "application/json")
                .send(body.to_string())
                .map_err(|e| format!("Can't reach the engine at {url}: {e}"))?;
            let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from the engine: {e}"))?;
            serde_json::from_str(&text).map_err(|e| format!("The engine did not answer with JSON ({e})"))?
        };
        #[cfg(target_arch = "wasm32")]
        let v = crate::ai::post_json_browser(&url, &body).await?;
        serde_json::from_value(v).map_err(|e| format!("The engine did not answer with a self-test ({e})"))
    }
}

/// What a run produced.
#[derive(Debug, Default)]
pub struct AuthorRun {
    /// The last question the AI wrote (passing or not; check `passed`).
    pub question: Option<MomQuestion>,
    /// It passed the engine's self-test at every seed.
    pub passed: bool,
    /// The engine's last report.
    pub report: Option<SelftestResponse>,
    /// Problems left on the last attempt (empty when `passed`).
    pub problems: Vec<String>,
    /// One line per step, for the chat panel and the headless hook.
    pub log: Vec<String>,
    /// Questions the engine tested.
    pub tries: usize,
    /// Set when the run could not produce a question at all (AI or engine unreachable).
    pub error: Option<String>,
}

const REPLY_SHAPE: &str = "Reply with the complete question file and nothing else, in exactly this format:

// === NAME - DESCRIPTION: <what the question assesses> ===
// === SET QUESTION TYPE TO: <number, numfunc, string, choices, multans, matching, calculated, interval, ntuple, matrix, essay, or multipart> ===

// === COMMON CONTROL ===
<the Common Control code>

// === QUESTION TEXT ===
<the Question Text, with $answerbox (or $answerbox[i] in multipart)>

// === ANSWER ===
<code run after the control, such as $solutionguide = '...'; may be left empty>";

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

/// The question in the AI's reply: a mom file, possibly inside a code fence or after a sentence.
pub fn parse_reply(reply: &str) -> Result<MomQuestion, String> {
    let start = reply.find("// === ").ok_or("The reply is not a question file: it has no `// === ` section markers.")?;
    let body: String = reply[start..]
        .lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n");
    let q = MomQuestion::parse(&body);
    if q.control.trim().is_empty() || q.qtext.trim().is_empty() {
        return Err("The reply is missing the COMMON CONTROL or QUESTION TEXT section.".into());
    }
    Ok(q)
}

/// Problems the static checks find without an engine (the editor, offline). Only what a student
/// would see: a variable the text prints but nothing sets renders as a silent blank. With an engine,
/// its run-time check reports the same thing per seed and is the one the loop uses.
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
pub async fn author_async<C: AuthorChat, E: AuthorEngine>(request: &str, chat: &mut C, engine: &mut E, max_tries: usize) -> AuthorRun {
    let mut run = AuthorRun::default();
    let mut turns = vec![Turn::new("system", system_prompt()), Turn::new("user", request.to_string())];
    let mut format_retries = 0;
    while run.tries < max_tries.max(1) {
        let reply = match chat.chat(&turns).await {
            Ok(r) => r,
            Err(e) => {
                run.error = Some(format!("The AI did not answer: {e}"));
                run.log.push("the AI did not answer".into());
                return run;
            }
        };
        turns.push(Turn::new("assistant", reply.clone()));
        let q = match parse_reply(&reply) {
            Ok(q) => q,
            Err(e) if format_retries < FORMAT_RETRIES => {
                format_retries += 1;
                run.log.push(format!("unreadable reply, asked again ({e})"));
                turns.push(Turn::new("user", format!("{e}\n\n{REPLY_SHAPE}")));
                continue;
            }
            Err(e) => {
                run.error = Some(format!("The AI's replies could not be read as a question: {e}"));
                run.log.push("gave up: unreadable replies".into());
                return run;
            }
        };
        run.tries += 1;
        let attempt = run.tries;
        let report = match engine.selftest(&q.emit(), &DEFAULT_SEEDS).await {
            Ok(r) => r,
            Err(e) => {
                run.error = Some(e);
                run.question = Some(q);
                run.log.push(format!("try {attempt}: the engine could not be reached"));
                return run;
            }
        };
        // The engine checks never-set variables at run time, per seed; the static check is only
        // the offline fallback, so it is not repeated here.
        let problems = report.problems();
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
                 question file again in the same format.",
                problems.join("\n- ")
            ),
        ));
    }
    run
}

/// The desktop's blocking entry point.
pub fn author<C: AuthorChat, E: AuthorEngine>(request: &str, chat: &mut C, engine: &mut E, max_tries: usize) -> AuthorRun {
    block_on_ready(author_async(request, chat, engine, max_tries))
}

/// Drive a future whose awaits never actually wait (blocking chat and engine calls). Not a general
/// executor: a future that returns Pending is polled again in a loop.
pub fn block_on_ready<F: std::future::Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mom::engine::{ScoreResponse, SelftestRun};

    struct Scripted(Vec<&'static str>, usize);
    impl AuthorChat for Scripted {
        async fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
            assert_eq!(turns[0].role, "system");
            self.1 += 1;
            Ok(self.0[self.1 - 1].to_string())
        }
    }

    /// Fails any question whose control does not set "$answer", like an engine that cannot score it.
    struct FakeEngine;
    impl AuthorEngine for FakeEngine {
        async fn selftest(&mut self, src: &str, seeds: &[i64]) -> Result<SelftestResponse, String> {
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

    const BAD: &str = "Here it is:\n```php\n// === NAME - DESCRIPTION: Add one ===\n// === SET QUESTION TYPE TO: number ===\n// === COMMON CONTROL ===\n$a = rand(2,9)\n// === QUESTION TEXT ===\nWhat is `$a+1`? $answerbox\n```";
    const GOOD: &str = "// === NAME - DESCRIPTION: Add one ===\n// === SET QUESTION TYPE TO: number ===\n// === COMMON CONTROL ===\n$a = rand(2,9)\n$answer = $a + 1\n// === QUESTION TEXT ===\nWhat is `$a+1`? $answerbox\n";

    #[test]
    fn repairs_until_the_engine_passes() {
        let run = author("A question about adding one", &mut Scripted(vec![BAD, GOOD], 0), &mut FakeEngine, 4);
        assert!(run.passed, "{:?}", run.log);
        assert_eq!(run.tries, 2);
        assert_eq!(run.question.unwrap().control, "$a = rand(2,9)\n$answer = $a + 1");
    }

    #[test]
    fn gives_up_after_max_tries_with_the_problems_listed() {
        let run = author("x", &mut Scripted(vec![BAD, BAD], 0), &mut FakeEngine, 2);
        assert!(!run.passed);
        assert_eq!(run.tries, 2);
        assert_eq!(run.problems, vec!["no correct answers exposed (seeds 4242, 7, 99)".to_string()]);
    }

    #[test]
    fn an_unreadable_reply_does_not_spend_a_try() {
        let run = author("x", &mut Scripted(vec!["Sure! What grade level?", GOOD], 0), &mut FakeEngine, 1);
        assert!(run.passed, "{:?}", run.log);
        assert_eq!(run.tries, 1);
    }

    #[test]
    fn unreadable_replies_are_bounded() {
        let run = author("x", &mut Scripted(vec!["no", "no", "no"], 0), &mut FakeEngine, 4);
        assert!(run.error.is_some());
        assert_eq!(run.tries, 0);
    }

    #[test]
    fn a_never_set_variable_is_a_problem_even_when_the_engine_passes() {
        let q = MomQuestion { control: "$answer = 2".into(), qtext: "`$m x` $answerbox".into(), ..Default::default() };
        assert_eq!(static_problems(&q).len(), 1);
    }

    #[test]
    fn a_fenced_reply_is_read() {
        let q = parse_reply(BAD).unwrap();
        assert_eq!(q.name, "Add one");
        assert_eq!(q.qtext, "What is `$a+1`? $answerbox");
    }
}
