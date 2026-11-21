# Gemma 4 31B: how much the loop raises it (2026-11-20)

40 cases (harness/evals.json), engine self-test at seeds 4242, 7, 99, up to 4 tries per conversation.
Judged 1-5; "4+" counts cases out of all 40. Two judges: Claude Sonnet 5.5 (the run's own judge,
which in D/D2 also coached the writer in the loop) and DeepSeek V4.1 Flash (independent, afterwards).

| Setup | Passed | 4+ (Claude) | 4+ (DeepSeek, independent) | Median time |
|---|---|---|---|---|
| A: failing line quoted | 32/40 | 7/40 | 12/40 | 5 s |
| B: + MyOpenMath reference (cheat sheet, lookups, pre-flight) | 35/40 | 16/40 | 21/40 | 5 s |
| C: + 2 fresh restarts | 37/40 | 12/40 | 20/40 | 4 s |
| D: + judge in the loop (below 4 goes back) | 36/40 | 31/40 | 30/40 | 31 s |
| D2: D again, line quoting fixed inside blocks | 36/40 | 36/40 | 25/40 | 28 s |
| DeepSeek V4.1 Flash, plain loop (2026-11-20 run) | 40/40 | 34/40 | (judges itself) | 57 s |

- The reference is the cheap win: +3 passes and about +9 good questions by either judge, no time cost.
  Gemma never used the LOOKUP tools (0 in every run); the gain is the cheat sheet and pre-flight.
- The judge in the loop is the big quality step (independent judge: 12 to 25-30 good), at about
  six times the time. Some of what the coaching judge rewards does not carry to another judge
  (D2: 36 by Claude, 25 by DeepSeek), so an independent judge should score any run whose own judge
  coached it.
- Run-to-run variation is several cases (D vs D2), larger than the line-quoting fix could show.
- Every setup still failed edge-matrix (the matrix answer never scores) and edge-distractors
  (jointshuffle misused).
