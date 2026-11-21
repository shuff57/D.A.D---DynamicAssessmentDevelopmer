# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| gemma4-31b | 36/40 | 2/40 | 4.33 | 28 s | 4.28 | 36/40 | 36/36 | 35/36 | 36/36 | 14 | 33 | 0 | 10 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | gemma4-31b |
|---|---|
| `bank-01` | pass, 4 tries, judge 4 |
| `bank-02` | pass, 3 tries, judge 4 |
| `bank-03` | pass, 4 tries, judge 4 |
| `bank-04` | pass, 6 tries, judge 4 |
| `bank-05` | pass, 3 tries, judge 4 |
| `bank-06` | pass, 3 tries, judge 5 |
| `bank-07` | pass, 7 tries, judge 4 |
| `bank-08` | pass, 4 tries, judge 4 |
| `bank-09` | pass, 1 try, judge 4 |
| `bank-10` | pass, 1 try, judge 5 |
| `bank-11` | pass, 2 tries, judge 5 |
| `bank-12` | pass, 3 tries, judge 4 |
| `bank-13` | pass, 3 tries, judge 4 |
| `bank-14` | pass, 12 tries, judge 4 |
| `bank-15` | fail (2 left) |
| `bank-16` | pass, 5 tries, judge 4 |
| `bank-17` | pass, 3 tries, judge 4 |
| `bank-18` | pass, 2 tries, judge 4 |
| `bank-19` | pass, 2 tries, judge 5 |
| `bank-20` | pass, 7 tries, judge 4 |
| `bank-21` | pass, 4 tries, judge 4 |
| `bank-22` | pass, 8 tries, judge 4 |
| `bank-23` | pass, 2 tries, judge 5 |
| `bank-24` | pass, 3 tries, judge 5 |
| `bank-25` | pass, 3 tries, judge 4 |
| `bank-26` | pass, 2 tries, judge 4 |
| `bank-27` | pass, 4 tries, judge 4 |
| `bank-28` | pass, 10 tries, judge 5 |
| `bank-29` | pass, 8 tries, judge 5 |
| `bank-30` | pass, 2 tries, judge 4 |
| `edge-picture` | pass, 3 tries, judge 4 |
| `edge-select-all` | pass, 8 tries, judge 4 |
| `edge-matching` | pass, 11 tries, judge 5 |
| `edge-dropdown` | fail (1 left) |
| `edge-money` | pass, 2 tries, judge 4 |
| `edge-matrix` | fail (1 left) |
| `edge-equation` | pass, 6 tries, judge 4 |
| `edge-distractors` | fail (1 left) |
| `edge-string` | pass, 3 tries, judge 4 |
| `edge-feedback-solution` | pass, 2 tries, judge 5 |

## Judge notes on questions scoring 3 or less


## Problems left on failing runs

- `bank-15` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-dropdown` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-matrix` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-distractors` / gemma4-31b: Caught error while evaluating the code in this question: Unsupported operand types: int * string on line 28 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     27 | 
  >  28 | // I
