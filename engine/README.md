# D.A.D engine

A stateless IMathAS (MyOpenMath's engine) that renders, scores and self-tests one question per
request. No database, no login. IMathAS is pinned in the `Dockerfile`.

```
engine/run.sh                                   # build + run on http://127.0.0.1:8791
python3 engine/regress.py <mom-content folder>  # self-test the whole bank (about 13 s for 1,239)
```

| Route | Body | Returns |
|---|---|---|
| `POST /` | mom question as text, `?seed=` | an HTML page (the old VPS sandbox's contract) |
| `POST /api/render` | `{src, seed?, showans?}` | `html`, `jsparams`, `answers` (displayed correct answers), `answeights`, `errors` |
| `POST /api/score` | `{src, seed?, answers: {part: value}}` | `scores`, `raw`, `total`, `errors` |
| `POST /api/selftest` | `{src, seeds?}` | per seed: the correct answers submitted the way a student would (each part must score 1) and a blank attempt (each part must score 0) |

The self-test answers choices (radio buttons or dropdowns), select-all, matching, and typed answers
(taking the first of "A or B"). Essay, file and drawing parts are reported as `untested_parts`:
they need a teacher's look, but they don't fail the test.

First full run (2026-11-20, steve-desktop bank): 1,068 of 1,239 pass at seeds 4242, 7 and 99.
Of the 25 questions steve-desktop marks BROKEN, 22 fail here too.

`serve-vps-original.php` is the render-only sandbox that runs on the VPS today
(`mom.huffpalmer.fyi`), kept for reference.
