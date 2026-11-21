# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| gemma4-31b | 36/40 | 5/40 | 4.75 | 32 s | 3.97 | 31/40 | 35/36 | 33/36 | 34/36 | 16 | 30 | 0 | 8 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | gemma4-31b |
|---|---|
| `bank-01` | pass, 7 tries, judge 4 |
| `bank-02` | pass, 8 tries, judge 4 |
| `bank-03` | pass, 6 tries, judge 4 |
| `bank-04` | pass, 7 tries, judge 5 |
| `bank-05` | pass, 12 tries, judge 3 |
| `bank-06` | pass, 3 tries, judge 4 |
| `bank-07` | pass, 3 tries, judge 4 |
| `bank-08` | pass, 4 tries, judge 4 |
| `bank-09` | pass, 1 try, judge 5 |
| `bank-10` | pass, 5 tries, judge 4 |
| `bank-11` | pass, 1 try, judge 4 |
| `bank-12` | pass, 3 tries, judge 4 |
| `bank-13` | pass, 2 tries, judge 4 |
| `bank-14` | pass, 12 tries, judge 2 |
| `bank-15` | fail (4 left) |
| `bank-16` | pass, 2 tries, judge 4 |
| `bank-17` | pass, 4 tries, judge 4 |
| `bank-18` | pass, 2 tries, judge 5 |
| `bank-19` | pass, 1 try, judge 4 |
| `bank-20` | pass, 8 tries, judge 4 |
| `bank-21` | fail (1 left) |
| `bank-22` | pass, 12 tries, judge 3 |
| `bank-23` | pass, 2 tries, judge 5 |
| `bank-24` | pass, 2 tries, judge 4 |
| `bank-25` | pass, 2 tries, judge 4 |
| `bank-26` | pass, 7 tries, judge 5 |
| `bank-27` | pass, 3 tries, judge 4 |
| `bank-28` | pass, 5 tries, judge 4 |
| `bank-29` | pass, 1 try, judge 4 |
| `bank-30` | pass, 2 tries, judge 4 |
| `edge-picture` | pass, 4 tries, judge 4 |
| `edge-select-all` | pass, 12 tries, judge 3 |
| `edge-matching` | pass, 4 tries, judge 4 |
| `edge-dropdown` | pass, 12 tries, judge 3 |
| `edge-money` | pass, 2 tries, judge 4 |
| `edge-matrix` | fail (1 left) |
| `edge-equation` | pass, 7 tries, judge 4 |
| `edge-distractors` | fail (1 left) |
| `edge-string` | pass, 2 tries, judge 4 |
| `edge-feedback-solution` | pass, 1 try, judge 4 |

## Judge notes on questions scoring 3 or less

- `bank-05` / gemma4-31b: 3 — Scenario 0 is keyed 'observational', but the researcher asks volunteers to take the supplement, so she imposes the treatment and it is a non-randomized experiment. The solution's reason ('did not use random assignment to impose a treatment') contradicts itself. Rewrite it so participants already take the supplement on their own. Also, 'daily sleep patterns' as a confounder in the randomized blueberry study is weak, because random assignment balances such variables.
- `bank-14` / gemma4-31b: 2 — The code picks the intersection of the two constraint lines and assumes it is the optimum, but that holds only if the profit vector lies between the two constraint normals. For example, wax 4x+3y with scent 2x+2y and profit 6x+7y has its optimum at a different vertex, so parts b–d are wrong at those seeds. Fix by choosing profits as positive combinations of the constraint coefficients, or by checking the ratio condition.
- `bank-22` / gemma4-31b: 3 — Each classification is right, but the scenario text is wrapped in backticks (`$s0`, `$s1`, `$s2`), so IMathAS will typeset the English sentences as AsciiMath, collapsing spaces and turning words like "in" into symbols. Remove the backticks. Also, every seed has exactly two Binomial parts and one Not Binomial part, so students can guess the pattern.
- `edge-select-all` / gemma4-31b: 3 — The answers are right, but five of the six truth values are hardcoded (statements 1-4 are always true, statement 5 is always false) and only the range-vs-mean comparison is computed from the data. That breaks the 'computed from the data at that seed' requirement and makes the pattern guessable, with 4 or 5 statements true at every seed. Build each statement from a randomly chosen true or false claim, such as an offset mean or median, a wrong mode, or a comparison, and set its truth by checking it against the computed values.
- `edge-dropdown` / gemma4-31b: 3 — The math is right at every seed (a is always positive so the inequality never flips, and c-b is always divisible by a). However, part (a) is graded as an exact string, and the question never tells students to type 'infinity' or 'inf'. Students who use the usual 'oo' or the ∞ symbol, or who put in extra spaces, will be marked wrong. Part (b) also says 'the endpoint' when the interval has two endpoints, so say 'finite endpoint'. Add an entry hint or switch to the interval answer type.

## Problems left on failing runs

- `bank-15` / gemma4-31b: part 1 (calcntuple): no correct answer to try (seeds 4242, 7, 99)
- `bank-21` / gemma4-31b: Error - First argument to calconarray must be an array (seeds 4242, 7, 99)
- `edge-matrix` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-distractors` / gemma4-31b: Caught error while evaluating the code in this question: Unsupported operand types: int * string on line 34 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     33 | for ($i = 0; $i
