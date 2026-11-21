# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| gemma4-31b | 37/40 | 14/40 | 2.49 | 4 s | 3.32 | 12/40 | 31/37 | 21/37 | 31/37 | 7 | 0 | 0 | 5 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | gemma4-31b |
|---|---|
| `bank-01` | pass, 6 tries, judge 4 |
| `bank-02` | pass, 11 tries, judge 3 |
| `bank-03` | pass, 2 tries, judge 3 |
| `bank-04` | pass, 6 tries, judge 3 |
| `bank-05` | pass, 2 tries, judge 2 |
| `bank-06` | pass, 2 tries, judge 5 |
| `bank-07` | pass, 1 try, judge 3 |
| `bank-08` | pass, 1 try, judge 3 |
| `bank-09` | pass, 1 try, judge 4 |
| `bank-10` | pass, 1 try, judge 3 |
| `bank-11` | pass, 2 tries, judge 3 |
| `bank-12` | pass, 2 tries, judge 4 |
| `bank-13` | pass, 2 tries, judge 4 |
| `bank-14` | pass, 3 tries, judge 3 |
| `bank-15` | pass, 10 tries, judge 3 |
| `bank-16` | pass, 1 try, judge 5 |
| `bank-17` | pass, 2 tries, judge 3 |
| `bank-18` | pass, 1 try, judge 3 |
| `bank-19` | pass, 1 try, judge 3 |
| `bank-20` | pass, 2 tries, judge 3 |
| `bank-21` | pass, 1 try, judge 4 |
| `bank-22` | pass, 1 try, judge 3 |
| `bank-23` | pass, 2 tries, judge 3 |
| `bank-24` | pass, 1 try, judge 3 |
| `bank-25` | pass, 1 try, judge 3 |
| `bank-26` | pass, 1 try, judge 4 |
| `bank-27` | pass, 3 tries, judge 3 |
| `bank-28` | pass, 1 try, judge 2 |
| `bank-29` | pass, 2 tries, judge 5 |
| `bank-30` | pass, 2 tries, judge 2 |
| `edge-picture` | pass, 2 tries, judge 2 |
| `edge-select-all` | pass, 2 tries, judge 3 |
| `edge-matching` | pass, 6 tries, judge 4 |
| `edge-dropdown` | fail (1 left) |
| `edge-money` | pass, 3 tries, judge 3 |
| `edge-matrix` | fail (1 left) |
| `edge-equation` | pass, 2 tries, judge 3 |
| `edge-distractors` | fail (1 left) |
| `edge-string` | pass, 2 tries, judge 4 |
| `edge-feedback-solution` | pass, 1 try, judge 5 |

## Judge notes on questions scoring 3 or less

- `bank-03` / gemma4-31b: 3 — The ranges allow impossible setups: pA=0.7, pB=0.7, pAnd=0.05 gives P(A or B)=1.35, which is greater than 1. The overlap must satisfy pAnd >= pA+pB-1, so draw pAnd from [max(0.05, pA+pB-1), min(pA,pB)].
- `bank-05` / gemma4-31b: 2 — Part (c) never asks for the justified conclusion, and the randomized scenario breaks the grading: its part (b) answer is "none" and the generic "confounding variable" is accepted, while its part (c) accepts "cause" even though a causal claim is justified there. The exact-match string answers (a full sentence, or the single word "cause"/"causes") will mark nearly all valid free-text answers wrong, and the prompt gives no guidance on what to type.
- `bank-04` / gemma4-31b: 3 — The expected-count math and the cell-to-label mapping are right at every seed, and the three parts match the request. But the labels and prompts ($colLabel0, $rowLabel0, $prompt0 etc.) are wrapped in backticks, so IMathAS treats them as AsciiMath and renders words like 'Group A' and 'Expected count for Group A, Success' as garbled italic math with the spaces lost. Remove the backticks around text variables, keeping them only for true math.
- `bank-07` / gemma4-31b: 3 — The request says 'Scatterplot with Line', but the plot shows only points and no fitted line, so students judge linearity from the points alone. Draw the line of best fit (or a line through the trend) in the SVG. Secondary: with m=1 and noise of ±2, the stretched y-axis makes the linear case look quite scattered, and the SVG has no alt text.
- `bank-02` / gemma4-31b: 3 — Table 2 already prints every percentage, so part (b) is a read-off rather than a computation, and part (a) is just summing Table 1. Also, 'exact percentage' conflicts with rounding to one decimal (total 80 gives values like 3.75 stored as 3.8), the correct choice in (c) is always the first option, and sortarray sorts rather than shuffles the categories.
- `bank-10` / gemma4-31b: 3 — $anstypes is "number,number", so part (b) shows a plain text box and the four formula options in $questions[1] never appear; a student would have to guess to type 2. Changing it to "number,choices" fixes this, and part (a) and the nC2 math are otherwise correct.
- `bank-08` / gemma4-31b: 3 — The rubric never says how many checklist items earn Full, Partial or No Credit, and the key contradicts a checklist reading. Student 4 gets the correct mean and SD (items 2 and 3), the same count as Student 3, yet is keyed No Credit while Student 3 is keyed Partial. Student 2 does name the sampling distribution (item 1) but is keyed No Credit. Define the scoring scale, e.g. 4/4 Full, 2-3 Partial, 0-1 None, then rewrite the responses so each key follows from it.
- `bank-11` / gemma4-31b: 3 — The math is correct at every seed: the totals use inclusive 'both' counts, so inclusion-exclusion gives r1, r4 and r0 correctly. The service names are wrapped in backticks (`$A`, `$B`, `$C`), so they render as math, and 'Disney+' will show as italic letters with a dangling plus operator. Remove the backticks from the service names, and from `$total` and the counts too, since they aren't needed.
- `bank-14` / gemma4-31b: 3 — At some seeds (e.g. waxA=4, waxB=3, scentA=1, scentB=3, maxWax=100, maxScent=120) the constraint intersection still has a negative coordinate after the +50 adjustment, yet it is used as a vertex, so the keyed optimum can be an infeasible point. Parts (a)-(c) are also fragile: string matching with only trim_whitespace rejects correct entries like '10x+15y', so use numfunc with $variables='x,y' instead. Answers are also non-integer candle counts.
- `bank-17` / gemma4-31b: 3 — The IQR, fence and outlier logic is correct, but `$pop` and `$item` are wrapped in backticks, so they render as AsciiMath and the words run together (e.g. "agreenhousesample"); remove the backticks. Also, the lower-outlier test value can be negative (e.g. Q1=10, IQR=20 gives a fence of -20), which is impossible for heights, times, weights and lengths, so keep testVal >= 0 or adjust the ranges.
- `bank-18` / gemma4-31b: 3 — Part (b) never tells students to round, but the answer key is the root rounded to 2 decimals, so a student who enters 3.4457 or the exact radical will likely be marked wrong. Add "round to 2 decimal places" to part (b), and ideally to the other parts. Parts (a) and (c) work out exactly and the math is otherwise correct.
- `bank-15` / gemma4-31b: 3 — The cofactor, adjugate and X = A^{-1}B math is correct, and det = ±1 keeps every entry an integer. But the matrices are written as LaTeX \begin{pmatrix} inside AsciiMath backticks, so they probably won't display properly (use \( \) or AsciiMath matrix syntax). The matrix answers also set no $answersize, so students are never told what grid or format to enter (3x3 for A^{-1}, 3x1 for X).
- `bank-19` / gemma4-31b: 3 — The answers are correct (positive residual means the model underestimated, and index 0 matches that choice). However, the $solutionguide is a double-quoted string containing `\text{Residual}`, and PHP turns `\t` into a tab, so the worked solution will likely display garbled (escape it as `\\text` or use single quotes). Also, the comment promises a line 'reasonably close' to the point, but the random slope and intercept can give residuals of several hundred, so the candidate line is often a poor fit.
- `bank-20` / gemma4-31b: 3 — The math is right and a student knows to enter the right side of y = mx + b, but fraction y-values are stored with their own backticks and then wrapped in backticks again in the table and $solutionguide, so the math delimiters nest and the fractions will likely not typeset. The solution also calls an unreduced slope like (2/4) 'simplified', since m and b are not reduced. Fix by storing plain 'a/b' strings and reducing n1/d1 and n2/d2 by their gcd.
- `bank-22` / gemma4-31b: 3 — The classifications and reasons are all correct, but the question text puts each scenario in backticks (`$prompt[0]`, etc.). MyOpenMath renders backticks as AsciiMath, so the sentences will display as garbled italic math and some words may turn into symbols. Remove the backticks and write plain $prompt[0], $prompt[1], $prompt[2].
- `bank-24` / gemma4-31b: 3 — The teacher asked for a free-response question, but both parts are multiple choice ('choices,choices'). The answer keys are correct (A has the lower SD, B has the wider spread), but the correct choice is always in the same position, so add $randomizep or shuffle the options. The ranges are also unrealistic for two contexts: delivery times of 65-95 days and phone battery life of 65-95 hours. The $strflags and the $high_std/$low_std variables are unused.
- `bank-23` / gemma4-31b: 3 — The logic is right (null value inside the interval means fail to reject; outside means reject), but the ranges don't fit the contexts: sleep of 15-45 hours per night is impossible, and grocery spending of 15-45 dollars a month is implausible. Text variables like `$pop`, `$variable` and `$unit` are wrapped in backticks, which render as asciimath and will garble the wording, and phrases like "the adults in a city's average commute time" read awkwardly. The question also never says the test is two-sided, which the CI method assumes.
- `bank-25` / gemma4-31b: 3 — Parts (a)-(c) correctly give 75, 88.89 and 95.06, and part (d) rejects the 'exact percentage' claim. However, choice 3 ('no more than 1/k^2 falls outside') is also a true consequence of Chebyshev's Rule, so a careful student could defend two answers. Replace it with a false distractor, such as 'at most 1/k^2 falls outside' changed to something incorrect. Also, 'Chebyshev\'s' in the question text may display a stray backslash.
- `bank-27` / gemma4-31b: 3 — The solution guide's z-score step builds `\frac{c - mu}` and then appends the denominator without braces, so the denominator renders wrongly (e.g. only its first digit is under the bar). The question also gives no rounding instruction. The standard deviation is irrational, and with no tolerance set the default is tight, so a student who rounds to 2 decimals could be marked wrong. Also check that normalcdf(-99, z, 0, 1) is a valid call in the stats library and returns the left-tail area; if the extra arguments are ignored, the probability would be wrong.
- `bank-28` / gemma4-31b: 2 — For fractional exponents the display `log_b(x^n/m)` is AsciiMath, which renders as (x^n)/m, not x^(n/m), so the question shown doesn't match the answer (n/m)*log(x); this affects about half the seeds, and the fix is to write the exponent as ^(n/m). Also, the numeric answer string uses a decimal p_val, and random x can equal the base or a perfect power (e.g. log_2(4^3)), which makes 'expand' ambiguous or trivial.
- `bank-30` / gemma4-31b: 2 — Part (b) shuffles the expected subsets into a random order that the student cannot know, so a correct list is usually marked wrong (about 5 in 6 seeds if entered naturally). Exact-string matching also rejects equivalent entries like {b, a} or {a,b}. Fix by fixing the order and stating it (e.g. singletons first, then the full set), or by using a single order-independent answer, and by telling students the exact format to use.
- `edge-picture` / gemma4-31b: 2 — The teacher's key constraint (reduced fraction, no decimals) is not enforced. Type numfunc with "($num)/($den)" accepts 0.5 or 2/4 for 1/2; use a number answer with $answerformat="reducedfraction". The showplot use also looks wrong: `showplot(grid=..., $graph)` sits in the question text in backticks, and the "plot(...)"/"point(...)" strings are not valid showplot syntax (dots are "dot,x,y"). The graph likely won't draw the line and points as intended.
- `edge-select-all` / gemma4-31b: 3 — Truth values for statements 1-3 are fixed in the code rather than computed from the data: the mean and median statements are always true and the range statement is always false, so students can learn the pattern and the 'at least two true' rule is met by construction. Also, the mean is rounded to 2 decimals, but sum/8 can have 3 decimals (e.g. 12.375 shown as 12.38), so a 'true' statement is not exactly true; a bimodal set also makes 'has a mode of X' ambiguous.
- `edge-money` / gemma4-31b: 3 — The math is right (A = P(1+r/12)^(12t), rounded to the cent). The question text uses backticks as dollar-sign wrappers (`$`$pDisp`, `$`$answerbox), so there are 9 backticks in total. They pair up wrongly, which puts prose like " is invested at an annual interest rate of " into math mode and garbles the display. Use \$ for the literal dollar sign and put only $rDisp in backticks, or leave it plain. Also set $abstolerance = 0.01, because the default relative tolerance would accept answers that are off by more than a cent.
- `edge-equation` / gemma4-31b: 3 — The computed line is right, but the prompt has a stray `$` in `$p2$` that will print after the second point. Slopes are often non-terminating decimals (e.g. 7/3), and neither the prompt nor the worked solution says to enter exact fractions: the solution shows rounded decimals with ≈, so students may enter rounded values and be marked wrong.

## Problems left on failing runs

- `edge-dropdown` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-matrix` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-distractors` / gemma4-31b: Caught error while evaluating the code in this question: Unsupported operand types: int * string on line 36 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     35 | $questions = ar
