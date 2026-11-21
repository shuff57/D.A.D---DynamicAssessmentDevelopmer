# DeepSeek thinking levels, and OpenRouter's free router (2026-11-21)

Same 40 cases, plain loop (reference on, 4 tries, no restarts, no judge in the loop), judged afterwards
by claude-sonnet-5.5. `think` is a new per-model setting in `harness/models.json` (Ollama `think`,
OpenAI-style `reasoning_effort` elsewhere).

| Setup | Passed | First try | Judged 4+ | Judge mean | Median time | Lookups |
|---|---|---|---|---|---|---|
| DeepSeek, default thinking | 38/40 | 19/40 | 33/40 | 4.21 | 62 s | 78 |
| DeepSeek, think low | 35/40 | 6/40 | 29/40 | 4.23 | 58 s | 74 |
| DeepSeek, think off | 29/40 | 9/40 | 7/40 | 3.00 | 12 s | 0 |
| DeepSeek, think off + split-line pre-flight | 30/40 | 15/40 | 11/40 | 3.13 | 6 s | 0 |
| OpenRouter `openrouter/free` | 33/40 | 15/40 | 11/40 | 3.21 | 168 s | 30 |

- **Thinking off** loses about a quarter of the passes and most of the quality. Seven of its first
  run's 11 failures were one mistake: a string continued on a new line starting with `.`. IMathAS reads
  one statement per line (an expression may continue only inside an open `(` or `[`, checked against the
  engine), and its error points at the wrong line, so the model never found it. Pre-flight now catches
  that (0 false positives on the bank's 1,167 passing questions). With it the syntax errors are gone,
  first-try passes go from 9 to 15 and time halves, but the remaining failures (duplicate options,
  answers that don't score) need reasoning, so it stays near 30/40.
- **Think low** doesn't save time: it needs more tries (6/40 first try), so the median is unchanged.
- **Default thinking** is the one to use. It did use the lookups (78), unlike Gemma (0). It got 38/40
  here vs 40/40 on 2026-11-20, which is run-to-run variance (bank-18 and bank-28, both choices).
- **The free router** served 11 different free models across one run (Nemotron 3 Ultra 23 replies,
  Nemotron 3.5 Lightning 18, Flint 15, Nemotron 3.5 Super 11, ...), often switching mid-conversation.
  Gemma-level quality at about 3 times DeepSeek's time; 2 cases got empty replies (one after 888 s).
  It costs nothing, but it is slow and unpredictable. Free models also have daily request limits and
  their providers may keep prompts.
