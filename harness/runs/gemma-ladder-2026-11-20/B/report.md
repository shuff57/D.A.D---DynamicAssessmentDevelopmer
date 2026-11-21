# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| gemma4-31b | 35/40 | 16/40 | 1.74 | 5 s | 3.46 | 16/40 | 29/35 | 24/35 | 28/35 | 0 | 0 | 0 | 10 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | gemma4-31b |
|---|---|
| `bank-01` | pass, 1 try, judge 4 |
| `bank-02` | pass, 2 tries, judge 2 |
| `bank-03` | pass, 2 tries, judge 3 |
| `bank-04` | pass, 3 tries, judge 3 |
| `bank-05` | fail (2 left) |
| `bank-06` | pass, 2 tries, judge 4 |
| `bank-07` | pass, 3 tries, judge 3 |
| `bank-08` | pass, 2 tries, judge 4 |
| `bank-09` | pass, 1 try, judge 4 |
| `bank-10` | pass, 2 tries, judge 5 |
| `bank-11` | pass, 1 try, judge 5 |
| `bank-12` | pass, 2 tries, judge 3 |
| `bank-13` | pass, 2 tries, judge 4 |
| `bank-14` | pass, 1 try, judge 2 |
| `bank-15` | pass, 1 try, judge 3 |
| `bank-16` | pass, 1 try, judge 4 |
| `bank-17` | pass, 2 tries, judge 2 |
| `bank-18` | pass, 1 try, judge 3 |
| `bank-19` | pass, 1 try, judge 5 |
| `bank-20` | pass, 1 try, judge 3 |
| `bank-21` | pass, 4 tries, judge 4 |
| `bank-22` | pass, 3 tries, judge 3 |
| `bank-23` | pass, 2 tries, judge 4 |
| `bank-24` | pass, 1 try, judge 3 |
| `bank-25` | pass, 1 try, judge 4 |
| `bank-26` | pass, 1 try, judge 4 |
| `bank-27` | pass, 1 try, judge 3 |
| `bank-28` | pass, 2 tries, judge 3 |
| `bank-29` | fail (1 left) |
| `bank-30` | pass, 1 try, judge 3 |
| `edge-picture` | pass, 2 tries, judge 3 |
| `edge-select-all` | pass, 4 tries, judge 2 |
| `edge-matching` | pass, 2 tries, judge 5 |
| `edge-dropdown` | fail (1 left) |
| `edge-money` | pass, 2 tries, judge 4 |
| `edge-matrix` | fail (1 left) |
| `edge-equation` | pass, 2 tries, judge 3 |
| `edge-distractors` | fail (4 left) |
| `edge-string` | pass, 1 try, judge 3 |
| `edge-feedback-solution` | pass, 1 try, judge 4 |

## Judge notes on questions scoring 3 or less

- `bank-03` / gemma4-31b: 3 — The math and the part (b) distractors are sound, but the stem always says 'randomly selected person is in the group of...', which makes no sense for the car context ('person in the group of cars that are red'). Also round $pOr to 2 decimals before printing it in $solutionguide to avoid float artifacts. Fix the car context or make the stem wording context-specific.
- `bank-02` / gemma4-31b: 2 — The count generator only redraws once, so at many seeds the last count is below 5 or even negative (e.g. total 50, three draws up to 30 each), giving negative counts and percentages. Also, Table 2 already shows every category's percentage, so part (b) is just read off the table and no bar graphs are drawn; fix by generating counts that always sum correctly and by hiding the target percentage.
- `bank-04` / gemma4-31b: 3 — The expected counts are right, but no tolerance is set and the text says to round to two decimals, so a correctly rounded answer can be marked wrong. For small cells (minimum about 1.125, or 1.5 and 3.333 repeating at other totals) the default 0.1% relative tolerance is tighter than rounding error, so set $abstolerance=0.01. Also, the labels and cell names are wrapped in backticks, which render as asciimath (italic spaced letters), and cells named 'Row 1, Column 1' would read better as the actual labels, e.g. 'Treatment, Success'.
- `bank-07` / gemma4-31b: 3 — The request was for a scatterplot with a line, but the plot shows only dots and no line (such as a fitted line) for judging the trend. The axes also have no labels or ticks, and when m=1 the noise of ±2 is large relative to the y-range (about 9) after auto-scaling, so the linear case can look ambiguous.
- `bank-12` / gemma4-31b: 3 — The math is sound: the construction guarantees exactly 7 of 20 values above the boundary in each table, so 0.35 and 'same share' are always right. However, the request says 'more than three' and 'the boundary explains why', but the question uses a random boundary of 15-50 instead of 3 and never asks about or explains the role of the boundary (the solution just counts). Use 3 as the threshold, or confirm the randomization is intended, and add a boundary-based explanation.
- `bank-14` / gemma4-31b: 2 — At some seeds the code can return an infeasible 'optimal' point. The intersection of the two constraint lines is used without checking that it lies in the first quadrant. The fallback to 500/500 resources doesn't fix this: whenever det>0, xInt is still ≤0, so the negative-x point can win the profit comparison. Also, answers are often fractional (e.g. 16.67 candles), and the text never says whether fractions are allowed or whether to round. Fix by re-rolling until det<0 and the intersection is feasible, and choose values that give integer vertices.
- `bank-15` / gemma4-31b: 3 — The math is right: A = LU has det 1, the cofactors and transpose give the correct A^{-1}, and B = A*X makes X = A^{-1}B the intended integer vector. However, there is no $answersize (3,3 and 3,1), so the calcmatrix parts won't give grid boxes and students won't know what format to type. Also, the backtick AsciiMath wrapped around \begin{pmatrix} should be changed to proper LaTeX delimiters.
- `bank-17` / gemma4-31b: 2 — In the 'not outlier' branch, obs = rand($min,$max) can fall outside the fences, because min can sit more than 1.5*IQR below Q1 (e.g. Q1=20, IQR=4, min=10, obs=12) and max can likewise exceed the upper fence. The key then says 'No' for a true outlier; fix by drawing obs between the fences, and also keep min/max inside the fences or state that they are outliers. Fences can also go negative for heights, weights, etc.
- `bank-18` / gemma4-31b: 3 — t_eval can be 3, and for smaller v0 (about 10-14 with small h0) the ball has already landed by then, so part (a) gives a negative height and part (c)'s interval [1,3] extends past the domain computed in (b); pick t_eval/v0 so t_eval < t_end or compute the interval from t_end. Part (b) also never tells students to round the endpoint to 2 decimals. Separately, the $solutionguide for part (c) mixes PHP concatenation into a quoted string, so literal 'round(...)' text will show.
- `bank-20` / gemma4-31b: 3 — The table, the answer, and the task all match the request, and the numfunc answer is correct at every seed. The $solutionguide is broken, though: the string concatenation is written inside one double-quoted string, so students will see literal text like `(` . -1 . `, ` . 1/2 . `)` with stray periods and backticks. Step 2 never actually solves for b, and Step 3 can show '+ -1/2'. Fix the string building and show the work for b.
- `bank-22` / gemma4-31b: 3 — The pool scenario about sampling 50 of 500 people without replacement is keyed 'Not Binomial', but 50 is exactly 10% of the population, so under the usual 10% rule many courses treat it as approximately binomial. Change it to a clearly small population (e.g., 40 of 100) or a clearly large one. Also, the scenario text is wrapped in backticks, which MyOpenMath renders as ASCIIMath, so the sentences may display garbled; print `$prompt_text[n]` as plain text. Minor: the solution reasons begin mid-sentence with a capital letter ('because The trials...').
- `bank-24` / gemma4-31b: 3 — Part (a) is an exact-match string answer ("the same"), so correct responses like "same", "equal" or "identical" will be marked wrong, and a dropdown or choices type would fix this. Other edits are also needed: the contexts are unrealistic (means of 60-95 steps, hours of battery life or minutes of commute, and battery life "of students"), the correct group is always Group A so the answers are guessable, and parts (b) and (c) ask essentially the same thing. The `$label` and group names inside backticks will also render as math.
- `bank-27` / gemma4-31b: 3 — The question never says how to round, yet the key is stored to 4 decimals, so a student who gives a 3-decimal probability (e.g. 0.012 for 0.0123) may be marked wrong. n goes as low as 15 and the population shape is never given, so the normal model for the sum needs a statement that the population is normal or n must be at least 30. The calculation itself (1 - Φ(z) with mean nμ and SD σ√n) is correct, but the stray extra arguments in normalcdf(z,10,0) should be removed.
- `bank-28` / gemma4-31b: 3 — In MyOpenMath, log(x) is base 10 and ln(x) is the natural log, so when b is e the key p*log(x) is really p*log10(x) and rejects the correct p*ln(x). The question also shows 'log_e' instead of 'ln' and never tells students how to type the expanded form, such as p*log_3(5). Part (b) is correct.
- `edge-picture` / gemma4-31b: 3 — The teacher required a fraction (not a decimal), but numfunc accepts decimals like 0.5 and unreduced forms like 2/4. Switch to a number answer type with $answerformat='reducedfraction' (and give $answer as the reduced value) so decimals and non-lowest-terms answers are rejected. Minor polish: the solution guide prints things like '5 - -3' and never states the simplified slope.
- `bank-30` / gemma4-31b: 3 — The math is right (3 non-empty subsets: {e1}, {e2}, {e1, e2}), but part (b) is a brittle exact-string match. Only the 12 permutations with exactly ", " spacing are accepted, so a correct answer like {a},{b},{a,b} is marked wrong, and the student isn't told the format. Add remove_whitespace to $strflags[1], or use separate boxes, and tell students to write the subsets in braces separated by commas.
- `edge-select-all` / gemma4-31b: 2 — Several seeds mark statements true or false wrongly. The mean is rounded to 2 decimals, so when the sum is odd (true mean x.125, x.375, etc.) the statement 'mean is 10.13' is marked true though the mean is 10.125. When mean equals median or range equals median, the forced-true branches in statements 5 and 6 still print 'less than' or 'smaller than', which is false. In the roughly 20% of seeds with no repeated value, every value is a mode (or there is no mode), yet only the smallest is scored as correct. Fix these by using exact or stated-rounding means, handling ties, and requiring a repeated value.
- `edge-string` / gemma4-31b: 3 — The teacher asked for a typed answer (accepting either capitalization), but the file is set to 'choices' with a $questions list and index answer, so students pick from options instead of typing. The scenario answers are all correct, so the fix is to switch to a string type with case-insensitive matching (e.g. $anstype='string', $answer='experiment' or 'observational study', ignore_case).
- `edge-equation` / gemma4-31b: 3 — The answer is computed correctly, and numfunc accepts equivalent equations. Random points often give non-terminating slopes and intercepts (e.g. 2/3), but the prompt never says to enter exact fractions. A student who types a rounded decimal like 0.67 may be marked wrong. The solution also shows rounded decimals (y = 0.667x + 1.333) instead of exact fractions, and it prints ugly signs such as 'x - -3'. Add an instruction to use exact values and show the solution as fractions. The nested backticks around $p1 in the question text are also fragile.

## Problems left on failing runs

- `bank-05` / gemma4-31b: Caught error while evaluating the code in this question: array_search(): Argument #2 ($haystack) must be of type array, string given on line 45 of array.php (seeds 4242, 7, 99)
- `bank-29` / gemma4-31b: Caught error while evaluating the code in this question: Unsupported operand types: int * string on line 8 of Common Control (seeds 4242, 7, 99)
  The code around that line:
      7 | // we ensure all
- `edge-dropdown` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-matrix` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-distractors` / gemma4-31b: Caught error while evaluating the code in this question: count(): Argument #1 ($value) must be of type Countable|array, int given on line 609 of randomizers.php (seeds 4242, 7, 99)
