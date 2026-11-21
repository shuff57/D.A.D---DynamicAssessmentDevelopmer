# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| gemma4-31b | 32/40 | 16/40 | 1.88 | 5 s | 3.00 | 7/40 | 26/32 | 19/32 | 26/32 | 0 | 0 | 0 | 0 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | gemma4-31b |
|---|---|
| `bank-01` | pass, 3 tries, judge 5 |
| `bank-02` | pass, 2 tries, judge 2 |
| `bank-03` | pass, 1 try, judge 2 |
| `bank-04` | fail (1 left) |
| `bank-05` | pass, 1 try, judge 2 |
| `bank-06` | fail (1 left) |
| `bank-07` | pass, 1 try, judge 4 |
| `bank-08` | pass, 1 try, judge 3 |
| `bank-09` | pass, 2 tries, judge 3 |
| `bank-10` | pass, 1 try, judge 4 |
| `bank-11` | fail (1 left) |
| `bank-12` | pass, 1 try, judge 2 |
| `bank-13` | pass, 1 try, judge 3 |
| `bank-14` | pass, 1 try, judge 2 |
| `bank-15` | fail (2 left) |
| `bank-16` | pass, 2 tries, judge 3 |
| `bank-17` | pass, 2 tries, judge 2 |
| `bank-18` | pass, 2 tries, judge 3 |
| `bank-19` | pass, 2 tries, judge 4 |
| `bank-20` | pass, 3 tries, judge 4 |
| `bank-21` | pass, 4 tries, judge 3 |
| `bank-22` | pass, 3 tries, judge 3 |
| `bank-23` | pass, 1 try, judge 3 |
| `bank-24` | pass, 1 try, judge 3 |
| `bank-25` | pass, 1 try, judge 4 |
| `bank-26` | pass, 1 try, judge 3 |
| `bank-27` | pass, 3 tries, judge 3 |
| `bank-28` | pass, 1 try, judge 3 |
| `bank-29` | fail (1 left) |
| `bank-30` | pass, 4 tries, judge 3 |
| `edge-picture` | pass, 3 tries, judge 3 |
| `edge-select-all` | fail (1 left) |
| `edge-matching` | fail (1 left) |
| `edge-dropdown` | pass, 1 try, judge 2 |
| `edge-money` | pass, 4 tries, judge 2 |
| `edge-matrix` | fail (1 left) |
| `edge-equation` | pass, 3 tries, judge 3 |
| `edge-distractors` | pass, 2 tries, judge 4 |
| `edge-string` | pass, 1 try, judge 3 |
| `edge-feedback-solution` | pass, 1 try, judge 3 |

## Judge notes on questions scoring 3 or less

- `bank-03` / gemma4-31b: 2 — The probabilities are generated with rand(0.3, 0.6) / 0.01, which gives values like 45 instead of 0.45, and $pAnd can reach into the thousands. Students would see 'probabilities' above 1, and P(A or B) can come out negative or greater than 1, so the answer is not a valid probability. Use rand(30,60)/100 for P(A) and P(B), and rand(10, round(100*min)) / 100 for the overlap. Also use 'number' instead of 'numfunc', and take the `$label` text out of the backtick math delimiters.
- `bank-02` / gemma4-31b: 2 — Because N is always 100, every percentage equals its count, so the two tables show identical numbers and the counts-versus-percentages contrast is lost. Table 2 also prints each percentage, so part (b) is just read off the table instead of computed. Vary N (e.g. 80, 125, 200) and show only one category's percentage or count so the group size has to be recovered by dividing.
- `bank-05` / gemma4-31b: 2 — The teacher asked for free response but all four parts are multiple choice. Part (d) is also wrong for the experimental vitamin scenario: the keyed answer, 'sample size was large enough', is not clearly unjustified, and the real unjustified conclusion (generalizing to teenagers) is not offered. The generic part (d) options, including '(if observational)', don't fit that scenario, and that scenario has no bias or confounding to name.
- `bank-08` / gemma4-31b: 3 — The key for Student D (No Credit) contradicts the stated rubric: D has two of four elements correct (shape, context), and the rubric says Partial Credit is for 'some but not all elements correct.' Student A also has two of four correct and is keyed Partial, so the rubric needs a defined cutoff (e.g., 'No Credit if both mean and SD are wrong') or D should be keyed Partial. Minor: mean dog weights of 140-180 lb are unrealistic.
- `bank-09` / gemma4-31b: 3 — Part 1 is correct and well scoped, but Part 2 is graded as an exact string. The prompt shows the format 'base^exponent = value' with spaces, while the key is '2^-3=1/8' with no spaces, and trim_whitespace only trims the ends, so a correct answer typed in the format shown is marked wrong. Negative exponents also have no stated format ('2^-3' vs '2^(-3)'). Drop Part 2, or make it a multiple-choice question or use ignore_spaces with a stated format.
- `bank-12` / gemma4-31b: 2 — Both tables already display the 'More than 3%' relative frequencies (0.3 and 0.3), so parts (a) and (b) only ask students to read the answer off the table instead of computing count/total from frequency tables as the request implies. Fix by showing raw counts in the tables, and consider clearer, non-overlapping category labels (the '2% to 3%' and 'Exactly 3%' rows overlap).
- `bank-13` / gemma4-31b: 3 — The Step 3 line of $solutionguide opens a backtick before the answer[1] - answer[0] subtraction and never closes it, so the solution will render as broken math. Also, the question never says to round to the nearest cent. Part (c) is computed from unrounded balances, so a student subtracting the rounded (a) and (b) can be a cent off, and the default tolerance is tight for a small difference; set an explicit tolerance such as $abstolerance = 0.01.
- `bank-14` / gemma4-31b: 2 — The code assumes the optimum is always where the wax and scent constraint lines intersect, but that is only true for some profit/constraint combinations. For example, with profits 25 and 20 and constraints 2x+3y<=T1 and x+2y<=T2, the optimum is at an axis vertex, so the keyed answer is wrong at many seeds. The swap logic and the negative-intersection fallback also don't guarantee a feasible intersection, and the denominator can be 0. Answers are often fractional candle counts with no rounding instruction, and 'numfunc' is the wrong answer type for plain numbers. Fix by computing all corner points and taking the true maximum, or by designing the parameters so the intersection is provably optimal and integer.
- `bank-16` / gemma4-31b: 3 — The prompts are always in the same order as the first five options (SRS, Stratified, Cluster, Systematic, Convenience), so students can match by position without reading the scenarios; shuffle the prompts or options. Also, the systematic scenario 'every 15th person at a mall entrance' is arguably convenience sampling, which would confuse students, so reword it or drop it.
- `bank-18` / gemma4-31b: 3 — The math is right at every seed (t_eval stays inside the domain, and the AROC and quadratic root are correct). But part (b) never tells students to round, while the key is rounded to 2 decimals and no tolerance is set, so an unrounded answer like 3.1944 can be marked wrong against 3.19. Add 'round to two decimal places' and set a tolerance. Also consider using 'number' instead of 'numfunc', and having students find the domain rather than stating it in the prompt.
- `bank-17` / gemma4-31b: 2 — In the below-lower-fence outlier branch, max(0, ...) clamps the test value to 0, but the lower fence can be zero or negative (e.g., Q1=10, IQR=8 gives -2), so 0 is inside the fences while the key says 'outlier' (and the solution says it falls outside the range). The 'inside the fences' branch can also give negative commute times. Fix by constraining Q1 > 1.5*IQR (so the lower fence is positive) and drawing the inside value from a positive range.
- `bank-21` / gemma4-31b: 3 — The class labels in the table are broken. Text like ` . ($classStarts[0] + $classWidth - 1) . ` is PHP concatenation inside question text, where only $variables are substituted, so students will see a garbled expression instead of the upper limit. Compute the upper limits in the control section (e.g. $classEnds[$i]) and write `$classStarts[0]` to `$classEnds[0]` in the table. The frequencies, total, and the correct choice (index 1) are right, and the stray '(f)' label should be dropped or the parts lettered.
- `bank-23` / gemma4-31b: 3 — The answer logic is correct, but the interval is built with two independent random margins (mean-rand(5,12), mean+rand(5,12)), so it is usually asymmetric, which a real confidence interval for a mean never is; use one shared margin. Also, rand(10,100)/100 can put the null value exactly on the upper endpoint, which is a borderline case. The null value is also an integer when outside the interval but usually a decimal when inside, which hints at the answer. Finally, the question never says the test is two-sided with alpha = 1 - confidence level.
- `bank-22` / gemma4-31b: 3 — The scenario text is wrapped in backticks, which MyOpenMath treats as ASCIIMath delimiters, so each sentence will render as mangled math; remove the backticks. Also check that jointrandfrom(0,1,2,3,4,5,4) really gives 4 distinct indices, since that is not its documented use and diffrandsfrom or a shuffle slice is safer. The classifications and reasons themselves are correct.
- `bank-24` / gemma4-31b: 3 — The teacher asked for a free-response question, but all three parts are multiple choice, and parts (b) and (c) test nearly the same idea. Text variables such as `$item`, `$groupA` and `$unit` are wrapped in backticks, so AsciiMath will render phrases like 'daily commute times' as run-together math italics. The `$\mu$` and `$\sigma$` headers will also display literal dollar signs. The keyed answers are correct.
- `bank-26` / gemma4-31b: 3 — The context is unrealistic: the code comments say scores run 0 to 100, but with rand(2,5) slopes, rand(40,60) intercepts and 5 to 15 hours, predictions reach 135 (e.g. 5*15+60), which is impossible on a 100-point exam and teaches the wrong idea about prediction. Cap the parameters (e.g. slope 2 to 4, intercept 40 to 50, hours 5 to 12, or check $pred_score <= 100). Also, part (a) should use the 'number' answer type rather than 'numfunc', and $eq_text already contains backticks, so wrapping it again in backticks in the question text may break the math rendering.
- `bank-27` / gemma4-31b: 3 — The math is right, but the question never says how to round, and no $reltolerance or $abstolerance is set. With the default 0.1% relative tolerance, a student who enters an unrounded probability such as 0.02275 is marked wrong against the keyed 0.0228. Using the 3-decimal rounded z also shifts the answer beyond tolerance. Add 'round to 4 decimal places' (z to 3) and set an absolute tolerance of about 0.0005 for (d). Minor: write the subscripts as `mu_(Sigma x)` so they render correctly, and consider n >= 30 for the CLT.
- `bank-28` / gemma4-31b: 3 — Part (a) uses an exact 'string' match on "3 * log_2(5)", so equivalent entries like 3*log_2(5), 3log_2(5) or log_2(5)*3 are marked wrong. The random x can also be a power of b (for example log_2(4^3) or log_10(10^4)), so students may reasonably evaluate it to an integer and be marked wrong. Part (b) should use 'number' rather than 'numfunc', and the solution has a stray '$' after $k.
- `edge-picture` / gemma4-31b: 3 — The teacher said the student must enter a fraction, not a decimal, but nothing enforces it. It uses numfunc with a numeric $answer and no $answerformat, so 0.5 or -0.75 would be marked correct. Switch to a calculated type with $answerformat='reducedfraction'. Minor: integer slopes such as 3 show as '3/1' in the solution.
- `bank-30` / gemma4-31b: 3 — The answers (3, {e1}, {e2}, {e1, e2}) are correct and the ordering rule is stated. However, in $solutionguide the first `S = {` . $e1 . `, ` . $e2 . `}` sits inside a single-quoted string, so students will see the literal text ". $e1 ." instead of the set. Close and reopen the quotes there. Also consider the remove_whitespace flag instead of trim_whitespace, so inner spacing like {A,  B} is accepted.
- `edge-dropdown` / gemma4-31b: 2 — Part (a) uses an exact-match string answer built from $sol_val = ($c-$b)/$a, which is usually a repeating decimal (e.g. 2.3333333333333). A student who correctly enters (-inf, 7/3) or any rounded decimal is marked wrong, and the code comment says 'fraction' but the answer is a decimal. Students are also not told the format (-inf vs -oo, spacing, fraction vs decimal). Use the interval answer type, or choose values so a divides c-b, and tell students how to enter infinity.
- `edge-equation` / gemma4-31b: 3 — The question text has a stray `$` after `$p2` (written as `$p2$`), so a dollar sign shows after the second point. The solution guide also rounds fractional slopes and intercepts to decimals (e.g. y = 0.333x + 0.667) and prints 'x + -2' for negative b, so it shows a non-exact answer where students would enter fractions.
- `edge-string` / gemma4-31b: 3 — The teacher asked for students to type the answer with case-insensitive matching, but the file is set to 'choices' (multiple choice with $questions and an index $answer). It needs to become a string-type question with $answer = "observational" or "experiment" and case-insensitive matching (e.g. $strflags = "ignore_case=1"). The six scenarios are classified correctly and the solution guide is accurate.
- `edge-money` / gemma4-31b: 2 — The math is right: A = P(1+r/12)^(12t), rounded to cents, with sensible parameter ranges. But the dollar-sign pattern `$`$p_display` leaves a stray backtick, so backticks are unbalanced in the question text and in the solution (e.g. the principal line and 'we get' line). The pairing shifts and the text between ('is invested in an account... rate of') renders as garbled math. Write it as `$`$p_display with no trailing backtick, or use \$, in both places.
- `edge-feedback-solution` / gemma4-31b: 3 — The backticks in the question text are mismatched: `$a(`$b` - `$c`)^2` puts $b and $c outside the math delimiters and splits the expression into pieces, so the typesetting will be inconsistent and the parentheses may render oddly. Wrap the whole expression in one pair of backticks, e.g. `$a($b - $c)^2`. The answer (a*(b-c)^2, with b-c always positive) and the three-step solution guide are correct.

## Problems left on failing runs

- `bank-06` / gemma4-31b: Caught error while evaluating the code in this question: Unsupported operand types: float * string on line 21 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     20 | // Since $rig
- `bank-04` / gemma4-31b: Caught error while evaluating the code in this question: syntax error, unexpected token ";" on line 87 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     86 |   $part1_col = $col_
- `bank-11` / gemma4-31b: Eeek.. unallowed macro shuffle (seeds 4242, 7, 99)
- `bank-15` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `bank-29` / gemma4-31b: Caught error while evaluating the code in this question: Unsupported operand types: int * string on line 7 of Common Control (seeds 4242, 7, 99)
  The code around that line:
      6 | }
  >   7 | 
   
- `edge-select-all` / gemma4-31b: Eeek.. unallowed macro implode (seeds 4242, 7, 99)
- `edge-matching` / gemma4-31b: part 0: correct answer scored 0.833 (seeds 4242, 7, 99)
- `edge-matrix` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
