# D.A.D engine

A stateless IMathAS (MyOpenMath's engine) that renders, scores and self-tests one question per
request. No database, no login. IMathAS is pinned in the `Dockerfile`.

```
engine/run.sh                                   # build + run on http://127.0.0.1:8791
python3 engine/regress.py <mom-content folder>  # self-test the whole bank (about 15 s for 1,239)
python3 engine/regress.py <mom-content folder> --check engine/baseline.json   # exit 1 on a regression
python3 engine/triage.py <mom-content folder>   # rewrite docs/dad/bank-triage.md and the baseline
```

| Route | Body | Returns |
|---|---|---|
| `POST /` | mom question as text, `?seed=` | an HTML page (the old VPS sandbox's contract) |
| `POST /api/render` | `{src, seed?, showans?}` | `html`, `jsparams`, `answers` (displayed correct answers), `answeights`, `errors` |
| `POST /api/score` | `{src, seed?, answers: {part: value}}` | `scores`, `raw`, `total`, `errors` |
| `POST /api/selftest` | `{src, seeds?}` | per seed: the correct answers submitted the way a student would (each part must score 1) and a blank attempt (each part must score 0) |
| `POST /api/vars` | `{src, seed?}` | the question's variables after its code runs (`answer`, `anstypes`, `questions`, ...) |
| `POST /api/student` | `{src, seed?, static, engine, post?}` | the sandbox: MyOpenMath's own one-question student view (a port of IMathAS's `embedq2.php`, through its real `header.php`/`footer.php`); with `post`, its Submit / Try a similar question answers (`student.php`) |
| `GET /javascript/…`, `/mathquill/…`, `/assess2/vue/…`, `/*.css` … | | IMathAS's scripts, styles and images for that page, with CORS (never PHP source) |

The self-test types the author's own `$answer`, not the display form (which reads "4 × 3 = 12" or
"xge5"), and like a student retries a missed part with up to four forms (raw, display converted to
typing). Choices and select-all are matched by option text, dropdowns included; matching by letter;
matrices cell by cell. It also flags defects a student would trip on whatever the key says: duplicate
options, HTML or backtick math inside a dropdown option, and a variable the question text prints that
the code never sets at that seed (IMathAS silently prints nothing). Essay, upload, drawing and picture-only
parts are reported as `untested_parts`: a teacher must look, but they do not fail the test.

Question code runs in a scope of its own (`dad_eval_question`), so its variable names cannot collide
with the engine's. An `ANSWER` section is run as code only when it looks like code; prose there is the
worked solution (MyOpenMath's Detailed Solution field). Only the five real section markers start a
section: a comment banner like `// === Scenario picker ===` inside the code stays part of it.

Current bank (2026-11-20): 1,167 of 1,239 pass at seeds 4242, 7, 99. The 72 failures are triaged in
`docs/dad/bank-triage.md`: 57 are question defects, 15 need a look. `baseline.json` records pass/fail.

The same engine runs in the browser on PHP compiled to WebAssembly (`engine/browser/`), with the
same verdict on all 1,239 bank questions. Licensing: `engine/LICENSE.md` (GPL-2.0-only, like IMathAS).

`serve-vps-original.php` is the render-only sandbox that runs on the VPS today
(`mom.huffpalmer.fyi`), kept for reference.
