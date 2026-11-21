//! The question-writing harness: run the authoring loop over a fixed set of teacher requests on
//! several models, judge what comes out, and compare.
//!
//! ```text
//! DAD_EVAL=harness/evals.json DAD_EVAL_MODELS=harness/models.json DAD_EVAL_OUT=harness/runs/<name> \
//!     [DAD_EVAL_ONLY=bank-01,edge-matrix] [DAD_EVAL_JOBS=3] [DAD_TRIES=4] vershion
//! ```
//!
//! Each case and model is one authoring run (engine self-test at three seeds, repaired up to the
//! try limit). A question that passes is then shown to the judge model with the teacher's request:
//! does it ask what was asked, is it clearly worded, is the math right. Results are appended to
//! `results.jsonl` as they finish, so a stopped run resumes where it left off, and `report.md`
//! compares the models. Desktop only (threads and blocking HTTP).

#![cfg(not(target_arch = "wasm32"))]

use super::author::{author_with, AuthorChat, AuthorCritic, Budget, HttpEngine, ModelChat, NoCritic};
use super::format::MomQuestion;
use crate::ai::{AiCfg, Turn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::io::Write as _;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Deserialize, Clone, Debug)]
pub struct EvalCase {
    pub id: String,
    /// "bank" (from a real question's description) or "edge" (hand-written).
    pub source: String,
    pub request: String,
    /// The bank question the request came from, for comparing by hand.
    #[serde(default)]
    pub r#ref: Option<String>,
}

#[derive(Deserialize)]
struct EvalFile {
    cases: Vec<EvalCase>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ModelSpec {
    pub label: String,
    pub provider: String,
    pub base: String,
    pub model: String,
    /// "off", "low", "medium" or "high"; absent leaves the model's default.
    #[serde(default)]
    pub think: Option<String>,
    /// Offer the reference as a native function tool instead of the text LOOKUP protocol.
    #[serde(default)]
    pub tools: bool,
}

#[derive(Deserialize)]
struct ModelsFile {
    models: Vec<ModelSpec>,
    #[serde(default)]
    judge: Option<String>,
}

impl ModelSpec {
    pub fn cfg(&self) -> AiCfg {
        AiCfg { provider: self.provider.clone(), base: self.base.trim_end_matches('/').to_string(), model: self.model.clone(), think: self.think.clone() }
    }
}

/// The judge's reading of one question.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Judgement {
    /// It asks what the teacher asked for.
    pub matches_request: bool,
    /// A student could read it without guessing what is meant.
    pub clear: bool,
    /// The math and the answer key are right, as far as the judge can tell from the code.
    pub math_correct: bool,
    /// 1 (unusable) to 5 (ready to assign).
    pub score: u8,
    pub notes: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CaseResult {
    pub case: String,
    pub source: String,
    pub model: String,
    pub passed: bool,
    pub tries: usize,
    pub secs: f64,
    pub problems: Vec<String>,
    pub error: Option<String>,
    pub judge: Option<Judgement>,
    pub judge_error: Option<String>,
    pub question: Option<String>,
    pub log: Vec<String>,
    #[serde(default)]
    pub restarts: usize,
    #[serde(default)]
    pub critic_rounds: usize,
    #[serde(default)]
    pub lookups: usize,
    #[serde(default)]
    pub preflight_catches: usize,
}

/// The judge as an in-loop critic: a passing question scoring below `threshold` goes back to the
/// writer with the judge's notes. Its last reading is kept, so the result needs no second call.
pub struct JudgeCritic<'a> {
    pub cfg: &'a AiCfg,
    pub threshold: u8,
    pub last: Option<Judgement>,
}

impl AuthorCritic for JudgeCritic<'_> {
    async fn critique(&mut self, request: &str, q: &MomQuestion) -> Result<Option<Vec<String>>, String> {
        let j = judge(self.cfg, request, q)?;
        let ok = j.score >= self.threshold;
        let mut notes = Vec::new();
        if !ok {
            if !j.matches_request { notes.push("It does not ask what the teacher asked for.".to_string()); }
            if !j.clear { notes.push("A student would not know exactly what to enter.".to_string()); }
            if !j.math_correct { notes.push("The answer the code computes is not right for the question as worded, at every seed.".to_string()); }
            notes.push(j.notes.clone());
        }
        self.last = Some(j);
        Ok(Some(notes))
    }
}

const JUDGE_PROMPT: &str = "You review MyOpenMath (IMathAS) questions written for a teacher. You get the teacher's \
request and the question file (Common Control code, Question Text, and solution code). The question already \
renders and scores correctly at several random seeds; your job is what a machine cannot check. Reply with ONE JSON \
object: {\"matches_request\": true|false, \"clear\": true|false, \"math_correct\": true|false, \"score\": 1-5, \
\"notes\": \"<one or two sentences: the most important problem, or why it is good>\"}. \
matches_request: it asks what the teacher asked for (topic, parts, answer types, constraints). clear: a student \
would know exactly what to enter. math_correct: the answer the code computes is the right answer to the question \
as worded, at every seed. score: 5 ready to assign, 4 minor polish, 3 needs an edit, 2 major problem, 1 unusable. \
House conventions, not defects: the worked solution goes in $solutionguide (this teacher's course shows it), and \
the ANSWER section holds code that runs after the control.";

/// Ask the judge about one question.
pub fn judge(cfg: &AiCfg, request: &str, q: &MomQuestion) -> Result<Judgement, String> {
    let turns = [
        Turn::new("system", JUDGE_PROMPT),
        Turn::new("user", format!("TEACHER'S REQUEST:\n{request}\n\nQUESTION FILE:\n{}", q.emit())),
    ];
    let reply = crate::ai::chat_turns(cfg, &turns, 0.0)?;
    let start = reply.find('{').ok_or("the judge gave no JSON")?;
    let end = reply.rfind('}').ok_or("the judge gave no JSON")?;
    let v: Value = serde_json::from_str(&reply[start..=end]).map_err(|e| format!("judge JSON: {e}"))?;
    Ok(Judgement {
        matches_request: v.get("matches_request").and_then(Value::as_bool).unwrap_or(false),
        clear: v.get("clear").and_then(Value::as_bool).unwrap_or(false),
        math_correct: v.get("math_correct").and_then(Value::as_bool).unwrap_or(false),
        score: v.get("score").and_then(Value::as_u64).unwrap_or(0).min(5) as u8,
        notes: v.get("notes").and_then(Value::as_str).unwrap_or("").to_string(),
    })
}

/// How the harness runs the loop (see [`Budget`]); `critic` is the judge threshold for the in-loop
/// critic, None to judge only at the end.
#[derive(Clone, Copy, Debug)]
pub struct LoopConfig { pub budget: Budget, pub critic: Option<u8> }

/// Run one case on one model.
pub fn run_case<C: AuthorChat>(case: &EvalCase, label: &str, chat: &mut C, engine_base: &str, cfg: LoopConfig, judge_cfg: Option<&AiCfg>) -> CaseResult {
    let t0 = Instant::now();
    let mut engine = HttpEngine { base: engine_base.to_string() };
    let mut in_loop_judgement = None;
    let run = match (cfg.critic, judge_cfg) {
        (Some(threshold), Some(jc)) => {
            let mut critic = JudgeCritic { cfg: jc, threshold, last: None };
            let run = author_with(&case.request, chat, &mut engine, &mut critic, cfg.budget);
            in_loop_judgement = critic.last;
            run
        }
        _ => author_with(&case.request, chat, &mut engine, &mut NoCritic, cfg.budget),
    };
    let secs = t0.elapsed().as_secs_f64();
    let mut res = CaseResult {
        case: case.id.clone(),
        source: case.source.clone(),
        model: label.to_string(),
        passed: run.passed,
        tries: run.tries,
        secs: (secs * 10.0).round() / 10.0,
        problems: run.problems.clone(),
        error: run.error.clone(),
        question: run.question.as_ref().map(MomQuestion::emit),
        log: run.log.clone(),
        restarts: run.restarts,
        critic_rounds: run.critic_rounds,
        lookups: run.lookups,
        preflight_catches: run.preflight_catches,
        ..Default::default()
    };
    if run.passed && in_loop_judgement.is_some() {
        // The in-loop critic already judged the final question.
        res.judge = in_loop_judgement;
    } else if let (true, Some(jc), Some(q)) = (run.passed, judge_cfg, run.question.as_ref()) {
        match judge(jc, &case.request, q) {
            Ok(j) => res.judge = Some(j),
            Err(e) => res.judge_error = Some(e),
        }
    }
    res
}

/// Results already in `results.jsonl`, so a stopped run resumes.
pub fn load_results(path: &Path) -> Vec<CaseResult> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// The comparison: one row per model, then every case.
pub fn report(results: &[CaseResult], cases: &[EvalCase], models: &[String]) -> String {
    let mut out = String::from("# Question-writing harness\n\n");
    out.push_str(&format!("{} cases ({} from the bank, {} hand-written edge cases), engine self-test at seeds 4242, 7, 99.\n\n",
        cases.len(), cases.iter().filter(|c| c.source == "bank").count(), cases.iter().filter(|c| c.source == "edge").count()));
    out.push_str("| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for m in models {
        let rs: Vec<&CaseResult> = results.iter().filter(|r| &r.model == m).collect();
        if rs.is_empty() { continue; }
        let passed: Vec<&&CaseResult> = rs.iter().filter(|r| r.passed).collect();
        let first = passed.iter().filter(|r| r.tries == 1).count();
        let mean_tries = if passed.is_empty() { 0.0 } else { passed.iter().map(|r| r.tries as f64).sum::<f64>() / passed.len() as f64 };
        let mut times: Vec<f64> = rs.iter().map(|r| r.secs).collect();
        times.sort_by(|a, b| a.total_cmp(b));
        let median = times.get(times.len() / 2).copied().unwrap_or(0.0);
        let js: Vec<&Judgement> = rs.iter().filter_map(|r| r.judge.as_ref()).collect();
        let pct = |n: usize, d: usize| if d == 0 { "–".to_string() } else { format!("{}/{}", n, d) };
        let jmean = if js.is_empty() { "–".into() } else { format!("{:.2}", js.iter().map(|j| j.score as f64).sum::<f64>() / js.len() as f64) };
        let sum = |f: fn(&CaseResult) -> usize| rs.iter().map(|r| f(r)).sum::<usize>();
        out.push_str(&format!("| {m} | {} | {} | {:.2} | {:.0} s | {jmean} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            pct(passed.len(), rs.len()), pct(first, rs.len()), mean_tries, median,
            pct(js.iter().filter(|j| j.score >= 4).count(), rs.len()),
            pct(js.iter().filter(|j| j.matches_request).count(), js.len()),
            pct(js.iter().filter(|j| j.clear).count(), js.len()),
            pct(js.iter().filter(|j| j.math_correct).count(), js.len()),
            sum(|r| r.restarts), sum(|r| r.critic_rounds), sum(|r| r.lookups), sum(|r| r.preflight_catches)));
    }
    out.push_str("\nThe judge only sees questions that passed the engine. \"Passed\" is the engine's verdict; the judge columns are one model's opinion.\n\n## Cases\n\n| Case | ");
    out.push_str(&models.join(" | "));
    out.push_str(" |\n|---|");
    out.push_str(&"---|".repeat(models.len()));
    out.push('\n');
    let by: BTreeMap<(&str, &str), &CaseResult> = results.iter().map(|r| ((r.case.as_str(), r.model.as_str()), r)).collect();
    for c in cases {
        out.push_str(&format!("| `{}` |", c.id));
        for m in models {
            let cell = match by.get(&(c.id.as_str(), m.as_str())) {
                None => "not run".to_string(),
                Some(r) if r.passed => match &r.judge {
                    Some(j) => format!("pass, {} {}, judge {}", r.tries, if r.tries == 1 { "try" } else { "tries" }, j.score),
                    None => format!("pass, {} {}", r.tries, if r.tries == 1 { "try" } else { "tries" }),
                },
                Some(r) if r.error.is_some() => "error".to_string(),
                Some(r) => format!("fail ({} left)", r.problems.len()),
            };
            out.push_str(&format!(" {cell} |"));
        }
        out.push('\n');
    }
    out.push_str("\n## Judge notes on questions scoring 3 or less\n\n");
    for r in results.iter().filter(|r| r.judge.as_ref().is_some_and(|j| j.score <= 3)) {
        let j = r.judge.as_ref().unwrap();
        out.push_str(&format!("- `{}` / {}: {} — {}\n", r.case, r.model, j.score, j.notes));
    }
    out.push_str("\n## Problems left on failing runs\n\n");
    for r in results.iter().filter(|r| !r.passed) {
        let what = r.error.clone().unwrap_or_else(|| r.problems.first().cloned().unwrap_or_default());
        out.push_str(&format!("- `{}` / {}: {}\n", r.case, r.model, what.chars().take(200).collect::<String>()));
    }
    out
}

/// Re-judge a finished run's passing questions with another model (`DAD_REJUDGE=<run dir>`,
/// `DAD_REJUDGE_MODEL=<label>`): an independent check when the run's own judge also coached the
/// writer in the loop. Writes `<run dir>/rejudge-<label>.jsonl` and prints a summary line.
pub fn rejudge_from_env() -> Result<(), String> {
    let env = |k: &str| crate::env_var(k).ok();
    let dir = std::path::PathBuf::from(env("DAD_REJUDGE").unwrap());
    let models_file: ModelsFile = serde_json::from_str(&std::fs::read_to_string(env("DAD_EVAL_MODELS").unwrap_or_else(|| "harness/models.json".into())).map_err(|e| e.to_string())?)
        .map_err(|e| format!("models file: {e}"))?;
    let label = env("DAD_REJUDGE_MODEL").ok_or("DAD_REJUDGE_MODEL=<label> is required")?;
    let cfg = models_file.models.iter().find(|m| m.label == label).ok_or(format!("no model {label}"))?.cfg();
    let cases: Vec<EvalCase> = serde_json::from_str::<EvalFile>(&std::fs::read_to_string(env("DAD_EVAL").unwrap_or_else(|| "harness/evals.json".into())).map_err(|e| e.to_string())?)
        .map_err(|e| format!("evals file: {e}"))?.cases;
    let results = load_results(&dir.join("results.jsonl"));
    let todo: Vec<&CaseResult> = results.iter().filter(|r| r.passed && r.question.is_some()).collect();
    let jobs: usize = env("DAD_EVAL_JOBS").and_then(|t| t.parse().ok()).unwrap_or(4).max(1);
    let queue = Mutex::new(todo.iter().collect::<VecDeque<_>>());
    let out = Mutex::new(Vec::new());
    std::thread::scope(|s| for _ in 0..jobs {
        s.spawn(|| loop {
            let Some(r) = queue.lock().unwrap().pop_front() else { break };
            let request = cases.iter().find(|c| c.id == r.case).map(|c| c.request.clone()).unwrap_or_default();
            let q = MomQuestion::parse(r.question.as_deref().unwrap_or(""));
            let j = judge(&cfg, &request, &q);
            out.lock().unwrap().push((r.case.clone(), j.ok()));
        });
    });
    let out = out.into_inner().unwrap();
    let scores: Vec<u8> = out.iter().filter_map(|(_, j)| j.as_ref().map(|j| j.score)).collect();
    let mut f = std::fs::File::create(dir.join(format!("rejudge-{label}.jsonl"))).map_err(|e| e.to_string())?;
    for (case, j) in &out { let _ = writeln!(f, "{}", serde_json::json!({"case": case, "judge": j})); }
    let mean = scores.iter().map(|&x| x as f64).sum::<f64>() / scores.len().max(1) as f64;
    println!("{}: {} passed of {}, re-judged by {label}: mean {:.2}, 4+ {}/{}, unanswered {}",
        dir.display(), todo.len(), results.len(), mean, scores.iter().filter(|&&x| x >= 4).count(), results.len(), out.len() - scores.len());
    Ok(())
}

/// The `DAD_EVAL` entry point.
pub fn run_from_env() -> Result<(), String> {
    let env = |k: &str| crate::env_var(k).ok();
    let cases: Vec<EvalCase> = serde_json::from_str::<EvalFile>(&std::fs::read_to_string(env("DAD_EVAL").unwrap()).map_err(|e| e.to_string())?)
        .map_err(|e| format!("evals file: {e}"))?.cases;
    let only: Option<Vec<String>> = env("DAD_EVAL_ONLY").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let cases: Vec<EvalCase> = cases.into_iter().filter(|c| only.as_ref().is_none_or(|o| o.contains(&c.id))).collect();
    let models_file: ModelsFile = serde_json::from_str(&std::fs::read_to_string(env("DAD_EVAL_MODELS").unwrap_or_else(|| "harness/models.json".into())).map_err(|e| e.to_string())?)
        .map_err(|e| format!("models file: {e}"))?;
    let pick: Option<Vec<String>> = env("DAD_EVAL_PICK").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let models: Vec<ModelSpec> = models_file.models.iter().filter(|m| pick.as_ref().is_none_or(|p| p.contains(&m.label))).cloned().collect();
    let judge_cfg = models_file.judge.as_ref().and_then(|j| models_file.models.iter().find(|m| &m.label == j)).map(ModelSpec::cfg);
    let out_dir = std::path::PathBuf::from(env("DAD_EVAL_OUT").unwrap_or_else(|| "harness/runs/latest".into()));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let results_path = out_dir.join("results.jsonl");
    let engine = env("DAD_ENGINE").unwrap_or_else(|| "http://127.0.0.1:8791".into());
    let tries: usize = env("DAD_TRIES").and_then(|t| t.parse().ok()).unwrap_or(4);
    // DAD_RESTARTS: fresh conversations after a failure; DAD_CRITIC=<1-5>: send passing questions the
    // judge scores below that back for one more round; DAD_DOCS=0: no reference (cheat sheet, lookups,
    // pre-flight), for comparison.
    let loop_cfg = LoopConfig {
        budget: Budget {
            max_tries: tries,
            restarts: env("DAD_RESTARTS").and_then(|t| t.parse().ok()).unwrap_or(0),
            use_docs: env("DAD_DOCS").as_deref() != Some("0"),
        },
        critic: env("DAD_CRITIC").and_then(|t| t.parse().ok()),
    };
    let jobs: usize = env("DAD_EVAL_JOBS").and_then(|t| t.parse().ok()).unwrap_or(3).max(1);

    let done: Vec<(String, String)> = load_results(&results_path).into_iter().map(|r| (r.case, r.model)).collect();
    let file = Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(&results_path).map_err(|e| e.to_string())?);
    let todo: usize = models.iter()
        .map(|m| cases.iter().filter(|c| !done.contains(&(c.id.clone(), m.label.clone()))).count())
        .sum();
    eprintln!("{} cases x {} models, {} already done, {} jobs per model, judge: {}, loop: {:?}", cases.len(), models.len(), done.len(), jobs,
        models_file.judge.as_deref().unwrap_or("none"), loop_cfg);
    let finished = Mutex::new(0usize);
    std::thread::scope(|s| {
        for m in &models {
            let queue = Mutex::new(cases.iter().filter(|c| !done.contains(&(c.id.clone(), m.label.clone()))).cloned().collect::<VecDeque<_>>());
            let queue = std::sync::Arc::new(queue);
            for _ in 0..jobs {
                let (queue, file, finished, judge_cfg, engine) = (queue.clone(), &file, &finished, judge_cfg.as_ref(), engine.as_str());
                s.spawn(move || {
                    let cfg = m.cfg();
                    loop {
                        let Some(case) = queue.lock().unwrap().pop_front() else { break };
                        let mut chat = ModelChat { cfg: &cfg, tools: m.tools };
                        let r = run_case(&case, &m.label, &mut chat, engine, loop_cfg, judge_cfg);
                        let mut n = finished.lock().unwrap();
                        *n += 1;
                        eprintln!("[{}/{}] {} {}: {} in {} {} ({:.0} s){}", *n, todo, m.label, case.id,
                            if r.passed { "pass" } else if r.error.is_some() { "error" } else { "FAIL" },
                            r.tries, if r.tries == 1 { "try" } else { "tries" }, r.secs,
                            r.judge.as_ref().map(|j| format!(", judge {}", j.score)).unwrap_or_default());
                        let line = serde_json::to_string(&r).unwrap_or_default();
                        let mut f = file.lock().unwrap();
                        let _ = writeln!(f, "{line}");
                        let _ = f.flush();
                    }
                });
            }
        }
    });
    let all = load_results(&results_path);
    // Every model in the models file that has results, so a partial re-run (DAD_EVAL_PICK) still
    // reports the whole comparison.
    let labels: Vec<String> = models_file.models.iter().map(|m| m.label.clone())
        .filter(|l| all.iter().any(|r| &r.model == l)).collect();
    std::fs::write(out_dir.join("report.md"), report(&all, &cases, &labels)).map_err(|e| e.to_string())?;
    eprintln!("report: {}", out_dir.join("report.md").display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(case: &str, model: &str, passed: bool, tries: usize, score: Option<u8>) -> CaseResult {
        CaseResult {
            case: case.into(), source: "bank".into(), model: model.into(), passed, tries, secs: 10.0,
            judge: score.map(|s| Judgement { matches_request: s >= 4, clear: true, math_correct: true, score: s, notes: "n".into() }),
            problems: if passed { vec![] } else { vec!["part 0: correct answer scored 0".into()] },
            ..Default::default()
        }
    }

    #[test]
    fn report_compares_models() {
        let cases = vec![
            EvalCase { id: "a".into(), source: "bank".into(), request: "x".into(), r#ref: None },
            EvalCase { id: "b".into(), source: "edge".into(), request: "y".into(), r#ref: None },
        ];
        let results = vec![r("a", "m1", true, 1, Some(5)), r("b", "m1", false, 4, None), r("a", "m2", true, 2, Some(3))];
        let md = report(&results, &cases, &["m1".into(), "m2".into()]);
        assert!(md.contains("| m1 | 1/2 | 1/2 | 1.00 |"), "{md}");
        assert!(md.contains("| `b` | fail (1 left) | not run |"), "{md}");
        assert!(md.contains("`a` / m2: 3 — n"), "{md}");
    }
}
