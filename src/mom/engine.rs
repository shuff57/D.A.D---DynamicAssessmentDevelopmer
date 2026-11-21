//! The D.A.D engine's wire format (`engine/serve.php`): requests and responses only, no I/O, so the
//! same types serve the desktop build (blocking HTTP) and the web build (fetch).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Seeds the self-test uses unless told otherwise. Three seeds catch most randomization bugs
/// (a distractor that collides with the answer at some seed) at a third of a second per question.
pub const DEFAULT_SEEDS: [i64; 3] = [4242, 7, 99];

#[derive(Serialize, Debug)]
pub struct RenderRequest<'a> {
    pub src: &'a str,
    pub seed: i64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub showans: bool,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct RenderResponse {
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub jsparams: Value,
    /// IMathAS's displayed correct answer per part (formatted for reading, not for typing).
    #[serde(default)]
    pub answers: Value,
    #[serde(default)]
    pub answeights: Vec<f64>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct SelftestRequest<'a> {
    pub src: &'a str,
    pub seeds: &'a [i64],
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct SelftestResponse {
    pub pass: bool,
    #[serde(default)]
    pub qtype: String,
    /// Essay, upload, drawing and picture-option parts: a teacher has to look at these.
    #[serde(default)]
    pub untested_parts: Vec<usize>,
    #[serde(default)]
    pub runs: Vec<SelftestRun>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct SelftestRun {
    pub seed: i64,
    pub ok: bool,
    #[serde(default)]
    pub problems: Vec<String>,
    #[serde(default)]
    pub render_errors: Vec<String>,
    #[serde(default)]
    pub correct: ScoreResponse,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ScoreResponse {
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl SelftestResponse {
    /// Every distinct problem across seeds, each tagged with the seeds it happened at, in the order
    /// first seen: the feedback an author (or the AI) acts on.
    pub fn problems(&self) -> Vec<String> {
        let mut seen: Vec<(String, Vec<i64>)> = Vec::new();
        for run in &self.runs {
            let errs = run.render_errors.iter().chain(run.correct.errors.iter());
            for p in run.problems.iter().filter(|p| *p != "render errors" && *p != "score errors").chain(errs) {
                match seen.iter_mut().find(|(t, _)| t == p) {
                    Some((_, seeds)) => seeds.push(run.seed),
                    None => seen.push((p.clone(), vec![run.seed])),
                }
            }
        }
        seen.into_iter()
            .map(|(p, s)| format!("{p} (seed{} {})", if s.len() > 1 { "s" } else { "" },
                s.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problems_are_merged_across_seeds() {
        let r: SelftestResponse = serde_json::from_value(serde_json::json!({
            "pass": false, "qtype": "multipart", "untested_parts": [],
            "runs": [
                {"seed": 4242, "ok": false, "problems": ["part 0: correct answer scored 0", "render errors"],
                 "render_errors": ["Caught error: x"], "correct": {"total": 0, "errors": []}},
                {"seed": 7, "ok": false, "problems": ["part 0: correct answer scored 0"],
                 "render_errors": [], "correct": {"total": 0, "errors": []}}
            ]
        })).unwrap();
        assert_eq!(r.problems(), vec![
            "part 0: correct answer scored 0 (seeds 4242, 7)".to_string(),
            "Caught error: x (seed 4242)".to_string(),
        ]);
    }

    #[test]
    fn render_request_omits_showans_when_false() {
        let j = serde_json::to_string(&RenderRequest { src: "x", seed: 1, showans: false }).unwrap();
        assert_eq!(j, r#"{"src":"x","seed":1}"#);
    }
}
