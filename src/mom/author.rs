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
use super::reference;
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
        let body = serde_json::to_value(SelftestRequest { src, seeds }).map_err(|e| e.to_string())?;
        let v = post_engine(&self.base, "/api/selftest", &body).await?;
        serde_json::from_value(v).map_err(|e| format!("The engine did not answer with a self-test ({e})"))
    }
}

/// The engine address that means "the engine built into this page" (browser build only).
pub const BUNDLED_ENGINE: &str = "bundled";

/// The engine built into the page: `window.dadEngine.post(route, body)`, a PHP-in-WebAssembly
/// worker loaded from engine/ (see engine/browser).
#[cfg(target_arch = "wasm32")]
async fn post_bundled(route: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
    use wasm_bindgen::JsCast as _;
    let window = web_sys::window().ok_or("no window")?;
    let engine = js_sys::Reflect::get(&window, &"dadEngine".into()).map_err(|_| "no dadEngine".to_string())?;
    if engine.is_undefined() {
        return Err("The built-in question engine is missing from this page (engine/dad-engine.js).".into());
    }
    let post: js_sys::Function = js_sys::Reflect::get(&engine, &"post".into())
        .ok().and_then(|f| f.dyn_into().ok()).ok_or("dadEngine.post is missing")?;
    let promise: js_sys::Promise = post.call2(&engine, &route.into(), &body.to_string().into())
        .map_err(|e| format!("{e:?}"))?.dyn_into().map_err(|_| "dadEngine.post returned no promise".to_string())?;
    let text = wasm_bindgen_futures::JsFuture::from(promise).await
        .map_err(|e| format!("The built-in question engine failed: {}", e.as_string().or_else(|| js_sys::Reflect::get(&e, &"message".into()).ok().and_then(|m| m.as_string())).unwrap_or_else(|| format!("{e:?}"))))?
        .as_string().unwrap_or_default();
    serde_json::from_str(&text).map_err(|e| format!("The built-in engine did not answer with JSON ({e})"))
}

/// Start downloading the built-in engine now (browser build), so it is ready by the first test.
pub fn warm_bundled_engine() {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast as _;
        let Some(window) = web_sys::window() else { return };
        let Ok(engine) = js_sys::Reflect::get(&window, &"dadEngine".into()) else { return };
        if let Some(warm) = js_sys::Reflect::get(&engine, &"warm".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()) {
            let _ = warm.call0(&engine);
        }
    }
}

/// One JSON POST to the engine: blocking HTTP on the desktop, fetch in the browser.
pub async fn post_engine(base: &str, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
    let url = format!("{}{path}", base.trim_end_matches('/'));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut resp = ureq::post(&url)
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|e| format!("Can't reach the question engine at {base}. Start it with engine/run.sh, or set its address in Settings. ({e})"))?;
        let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from the engine: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("The engine did not answer with JSON ({e})"))
    }
    #[cfg(target_arch = "wasm32")]
    {
        if base == BUNDLED_ENGINE {
            let _ = &url;
            return post_bundled(path, body).await;
        }
        crate::ai::post_json_browser(&url, body).await
            .map_err(|e| format!("Can't reach the question engine at {base}. ({e})"))
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
    /// Fresh conversations started after the first (see [`Budget::restarts`]).
    pub restarts: usize,
    /// Repair rounds asked for by the critic.
    pub critic_rounds: usize,
    /// The critic's last verdict: None when there is no critic or it never ran.
    pub critic_ok: Option<bool>,
    /// The critic's last notes, when it wanted changes.
    pub critic_notes: Vec<String>,
    /// Reference lookups the writer asked for (not tries).
    pub lookups: usize,
    /// Tries whose problems the pre-flight check found without running the engine.
    pub preflight_catches: usize,
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

/// The system prompt with the MyOpenMath reference (see [`system_prompt_with`]).
pub fn system_prompt() -> String {
    system_prompt_with(true)
}

/// The system prompt: what IMathAS is, the reply shape, optionally the reference cheat sheet and its
/// LOOKUP tools, and the learned rules.
pub fn system_prompt_with(use_docs: bool) -> String {
    let docs = if use_docs { format!("\n\n{}", reference::cheat_sheet()) } else { String::new() };
    format!(
        "You write questions for MyOpenMath, which runs IMathAS. A question is IMathAS question code: \
         a Common Control block (variables, randomization with rand/diffrands/randfrom, and the answer \
         in $answer or $answer[i]), and Question Text that shows the prompt with `backtick` math and \
         $answerbox placeholders. Multipart questions set $anstypes (one type per part) and index \
         $answer, $questions and $answerbox by part. Choices parts set $questions[i] to the options and \
         $answer[i] to the index of the right one.\n\n\
         Every question you write is rendered by a real IMathAS at several random seeds, its correct \
         answers are typed in like a student would, and any error or wrong score is sent back to you \
         to fix. Write questions that are correct at EVERY seed.\n\n{REPLY_SHAPE}{docs}\n\n\
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

/// How much effort one request may take.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// Questions the engine tests within one conversation.
    pub max_tries: usize,
    /// Fresh conversations after the first one fails (0: none). Small models dig in during a long
    /// repair thread; a new start, told what went wrong last time, often does better.
    pub restarts: usize,
    /// Give the writer the MyOpenMath reference: the cheat sheet in the prompt, LOOKUP tools, and the
    /// pre-flight check of its code against IMathAS's allowed functions (src/mom/reference.rs).
    pub use_docs: bool,
}

impl Budget {
    pub fn tries(max_tries: usize) -> Budget { Budget { max_tries, restarts: 0, use_docs: true } }
}

/// An optional reader after the engine passes (the harness's judge): `Ok(Some(notes))` sends the
/// notes back as one more repair round; `Ok(None)` accepts the question.
#[allow(async_fn_in_trait)]
pub trait AuthorCritic {
    async fn critique(&mut self, request: &str, q: &MomQuestion) -> Result<Option<Vec<String>>, String>;
}

/// No critic: the engine's verdict is final.
pub struct NoCritic;
impl AuthorCritic for NoCritic {
    async fn critique(&mut self, _: &str, _: &MomQuestion) -> Result<Option<Vec<String>>, String> { Ok(None) }
}

/// The control as the engine runs it: Common Control, then ANSWER code, one newline apart, so
/// IMathAS's "on line N of Common Control" points into it.
fn engine_control(q: &MomQuestion) -> String {
    if q.answer_code.trim().is_empty() { q.control.clone() } else { format!("{}\n{}", q.control, q.answer_code) }
}

/// IMathAS names a line ("syntax error ... on line 57 of Common Control") and the model has to count
/// to find it; quote that line and its neighbours under the problem instead.
pub fn with_code_context(problems: &[String], q: &MomQuestion) -> Vec<String> {
    let code = engine_control(q);
    let lines: Vec<&str> = code.lines().collect();
    problems.iter().map(|p| {
        let Some(at) = p.find("on line ") else { return p.clone() };
        let digits: String = p[at + 8..].chars().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(n) = digits.parse::<usize>() else { return p.clone() };
        if n == 0 || n > lines.len() || p[at..].contains("in file") || p[at..].contains(".php") { return p.clone(); }
        let mut out = p.clone();
        out.push_str("\n  The code around that line:");
        for i in n.saturating_sub(2)..(n + 1).min(lines.len()) {
            out.push_str(&format!("\n  {}{:>4} | {}", if i + 1 == n { ">" } else { " " }, i + 1, lines[i]));
        }
        out
    }).collect()
}

/// Write a question for `request`, test it, and repair it until it passes or `max_tries` runs out.
pub async fn author_async<C: AuthorChat, E: AuthorEngine>(request: &str, chat: &mut C, engine: &mut E, max_tries: usize) -> AuthorRun {
    author_async_logged(request, chat, engine, max_tries, |_| {}).await
}

/// [`author_async`], calling `on_log` with each log line as it happens (the screen's live progress).
pub async fn author_async_logged<C: AuthorChat, E: AuthorEngine>(
    request: &str, chat: &mut C, engine: &mut E, max_tries: usize, on_log: impl FnMut(&str),
) -> AuthorRun {
    author_full(request, chat, engine, &mut NoCritic, Budget::tries(max_tries), on_log).await
}

/// The whole loop: conversations of up to `budget.max_tries` engine tests each, up to
/// `budget.restarts` fresh ones after a failure, and the critic (if any) after the engine passes.
/// Returns the best attempt: passing beats failing, accepted by the critic beats not, then fewest
/// problems. `tries` counts every engine test across conversations.
pub async fn author_full<C: AuthorChat, E: AuthorEngine, K: AuthorCritic>(
    request: &str, chat: &mut C, engine: &mut E, critic: &mut K, budget: Budget, mut on_log: impl FnMut(&str),
) -> AuthorRun {
    let mut best: Option<AuthorRun> = None;
    let mut total_tries = 0;
    let mut lessons: Vec<String> = Vec::new();
    for round in 0..=budget.restarts {
        if round > 0 {
            on_log(&format!("starting over (fresh attempt {} of {})…", round + 1, budget.restarts + 1));
        }
        let ask = if lessons.is_empty() {
            request.to_string()
        } else {
            format!("{request}\n\nAn earlier attempt at this question failed IMathAS's test with:\n- {}\nWrite it fresh and avoid those mistakes.",
                lessons.iter().take(4).cloned().collect::<Vec<_>>().join("\n- "))
        };
        let mut run = conversation(&ask, request, chat, engine, critic, budget.max_tries, budget.use_docs, &mut on_log).await;
        total_tries += run.tries;
        run.restarts = round;
        let rank = |r: &AuthorRun| (r.passed, r.critic_ok != Some(false), usize::MAX - r.problems.len());
        let done = run.passed && run.critic_ok != Some(false) || run.error.is_some() && run.question.is_none();
        lessons = run.problems.iter().map(|p| p.lines().next().unwrap_or("").to_string()).collect();
        if best.as_ref().is_none_or(|b| rank(&run) > rank(b)) {
            if let Some(b) = best.take() { run.log.splice(0..0, b.log); }
            best = Some(run);
        } else if let Some(b) = best.as_mut() {
            b.log.extend(run.log);
        }
        if done { break; }
    }
    let mut best = best.unwrap_or_default();
    best.tries = total_tries;
    best
}

/// One conversation: write, test, repair, up to `max_tries` engine tests; then the critic.
async fn conversation<C: AuthorChat, E: AuthorEngine, K: AuthorCritic>(
    ask: &str, request: &str, chat: &mut C, engine: &mut E, critic: &mut K, max_tries: usize, use_docs: bool, on_log: &mut impl FnMut(&str),
) -> AuthorRun {
    let mut run = AuthorRun::default();
    let mut logged = 0;
    macro_rules! flush { () => { while logged < run.log.len() { on_log(&run.log[logged]); logged += 1; } } }
    on_log("writing the question…");
    let mut turns = vec![Turn::new("system", system_prompt_with(use_docs)), Turn::new("user", ask.to_string())];
    let mut format_retries = 0;
    while run.tries < max_tries.max(1) {
        // One retry: a provider or proxy hiccup (a gateway error page instead of JSON) is not the question's fault.
        let reply = match chat.chat(&turns).await {
            Ok(r) => Ok(r),
            Err(_) => { on_log("the AI did not answer; trying once more…"); chat.chat(&turns).await }
        };
        let reply = match reply {
            Ok(r) => r,
            Err(e) => {
                run.error = Some(format!("The AI did not answer: {e}"));
                run.log.push("the AI did not answer".into());
                flush!();
                return run;
            }
        };
        turns.push(Turn::new("assistant", reply.clone()));
        let asked = if use_docs { reference::lookup_lines(&reply) } else { Vec::new() };
        if !asked.is_empty() && run.lookups < reference::MAX_LOOKUPS {
            run.lookups += asked.len();
            run.log.push(format!("looked up: {}", asked.join("; ")));
            flush!();
            let docs: Vec<String> = asked.iter().map(|l| format!("LOOKUP: {l}\n{}", reference::lookup(l))).collect();
            let more = if run.lookups >= reference::MAX_LOOKUPS { "That was the last lookup; now write the question file." } else { "Look up more, or write the question file." };
            turns.push(Turn::new("user", format!("{}\n\n{more}", docs.join("\n\n"))));
            continue;
        }
        let q = match parse_reply(&reply) {
            Ok(q) => q,
            Err(e) if format_retries < FORMAT_RETRIES => {
                format_retries += 1;
                run.log.push(format!("unreadable reply, asked again ({e})"));
                turns.push(Turn::new("user", format!("{e}\n\n{REPLY_SHAPE}")));
                flush!();
                continue;
            }
            Err(e) => {
                run.error = Some(format!("The AI's replies could not be read as a question: {e}"));
                run.log.push("gave up: unreadable replies".into());
                flush!();
                return run;
            }
        };
        run.tries += 1;
        let attempt = run.tries;
        if use_docs {
            let caught = reference::preflight(&q);
            if !caught.is_empty() {
                // IMathAS's code checker would reject this; say why, with the fix, without running it.
                run.preflight_catches += 1;
                run.log.push(format!("try {attempt}: \"{}\" caught before testing: {} problem(s)", q.name, caught.len()));
                run.log.extend(caught.iter().map(|p| format!("    {p}")));
                run.passed = false;
                run.problems = caught.clone();
                run.question = Some(q);
                flush!();
                turns.push(Turn::new("user", format!(
                    "IMathAS's code checker would reject that question. Problems:\n- {}\n\nFix every one and reply with the whole \
                     question file again in the same format.", caught.join("\n- "))));
                continue;
            }
        }
        on_log(&format!("try {attempt}: testing \"{}\" at seeds {:?}…", q.name, DEFAULT_SEEDS));
        let report = match engine.selftest(&q.emit(), &DEFAULT_SEEDS).await {
            Ok(r) => r,
            Err(e) => {
                run.error = Some(e);
                run.question = Some(q);
                run.log.push(format!("try {attempt}: the engine could not be reached"));
                flush!();
                return run;
            }
        };
        // The engine checks never-set variables at run time, per seed; the static check is only
        // the offline fallback, so it is not repeated here.
        let mut problems = with_code_context(&report.problems(), &q);
        if use_docs {
            for p in problems.iter_mut() {
                if let Some(i) = p.find("unallowed macro ") {
                    let name: String = p[i + 16..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                    if !name.is_empty() { p.push_str(&format!("\n  {}", reference::check(&[name.as_str()]))); }
                }
            }
        }
        run.log.push(if problems.is_empty() {
            format!("try {attempt}: \"{}\" passes at seeds {:?}", q.name, DEFAULT_SEEDS)
        } else {
            format!("try {attempt}: \"{}\" has {} problem(s)", q.name, problems.len())
        });
        run.log.extend(problems.iter().map(|p| format!("    {}", p.lines().next().unwrap_or(""))));
        run.passed = report.pass && problems.is_empty();
        run.problems = problems.clone();
        run.report = Some(report);
        run.question = Some(q);
        flush!();
        let feedback = if run.passed {
            // The engine is satisfied; ask the critic, if there is one.
            match critic.critique(request, run.question.as_ref().unwrap()).await {
                Ok(None) => { if run.critic_rounds > 0 || run.critic_ok.is_some() { run.critic_ok = Some(true); } return run; }
                Ok(Some(notes)) if notes.is_empty() => { run.critic_ok = Some(true); return run; }
                Ok(Some(notes)) => {
                    run.critic_ok = Some(false);
                    run.critic_notes = notes.clone();
                    run.log.push(format!("try {attempt}: a reviewer wants changes: {}", notes.first().cloned().unwrap_or_default()));
                    flush!();
                    if run.tries >= max_tries.max(1) { return run; }
                    run.critic_rounds += 1;
                    format!(
                        "IMathAS tested that question and it works. A teacher reviewing it asked for changes:\n- {}\n\nMake those \
                         changes, keep it working, and reply with the whole question file again in the same format.",
                        notes.join("\n- "))
                }
                Err(e) => { run.log.push(format!("the reviewer did not answer ({e}); keeping the question")); flush!(); return run; }
            }
        } else {
            format!(
                "IMathAS tested that question. Problems:\n- {}\n\nFix every one and reply with the whole \
                 question file again in the same format.",
                problems.join("\n- "))
        };
        on_log("sending the problems back to the AI…");
        turns.push(Turn::new("user", feedback));
    }
    flush!();
    run
}

/// The desktop's blocking entry point.
pub fn author<C: AuthorChat, E: AuthorEngine>(request: &str, chat: &mut C, engine: &mut E, max_tries: usize) -> AuthorRun {
    block_on_ready(author_async(request, chat, engine, max_tries))
}

/// The blocking [`author_full`].
pub fn author_with<C: AuthorChat, E: AuthorEngine, K: AuthorCritic>(request: &str, chat: &mut C, engine: &mut E, critic: &mut K, budget: Budget) -> AuthorRun {
    block_on_ready(author_full(request, chat, engine, critic, budget, |_| {}))
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
    fn a_fresh_restart_is_told_what_went_wrong_and_can_pass() {
        // Two tries of BAD in the first conversation, then a fresh one that writes GOOD.
        struct Seen(Vec<String>, usize);
        impl AuthorChat for Seen {
            async fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
                self.0.push(turns[1].text.clone());
                self.1 += 1;
                Ok(if self.1 <= 2 { BAD } else { GOOD }.to_string())
            }
        }
        let mut chat = Seen(vec![], 0);
        let run = author_with("Add one", &mut chat, &mut FakeEngine, &mut NoCritic, Budget { max_tries: 2, restarts: 1, use_docs: false });
        assert!(run.passed, "{:?}", run.log);
        assert_eq!((run.tries, run.restarts), (3, 1));
        assert!(chat.0[2].contains("An earlier attempt at this question failed"), "{}", chat.0[2]);
        assert!(chat.0[2].contains("no correct answers exposed"));
    }

    #[test]
    fn without_restarts_a_failure_stays_a_failure() {
        let run = author_with("x", &mut Scripted(vec![BAD, BAD, GOOD], 0), &mut FakeEngine, &mut NoCritic, Budget { max_tries: 2, restarts: 0, use_docs: false });
        assert!(!run.passed);
        assert_eq!(run.tries, 2);
    }

    #[test]
    fn the_line_imathas_names_is_quoted() {
        let q = MomQuestion { control: "$a = 1\n$b = 2 +\n$c = 3".into(), answer_code: "$solutionguide = 'x'".into(), ..Default::default() };
        let out = with_code_context(&["Caught error: syntax error on line 2 of Common Control (seed 7)".into(), "no line here".into()], &q);
        assert!(out[0].contains(">   2 | $b = 2 +"), "{}", out[0]);
        assert!(out[0].contains("   1 | $a = 1") && out[0].contains("   3 | $c = 3"), "{}", out[0]);
        assert_eq!(out[1], "no line here");
        // Line 4 is the ANSWER code, which the engine runs right after the control.
        let out = with_code_context(&["syntax error on line 4 of Common Control".into()], &q);
        assert!(out[0].contains(">   4 | $solutionguide = 'x'"), "{}", out[0]);
        // An error inside IMathAS's own files is not the question's line.
        let out = with_code_context(&["Undefined index on line 2 in file /imathas/x.php".into()], &q);
        assert!(!out[0].contains("code around"));
    }

    #[test]
    fn a_critic_round_sends_notes_back_and_accepts_the_revision() {
        struct Picky(usize);
        impl AuthorCritic for Picky {
            async fn critique(&mut self, _: &str, _: &MomQuestion) -> Result<Option<Vec<String>>, String> {
                self.0 += 1;
                Ok(Some(if self.0 == 1 { vec!["Say what to round to.".into()] } else { vec![] }))
            }
        }
        struct Echo(Vec<String>);
        impl AuthorChat for Echo {
            async fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
                self.0.push(turns.last().unwrap().text.clone());
                Ok(GOOD.to_string())
            }
        }
        let mut chat = Echo(vec![]);
        let run = author_with("x", &mut chat, &mut FakeEngine, &mut Picky(0), Budget { max_tries: 4, restarts: 0, use_docs: false });
        assert!(run.passed);
        assert_eq!((run.tries, run.critic_rounds, run.critic_ok), (2, 1, Some(true)));
        assert!(chat.0[1].contains("Say what to round to."));
    }

    #[test]
    fn a_lookup_is_answered_without_spending_a_try() {
        struct Asks(Vec<String>, usize);
        impl AuthorChat for Asks {
            async fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
                self.1 += 1;
                self.0.push(turns.last().unwrap().text.clone());
                Ok(if self.1 == 1 { "LOOKUP: check shuffle\nLOOKUP: answer_type numfunc".to_string() } else { GOOD.to_string() })
            }
        }
        let mut chat = Asks(vec![], 0);
        let run = author_with("x", &mut chat, &mut FakeEngine, &mut NoCritic, Budget { max_tries: 1, restarts: 0, use_docs: true });
        assert!(run.passed, "{:?}", run.log);
        assert_eq!((run.tries, run.lookups), (1, 2));
        assert!(chat.0[1].contains("shuffle: NOT allowed") && chat.0[1].contains("numfunc"), "{}", chat.0[1]);
    }

    #[test]
    fn preflight_sends_the_fix_back_before_the_engine_runs() {
        const SHUFFLES: &str = "// === NAME - DESCRIPTION: Add one ===\n// === SET QUESTION TYPE TO: number ===\n// === COMMON CONTROL ===\n$a = shuffle(array(2,3))\n$answer = 1\n// === QUESTION TEXT ===\nx $answerbox\n";
        struct Seen(Vec<String>, usize);
        impl AuthorChat for Seen {
            async fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
                self.1 += 1;
                self.0.push(turns.last().unwrap().text.clone());
                Ok(if self.1 == 1 { SHUFFLES } else { GOOD }.to_string())
            }
        }
        struct NoEngineFor(&'static str);
        impl AuthorEngine for NoEngineFor {
            async fn selftest(&mut self, src: &str, seeds: &[i64]) -> Result<SelftestResponse, String> {
                assert!(!src.contains(self.0), "the engine ran a question preflight should have caught");
                FakeEngine.selftest(src, seeds).await
            }
        }
        let mut chat = Seen(vec![], 0);
        let run = author_with("x", &mut chat, &mut NoEngineFor("shuffle("), &mut NoCritic, Budget { max_tries: 3, restarts: 0, use_docs: true });
        assert!(run.passed, "{:?}", run.log);
        assert_eq!((run.tries, run.preflight_catches), (2, 1));
        assert!(chat.0[1].contains("jointshuffle"), "{}", chat.0[1]);
    }

    #[test]
    fn a_fenced_reply_is_read() {
        let q = parse_reply(BAD).unwrap();
        assert_eq!(q.name, "Add one");
        assert_eq!(q.qtext, "What is `$a+1`? $answerbox");
    }
}
