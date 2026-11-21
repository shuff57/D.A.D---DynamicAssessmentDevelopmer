# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| deepseek-v4.1-flash | 38/40 | 19/40 | 1.79 | 62 s | 4.21 | 33/40 | 37/38 | 37/38 | 36/38 | 0 | 0 | 78 | 0 |
| deepseek-v4.1-flash-think-low | 35/40 | 6/40 | 2.29 | 58 s | 4.23 | 29/40 | 34/35 | 33/35 | 33/35 | 0 | 0 | 74 | 1 |
| deepseek-v4.1-flash-think-off | 29/40 | 9/40 | 2.24 | 12 s | 3.00 | 7/40 | 22/29 | 20/29 | 18/29 | 0 | 0 | 0 | 1 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | deepseek-v4.1-flash | deepseek-v4.1-flash-think-low | deepseek-v4.1-flash-think-off |
|---|---|---|---|
| `bank-01` | pass, 1 try, judge 3 | pass, 2 tries, judge 5 | pass, 1 try, judge 2 |
| `bank-02` | pass, 4 tries, judge 4 | pass, 1 try, judge 4 | fail (1 left) |
| `bank-03` | pass, 3 tries, judge 5 | pass, 1 try, judge 3 | pass, 3 tries, judge 2 |
| `bank-04` | pass, 2 tries, judge 5 | pass, 3 tries, judge 5 | pass, 2 tries, judge 3 |
| `bank-05` | pass, 1 try, judge 4 | pass, 1 try, judge 2 | pass, 2 tries, judge 2 |
| `bank-06` | pass, 1 try, judge 2 | pass, 3 tries, judge 5 | pass, 1 try, judge 4 |
| `bank-07` | pass, 2 tries, judge 3 | pass, 3 tries, judge 4 | pass, 3 tries, judge 2 |
| `bank-08` | pass, 1 try, judge 4 | pass, 2 tries, judge 3 | pass, 3 tries, judge 3 |
| `bank-09` | pass, 2 tries, judge 4 | pass, 4 tries, judge 5 | pass, 2 tries, judge 3 |
| `bank-10` | pass, 2 tries, judge 4 | pass, 1 try, judge 5 | pass, 2 tries, judge 4 |
| `bank-11` | pass, 3 tries, judge 4 | pass, 2 tries, judge 4 | fail (1 left) |
| `bank-12` | pass, 1 try, judge 3 | fail (1 left) | pass, 3 tries, judge 2 |
| `bank-13` | pass, 1 try, judge 5 | pass, 2 tries, judge 4 | pass, 1 try, judge 4 |
| `bank-14` | pass, 1 try, judge 4 | pass, 2 tries, judge 5 | pass, 2 tries, judge 2 |
| `bank-15` | pass, 3 tries, judge 4 | pass, 2 tries, judge 3 | fail (1 left) |
| `bank-16` | pass, 1 try, judge 4 | pass, 3 tries, judge 5 | fail (1 left) |
| `bank-17` | pass, 1 try, judge 5 | pass, 4 tries, judge 4 | fail (1 left) |
| `bank-18` | fail (2 left) | pass, 3 tries, judge 5 | pass, 1 try, judge 4 |
| `bank-19` | pass, 3 tries, judge 5 | pass, 2 tries, judge 5 | pass, 3 tries, judge 3 |
| `bank-20` | pass, 2 tries, judge 4 | pass, 2 tries, judge 4 | pass, 3 tries, judge 2 |
| `bank-21` | pass, 1 try, judge 4 | pass, 1 try, judge 4 | fail (1 left) |
| `bank-22` | pass, 1 try, judge 5 | fail (6 left) | pass, 3 tries, judge 2 |
| `bank-23` | pass, 2 tries, judge 5 | pass, 3 tries, judge 4 | fail (1 left) |
| `bank-24` | pass, 1 try, judge 5 | pass, 2 tries, judge 5 | pass, 1 try, judge 4 |
| `bank-25` | pass, 2 tries, judge 5 | pass, 2 tries, judge 3 | pass, 1 try, judge 3 |
| `bank-26` | pass, 1 try, judge 5 | fail (4 left) | pass, 4 tries, judge 3 |
| `bank-27` | pass, 2 tries, judge 4 | pass, 3 tries, judge 5 | pass, 2 tries, judge 3 |
| `bank-28` | fail (2 left) | pass, 2 tries, judge 4 | pass, 1 try, judge 3 |
| `bank-29` | pass, 3 tries, judge 5 | pass, 2 tries, judge 5 | pass, 3 tries, judge 3 |
| `bank-30` | pass, 1 try, judge 4 | fail (2 left) | pass, 2 tries, judge 3 |
| `edge-picture` | pass, 1 try, judge 3 | fail (2 left) | pass, 4 tries, judge 4 |
| `edge-select-all` | pass, 1 try, judge 4 | pass, 3 tries, judge 5 | fail (29 left) |
| `edge-matching` | pass, 1 try, judge 5 | pass, 2 tries, judge 4 | pass, 1 try, judge 3 |
| `edge-dropdown` | pass, 1 try, judge 4 | pass, 2 tries, judge 4 | fail (1 left) |
| `edge-money` | pass, 2 tries, judge 4 | pass, 2 tries, judge 4 | pass, 3 tries, judge 3 |
| `edge-matrix` | pass, 1 try, judge 5 | pass, 4 tries, judge 3 | fail (1 left) |
| `edge-equation` | pass, 3 tries, judge 4 | pass, 4 tries, judge 4 | pass, 3 tries, judge 3 |
| `edge-distractors` | pass, 4 tries, judge 4 | pass, 2 tries, judge 5 | fail (2 left) |
| `edge-string` | pass, 3 tries, judge 5 | pass, 1 try, judge 5 | pass, 1 try, judge 5 |
| `edge-feedback-solution` | pass, 2 tries, judge 4 | pass, 2 tries, judge 4 | pass, 4 tries, judge 3 |

## Judge notes on questions scoring 3 or less

- `bank-01` / deepseek-v4.1-flash-think-off: 2 — Each $questions[i] array starts with the prompt sentence, so MyOpenMath treats it as choice 0 and the real choices are shifted. With $answer = 1, 0, 1 the key marks 'Yes' for (a), the prompt text for (b), and 'Yes' for (c), so every key is wrong. Also, in (c) f1 and f3 are independent random values and can be equal (about 1 in 9 seeds), which collapses the set to a function; make f3 differ from f1. Fix both, then it is fine.
- `bank-04` / deepseek-v4.1-flash-think-off: 3 — The margins are not constrained to be valid: with N=80, c1=90 or 100 gives a negative column total, and r1=80 or c1=80 (N=80) or c1=100 (N=100) gives a zero margin, so some seeds show impossible tables with negative cells. Fix by choosing r1 and c1 from values strictly between 0 and N, for example 20 to N-20. Also, at N=160 some expected counts such as 5.625 are not exact at 2 decimals, so the Step 3 sum of rounded values can be off by 0.01.
- `bank-03` / deepseek-v4.1-flash-think-off: 2 — In part (c), $answer[2] = 1, but choices are 0-indexed, so the key marks the second option ('not independent') as correct instead of the first (the double-counting explanation), and the solution guide contradicts the key. Set $answer[2] = 0, and replace $noshuffle = "questions" with a valid value (or remove it) so shuffling behaves predictably. Parts (a) and (b) compute correctly.
- `bank-07` / deepseek-v4.1-flash-think-off: 2 — The $plot string is built but never passed to showplot or included in the question text, so students see no scatterplot even though the question says "below". Also, the drawn line is the generating line y=slope*x+intercept, not a true least-squares line as the text claims, so say "a line" or compute the actual regression.
- `bank-05` / deepseek-v4.1-flash-think-off: 2 — In the choices parts, the first element of $questions[0] and $questions[2] is the prompt text, so it renders as an option and shifts the indices. The keys then point at the wrong choices: for (a), 0 and 1 mark the prompt and 'an observational study', and for (c), 0 marks the prompt instead of the justified conclusion. Part (b) is a free-response item but uses a 'string' anstype keyed to a whole sentence, so almost every sensible student answer would be marked wrong. It should be an essay type (manually graded) and the choices should be plain option lists.
- `bank-09` / deepseek-v4.1-flash-think-off: 3 — Negative exponents give decimal arguments, and base 3 produces long repeating decimals such as 0.037037037037, which hides the power of 3 and conflicts with 'evaluate exactly'; show them as fractions like 1/27 instead. Part (c) is also imprecise: 'a number less than 1' includes 0 and negatives, where the log is undefined, so the keyed answer is false as worded. It should say 'a positive number less than 1'.
- `bank-08` / deepseek-v4.1-flash-think-off: 3 — The grading logic and sum formulas are correct at all seeds, but the final essay part asks students to write an explanation, which goes against the request that they grade responses 'instead of writing one'. It also adds a hand-graded part with no answer key. Drop it or turn it into a choices part. Also, 'graded lowest' is ambiguous because D is the lowest grade but the highest letter.
- `bank-12` / deepseek-v4.1-flash-think-off: 2 — The six counts are drawn independently, so Camila's share in Table A and Table B will almost never match, yet part (c) and the solution both say the shares are the same. The code needs to make the tables proportional (e.g. Table B = k × Table A) so the shares truly match. It also never covers the 'more than three / boundary' idea from the request, and 'Camila, Rosa, Tomas' are named as water sources, which is confusing.
- `bank-03` / deepseek-v4.1-flash-think-low: 3 — The math, parts and answer types are right (answers are exact decimals, and the part c distractors are all false at every seed). However, the table's total cells hold unevaluated text such as "$nBonly + $nNeither" and "$nBoth + $nBonly", so students see "56 + 120" instead of a number. Compute those totals in the control code (e.g. $nNotA, $nNotB, $nB) and display them.
- `bank-01` / deepseek-v4.1-flash: 3 — The keyed answers and the worked solutions are right, but the third choice, 'It is a function only if every y-value is used at most once', is a muddled distractor. It is never the key, yet it is vacuously true for any non-function. It is also true for a function whose y-values are all distinct, where 'Yes' is keyed. Replace it with something clearly wrong or drop it, so the choices are plain Yes/No.
- `bank-05` / deepseek-v4.1-flash-think-low: 2 — The answer key is hard-coded as $designAns=0 (observational) and $conclAns=1 (association only), but scenario 1 is a randomized experiment. There the key marks 'observational' and 'no cause-and-effect' as correct, which contradicts the worked solution. It should be $designAns=1 and $conclAns=0 when $scen==1, so a third of seeds score correct student answers as wrong. Also, the request said free response but the question uses multiple choice, which is a defensible adaptation but worth confirming.
- `bank-14` / deepseek-v4.1-flash-think-off: 2 — The profit-regeneration loop can never succeed. On each constraint line the intersection lies strictly between the two axis intercepts, so a linear P cannot exceed both. The loop always burns all 200 tries and keeps arbitrary p1, p2, so at many seeds the true optimum is an axis vertex, not (xopt, yopt), and the keyed answers and the 'both constraints tight' solution text are wrong. Fix by choosing p1/p2 between a1/b1 and a2/b2, or by testing only the feasible vertices.
- `bank-19` / deepseek-v4.1-flash-think-off: 3 — The math, parts and answer types all match the request. However, the displayed line `\hat{y} = $m x + $b` shows as '3x + -5' whenever b is negative (about half of seeds). Build the sign into the text (for example, show '- 5' using abs($b)) before assigning.
- `bank-20` / deepseek-v4.1-flash-think-off: 2 — The teacher asked for a table of fractions and an equation, but the table shows decimals because prettyreal() turns values like 1/3 into rounded decimals, and the student enters only the slope m rather than the equation. Fix this by displaying fractions (for example with \frac or fractionformat) and asking for the equation, or by adding a separate b answer. The solution also shows decimals and negative b as '+ -0.67'.
- `bank-06` / deepseek-v4.1-flash: 2 — The signs in parts (c) and (d) are swapped. $zmid = invnorm(left area) is negative, but $answer[2] = -$zmid (positive) is graded for the 'lower, negative' bound, and $answer[3] = $zmid (negative) for the upper. The solution text repeats the error by reporting round(-$zmid) as the lower bound. Swap the two answers and the solution signs; parts (a) and (b) are fine.
- `bank-22` / deepseek-v4.1-flash-think-off: 2 — The key is inverted: choices are listed as ("Binomial", "Not binomial"), so Binomial is index 0, but $answer[$i] = $isBinom[...] stores 1 for binomial, so every binomial scenario is keyed "Not binomial" and vice versa (use 1 - $isBinom or flip the array). Also, scenario 9 (count who choose blue) is flagged binomial in $isBinom, but its explanation says "not binomial because many colors", which is wrong and contradicts the key, since blue/not blue is a two-outcome trial.
- `bank-25` / deepseek-v4.1-flash-think-off: 3 — Parts (a)-(d) are correct and on target (75, 88.89, 95.06, and the 'guaranteed minimum' choice). The stray '/// [QUESTION 2]' essay block was not requested. It reuses $answerbox[0], which duplicates the part (a) box, and the extra essay type added in the ANSWER section has no matching box ($answerbox[4]) in the text. Delete that block and the essay entry in $anstypes. Part (b) should also say whether to round.
- `bank-28` / deepseek-v4.1-flash-think-off: 3 — Part (b) is wrong: the prompt tells students to enter a negative number for the z term, but $answer[3] = $coefB3 = $p is positive, so a correct -p is marked wrong. Set it to -$p. Also, the intro says 'for any base $base and any real number $p', which plugs random values into the general rule and confuses it; use b and p. Part (e) repeats part (a).
- `bank-27` / deepseek-v4.1-flash-think-off: 3 — The math is right, but $answer[2] is the unrounded p and no tolerance is set, so the default 0.1% relative tolerance applies. A student who follows 'round to four decimal places' can be marked wrong. For example, k=2 gives 0.0228 against a key of 0.02275, and k=3 gives 0.0013 against 0.00135. Fix by adding $answer[2]=$p3 or a $abstolerance of about 0.0001 (and similarly for part b).
- `bank-29` / deepseek-v4.1-flash-think-off: 3 — The question text always lists $names[0] through $names[10], but $names was shrunk to only $n entries. When $n < 11 the list ends with empty slots (e.g. 'Gray, , , , .'), which is sloppy and confusing next to 'a group of $n people'. Build the list from the $n names, for example with implode(', ', $names), and the question will be ready. The combination counts and the (b)+(c)=(a) check are correct.
- `bank-26` / deepseek-v4.1-flash-think-off: 3 — The computations are right and all parts match the request, but the random numbers ignore context: the sleep context has a 40-95 hour intercept and predictions over 100 hours, exam scores can top 200 points, and car value can rise with age. The wording 'each additional unit of hours studied' is also awkward, so give each context its own realistic ranges and sign, and write 'each additional hour'.
- `bank-08` / deepseek-v4.1-flash-think-low: 3 — The sample sizes (n = 4, 9, 16, 25) are too small for a strongly right-skewed population, so the CLT does not justify 'approximately Normal'. The response keyed as fully correct, and the note that the shape is 'only approximately Normal', therefore overstate the shape. Use n of 30 or more (e.g. 36, 49, 64, 100 with roots 6, 7, 8, 10) or soften the skew description. The code is otherwise sound: each flawed response has exactly one error.
- `bank-30` / deepseek-v4.1-flash-think-off: 3 — The set is put inside \( \) as plain braces ({a, b}), and LaTeX treats those as invisible grouping. The student will see 'S = a, b' with no set braces, so the problem as shown is unclear. Build $setdisp with \{ and \} for the math display (e.g. \\{a, b\\}), and keep plain braces in the answer choices, which are outside math mode. The math and answer key are correct: 4 subsets, and choice 0 is the only list with all three non-empty subsets.
- `edge-matching` / deepseek-v4.1-flash-think-off: 3 — The teacher wants a sixth distractor description, but $answers holds only the 5 correct descriptions, and $matchset is not a standard matching variable, so the spread description will likely never appear and the last term is free by elimination. Fix by appending $descs[5] to $answers (so it has 6 entries). Also, the distractor 'a number that describes how spread out the values are' could loosely fit a parameter or statistic, so consider rewording it to something clearly neither, e.g. a description of a sampling method.
- `edge-money` / deepseek-v4.1-flash-think-off: 3 — The setup, formula, monthly compounding and worked solution are correct, but no $abstolerance or $reltolerance is set. MyOpenMath's default number tolerance is 0.1% relative, so answers off by several dollars on a $9,000 balance would be marked correct, which defeats 'rounded to the cent'. Add something like $abstolerance = 0.01. Minor polish: drop the redundant 'dollars' after the '$' in the answer line.
- `edge-equation` / deepseek-v4.1-flash-think-off: 3 — The answer is correct, but the worked solution prints raw decimals for fractional slopes and intercepts (e.g. 0.3333333333) and sign artifacts like 'y = -2x + -3' and '(3 - -2)'. Format the fractions and signs properly (e.g. with fraction() and sign handling), and tell students to enter exact fractions rather than rounded decimals.
- `edge-feedback-solution` / deepseek-v4.1-flash-think-off: 3 — The computed answer a*(b-c)^2 is right, but Step 1 of $solutionguide builds the substituted expression wrongly. It shows a(b) - (c)^2, e.g. 3(5) - (-2)^2, instead of a((b) - (c))^2, so the worked solution misstates the problem and contradicts Step 2. Fix the parentheses in $step1; also note b = c occurs about 10% of the time and gives a trivial answer of 0.
- `bank-07` / deepseek-v4.1-flash: 3 — Part c asks for 9 hours, but the data only span 1-8 hours, so the student is extrapolating, which an intro stats course would normally flag. There is also no plotted point at x=9, so 'use the line, not a plotted point' and the solution's 'nearest plotted point' remark do not do what was intended. Change the x value to one within 1-8, such as 4 or 6, where a point visibly sits off the line (the generator makes this easy since residuals are never 0).
- `bank-12` / deepseek-v4.1-flash: 3 — Parts a, b and d compute correctly at all 8 seeds: the shares are always equal (e.g. 8/16 = 12/24, 13/26 = 12/24), and moving the cut to 3 or more never ties. Part c is conceptually unsound. The keyed 'explanation' and the solution line 'the boundary, not the counts, is what explains the agreement' are wrong, because the shares match only since d/N happens to be equal in the data, not because of how 'more than 3' is defined. The keyed option just restates that shares are count over total. Rewrite c so the correct choice says the equality holds only at this cut point and the 3-tomato row differs in proportion, and fix the solution wording.
- `bank-15` / deepseek-v4.1-flash-think-low: 3 — The math checks out: the L, U and inverse formulas are right, det(A)=1, and B=AX is consistent with the stated answers. But $displayformat[1]="vector" conflicts with the wording "ordered triple" and the answer stored as (x1,x2,x3), so students may not know whether to type parentheses or angle brackets. Remove that line or change the wording to say vector.
- `bank-25` / deepseek-v4.1-flash-think-low: 3 — The teacher asked for the guaranteed share within 2, 3 and 4.5 standard deviations, but the code randomly draws three values from a pool of 1.5 to 5, so most seeds will not show those values. Part d also fixes k = 2 in its wording, which may not match any of parts a-c. Fix $k1, $k2 and $k3 to 2, 3 and 4.5 (or ask the teacher whether randomizing is intended). The math, the answer key, the tolerances and the part d distractors are otherwise correct.
- `edge-picture` / deepseek-v4.1-flash: 3 — The answer is correct at every seed and is always in lowest terms. But the text gives the coordinates of A and B, so the student can compute the slope without reading the graph, and the plot never marks A and B. Drop the coordinates and plot the two points so the student reads them off the graph. The example `-3/4` is also a possible answer, so use a different example.
- `edge-matrix` / deepseek-v4.1-flash-think-low: 3 — The 2x3 by 3x2 setup and the 2x2 answer are correct, and the step-by-step entries match the answer. However, the last line of $solutionguide puts $c11, $c12, $c21 and $c22 inside a single-quoted string, so the variables are probably not substituted and the solution would show them literally. Build that line by concatenation like the others, or use double quotes. Minor polish: negative products show as '3 + -4 + 5'.

## Problems left on failing runs

- `bank-02` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: syntax error, unexpected token ".", expecting end of file on line 37 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     36
- `bank-11` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: syntax error, unexpected token ".", expecting end of file on line 50 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     49
- `bank-15` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: syntax error, unexpected token ".", expecting end of file on line 66 of Common Control (seeds 4242, 7, 99)
- `bank-17` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: syntax error, unexpected token ".", expecting end of file on line 42 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     41
- `bank-16` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: syntax error, unexpected token ".", expecting end of file on line 48 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     47
- `bank-21` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: syntax error, unexpected token ".", expecting end of file on line 56 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     55
- `bank-23` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: syntax error, unexpected token "?", expecting end of file on line 82 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     81
- `edge-dropdown` / deepseek-v4.1-flash-think-off: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-select-all` / deepseek-v4.1-flash-think-off: part 0 (multans): duplicate options [""] - picking the other copy is marked wrong (seeds 4242, 7, 99)
- `edge-matrix` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: syntax error, unexpected token ".", expecting end of file on line 29 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     28
- `edge-distractors` / deepseek-v4.1-flash-think-off: part 0 (choices): no option matches the correct answer (seeds 4242, 7, 99)
- `bank-12` / deepseek-v4.1-flash-think-low: Caught error while evaluating the code in this question: syntax error, unexpected token ";" on line 48 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     47 | $questions[2] = arra
- `bank-18` / deepseek-v4.1-flash: part 1 (choices): no option matches the correct answer (seeds 4242, 7, 99)
- `bank-22` / deepseek-v4.1-flash-think-low: part 1 (choices): no option matches the correct answer (seeds 4242, 7, 99)
- `bank-26` / deepseek-v4.1-flash-think-low: part 1 (choices): no option matches the correct answer (seeds 4242, 7, 99)
- `bank-30` / deepseek-v4.1-flash-think-low: part 1 (multans): matched 0 of 0 correct options (seeds 4242, 7, 99)
- `bank-28` / deepseek-v4.1-flash: part 1 (choices): no option matches the correct answer (seeds 4242, 7, 99)
- `edge-picture` / deepseek-v4.1-flash-think-low: part 0: blank answer scored 1 (seeds 4242, 7, 99)
