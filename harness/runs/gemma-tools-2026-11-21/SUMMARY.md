# Gemma 4 31B: native lookup tool and thinking (2026-11-21)

Same 40 cases, plain loop (reference on, 4 tries, no restarts, no judge in the loop), judged
afterwards by claude-sonnet-5.5, which coached none of them. Ollama says Gemma 4's thinking is off by
default, so every earlier Gemma run was without it.

| Setup | Passed | First try | Judged 4+ | Judge mean | Median time | Cases that looked something up |
|---|---|---|---|---|---|---|
| as is (text LOOKUP protocol) | 33/40 | 14/40 | 9/40 | 3.15 | 5 s | 0 |
| + native `lookup` tool | **38/40** | 21/40 | 18/40 | 3.34 | **4 s** | 16 (30 lookups) |
| + thinking on | 35/40 | 18/40 | 17/40 | 3.51 | 15 s | 0 |
| + both | 37/40* | 22/40 | **20/40** | 3.64 | 15 s | 1 |

\* One case still failed on an Ollama Cloud timeout after a re-run. Thinking runs had 7 such
network failures (timeouts and 502s) in the first pass; those cases were re-run
(`results-first-pass.jsonl` keeps the original).

- **The native tool is the fix for lookups.** Offered as a function, Gemma used it in 16 of 40 cases,
  mostly `answer_type` (26 of 30 lookups). With the text protocol, 0 in 240 Gemma cases so far.
  It took passing from 33 to 38 and doubled the good questions, at no cost in time.
- **Thinking raises quality, not passing**, at about 3 times the time, and Ollama Cloud drops more of
  those slower requests. With thinking on, Gemma almost stops calling the tool (1 case): it reasons
  instead of looking up.
- **Both** gives the most good questions (20/40) but not more passes than tools alone.
- Remaining failures with tools: one choices index mismatch (bank-21), one `jointshuffle` called with one
  argument (edge-select-all). Choice failures no longer dominate, so pushing docs on failure isn't
  needed yet.
- For comparison, DeepSeek with default thinking on the same loop: 38/40 passed, 33/40 good, 62 s.
