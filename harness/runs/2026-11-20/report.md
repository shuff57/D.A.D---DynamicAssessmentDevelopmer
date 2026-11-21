# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Matches request | Clear | Math right |
|---|---|---|---|---|---|---|---|---|
| deepseek-v4.1-flash | 40/40 | 18/40 | 1.73 | 58 s | 4.20 | 38/40 | 38/40 | 37/40 |
| gemma4-31b | 32/40 | 19/40 | 1.66 | 4 s | 3.22 | 28/32 | 20/32 | 22/32 |
| claude-sonnet-5.5 | 40/40 | 14/40 | 1.85 | 80 s | 4.85 | 39/40 | 40/40 | 40/40 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | deepseek-v4.1-flash | gemma4-31b | claude-sonnet-5.5 |
|---|---|---|---|
| `bank-01` | pass, 2 tries, judge 3 | fail (3 left) | pass, 1 try, judge 5 |
| `bank-02` | pass, 2 tries, judge 4 | fail (1 left) | pass, 1 try, judge 5 |
| `bank-03` | pass, 1 try, judge 3 | pass, 1 try, judge 4 | pass, 2 tries, judge 5 |
| `bank-04` | pass, 1 try, judge 5 | fail (1 left) | pass, 1 try, judge 5 |
| `bank-05` | pass, 1 try, judge 4 | pass, 1 try, judge 2 | pass, 1 try, judge 3 |
| `bank-06` | pass, 1 try, judge 3 | fail (1 left) | pass, 1 try, judge 5 |
| `bank-07` | pass, 1 try, judge 4 | pass, 2 tries, judge 4 | pass, 1 try, judge 5 |
| `bank-08` | pass, 1 try, judge 5 | pass, 1 try, judge 2 | pass, 2 tries, judge 5 |
| `bank-09` | pass, 1 try, judge 5 | pass, 1 try, judge 5 | pass, 1 try, judge 5 |
| `bank-10` | pass, 2 tries, judge 5 | pass, 1 try, judge 4 | pass, 3 tries, judge 5 |
| `bank-11` | pass, 2 tries, judge 5 | pass, 1 try, judge 4 | pass, 2 tries, judge 5 |
| `bank-12` | pass, 2 tries, judge 4 | pass, 1 try, judge 3 | pass, 2 tries, judge 4 |
| `bank-13` | pass, 4 tries, judge 4 | pass, 2 tries, judge 3 | pass, 4 tries, judge 5 |
| `bank-14` | pass, 2 tries, judge 3 | pass, 1 try, judge 2 | pass, 2 tries, judge 5 |
| `bank-15` | pass, 2 tries, judge 4 | fail (2 left) | pass, 2 tries, judge 5 |
| `bank-16` | pass, 1 try, judge 4 | pass, 1 try, judge 2 | pass, 2 tries, judge 5 |
| `bank-17` | pass, 3 tries, judge 5 | pass, 1 try, judge 3 | pass, 2 tries, judge 5 |
| `bank-18` | pass, 1 try, judge 4 | pass, 3 tries, judge 3 | pass, 2 tries, judge 5 |
| `bank-19` | pass, 1 try, judge 5 | pass, 1 try, judge 3 | pass, 1 try, judge 5 |
| `bank-20` | pass, 2 tries, judge 4 | pass, 3 tries, judge 4 | pass, 2 tries, judge 5 |
| `bank-21` | pass, 3 tries, judge 4 | pass, 3 tries, judge 3 | pass, 2 tries, judge 5 |
| `bank-22` | pass, 1 try, judge 5 | pass, 3 tries, judge 3 | pass, 1 try, judge 5 |
| `bank-23` | pass, 2 tries, judge 5 | pass, 1 try, judge 3 | pass, 2 tries, judge 5 |
| `bank-24` | pass, 2 tries, judge 4 | pass, 2 tries, judge 3 | pass, 1 try, judge 5 |
| `bank-25` | pass, 1 try, judge 5 | pass, 1 try, judge 4 | pass, 3 tries, judge 4 |
| `bank-26` | pass, 3 tries, judge 4 | pass, 1 try, judge 2 | pass, 2 tries, judge 5 |
| `bank-27` | pass, 1 try, judge 4 | pass, 3 tries, judge 3 | pass, 2 tries, judge 5 |
| `bank-28` | pass, 2 tries, judge 5 | pass, 1 try, judge 3 | pass, 2 tries, judge 5 |
| `bank-29` | pass, 2 tries, judge 5 | pass, 1 try, judge 4 | pass, 3 tries, judge 5 |
| `bank-30` | pass, 2 tries, judge 4 | fail (1 left) | pass, 2 tries, judge 5 |
| `edge-picture` | pass, 3 tries, judge 4 | pass, 4 tries, judge 2 | pass, 3 tries, judge 5 |
| `edge-select-all` | pass, 1 try, judge 3 | fail (1 left) | pass, 1 try, judge 5 |
| `edge-matching` | pass, 2 tries, judge 4 | pass, 3 tries, judge 5 | pass, 1 try, judge 5 |
| `edge-dropdown` | pass, 2 tries, judge 4 | pass, 1 try, judge 2 | pass, 2 tries, judge 5 |
| `edge-money` | pass, 1 try, judge 2 | pass, 2 tries, judge 4 | pass, 2 tries, judge 4 |
| `edge-matrix` | pass, 1 try, judge 4 | fail (1 left) | pass, 2 tries, judge 5 |
| `edge-equation` | pass, 1 try, judge 5 | pass, 2 tries, judge 3 | pass, 3 tries, judge 5 |
| `edge-distractors` | pass, 2 tries, judge 5 | pass, 2 tries, judge 4 | pass, 1 try, judge 5 |
| `edge-string` | pass, 1 try, judge 5 | pass, 1 try, judge 4 | pass, 1 try, judge 5 |
| `edge-feedback-solution` | pass, 3 tries, judge 4 | pass, 1 try, judge 3 | pass, 3 tries, judge 4 |

## Judge notes on questions scoring 3 or less

- `bank-05` / gemma4-31b: 2 — Parts (b)-(d) are graded as exact-match strings (e.g. 'self-reporting bias or diet or sleep habits'), so almost any correct free-response answer would be marked wrong. These parts should be essay/manual-graded or use a rubric. Some content is also weak: 'placebo effect' for the blueberry vs cranberry experiment doesn't fit, and 'blueberries are the only way' is a trivial non-justified conclusion.
- `bank-08` / gemma4-31b: 2 — The answer key is wrong for Student 3: choices are 0-indexed, so $ans2 = 2 marks the correct full-credit response as "No Credit" (it should be 0). Separately, the rubric never says what separates Partial from No Credit, and the problem never says the flour-bag weights are normally distributed, so "approximately normal" for n = 5 is unjustified.
- `bank-03` / deepseek-v4.1-flash: 3 — The table's 'Not over 50' Total cell and the 'No flu shot' Total cell use '$neither + $shotNotOver' and '$overNotShot + $neither', which print as unevaluated sums like '12 + 30' instead of single numbers. Compute these as variables (e.g. $notOverTotal = 100 - $overTotal; $noShotTotal = 100 - $shotTotal) and use those in the table. The math, answers, and distractors are otherwise correct and match the request.
- `bank-12` / gemma4-31b: 3 — Camila's c4 and c5 are each rand(5,10), so c6 = 18 - c4 - c5 can be 0 (about 8% of seeds) or negative (-1 or -2, about 8% of seeds), which gives an impossible negative relative frequency in the table. The code comment says 'with 5-10, 18 is safe', which is wrong. Narrowing the range, e.g. rand(3,7) for c4 and c5, keeps c6 at 4 or more.
- `bank-13` / gemma4-31b: 3 — The math is right: parts (a) and (b) use A=P(1+r/n)^(nt) with n=1 and n=12, and (c) is the difference of the balances. But the text never says to round to the nearest cent, and the deposit reads "deposits 3500" with no dollar sign. Also, $answerbox sits inside backticks (math mode), and the default 0.1% tolerance accepts answers several dollars off, so consider setting $abstolerance=0.01.
- `bank-14` / gemma4-31b: 2 — The code reports the wax/scent intersection as the optimum, but that is only optimal when profitA/profitB lies between 1/2 and 2/3. Profits are randomized so the ratio runs from about 0.33 to 1.33; for example A=$20, B=$15 gives a better profit at (wax/2, 0), so parts (b)-(d) are wrong at many seeds. Fix by choosing profits that keep the ratio in range, or by computing the best vertex. Part (a) uses a string answer, which will mark equivalent forms (such as reordered terms) wrong, so use an expression/equation type instead.
- `bank-16` / gemma4-31b: 2 — The five core matches are correct, but the added sixth prompt is keyed as -1 ('no match') even though no 'none' option exists and the directions never mention one. The volunteer-survey variant is generally classed as convenience (voluntary response) sampling, so the key is wrong or ambiguous on half the seeds. Remove the distractor, or add a 'None of these' option and use an unambiguous non-example.
- `bank-17` / gemma4-31b: 3 — The IQR, fences and outlier decision are computed correctly, but the lower fence is often negative (Q1 can be as low as 15 while 1.5*IQR reaches 45). The new observation is then frequently negative, e.g. -25 pounds, pages or minutes, which is impossible in three of the four contexts. Keep the contexts positive, for example by requiring Q1 > 1.5*IQR or by making low outliers possible only when the fence is positive. The max can also exceed the upper fence when IQR is small, so the summary already contains an outlier.
- `bank-01` / deepseek-v4.1-flash: 3 — Part d's distractor "The relation has more outputs than inputs" is not a valid wrong answer. A function cannot have more distinct outputs than inputs, so that condition does force a non-function and is arguably also correct. The solution's claim that such relations are "still functions" is false, so replace the distractor with something like "Every output is used only once." Minor polish: set $noshuffle so the two function/not-function choices keep a fixed order. "Each ZIP code is paired with the state it is in" is slightly shaky because a few ZIP codes cross state lines.
- `bank-19` / gemma4-31b: 3 — Option 4 ("perfect fit for this point") is equivalent to option 3, so when the residual is 0 (possible, e.g. m=2.0) two choices are correct but only one is keyed. The sign test also uses the unrounded float residual, so values like 1.1*30 can give a tiny nonzero residual and key "below" when the true residual is 0. Also, m up to 3 with x up to 50 gives ŷ up to about 170 against y at most 100, so the line is rarely "close" and the residual is almost always negative. Use answer type "number" instead of "numfunc", and drop the redundant distractor.
- `bank-18` / gemma4-31b: 3 — The math is right, but part (b) never tells students to round to 2 decimals, and it is graded as an exact string match against "[0, t_max]". Valid answers like "[0,5.2]" or "[0, 5.20]" or an unrounded endpoint would be marked wrong. Add a rounding instruction and switch to the interval answer type, which accepts spacing variants and uses a tolerance. Using "number" instead of "numfunc" for parts (a) and (c) would also be cleaner.
- `bank-21` / gemma4-31b: 3 — The class labels overlap ("10 to 15", "15 to 20") and the question never says which class a boundary value belongs to, but the code counts values lower-inclusive. A student who puts a 15 in "10 to 15" gets marked wrong, and boundary values show up often with integer data. Fix by labeling the classes as non-overlapping integer ranges (e.g. 10-14, 15-19) or by stating the convention.
- `bank-22` / gemma4-31b: 3 — The scenario text is wrapped in backticks (`$q_text[0]` etc.), which MyOpenMath treats as AsciiMath delimiters. The sentences will render as italic math with mangled spacing instead of plain text, so remove the backticks. The classifications and reasons are all correct at every shuffle.
- `bank-23` / gemma4-31b: 3 — The code uses an inclusive check (>= and <=), but mu0 and the bounds are integers, so mu0 sometimes equals a bound exactly (about 7% of seeds). At that point the hypothesized value sits on the edge of the interval, the p-value is exactly 0.05, and the 'Yes / fail to reject' answer is debatable. Fix by making the bounds non-integers (e.g. add .5) or by re-rolling when mu0 equals $lower or $upper. Minor: the Yes/No box would be more robust as a choices question.
- `bank-24` / gemma4-31b: 3 — The teacher asked for a free-response question, but parts (b), (c) and (d) are all multiple choice, and (b) and (c) are complements of each other, so the second gives away the first. Part (d) also hands students the reasoning instead of asking them to explain it. Consider short-answer or text-entry parts, and use the 'number' type instead of 'numfunc' for part (a). The math and answer keys are correct at every seed.
- `bank-26` / gemma4-31b: 2 — Part c has the wrong answer key: $answer[2] = 0 selects 'The total cost of any cake regardless of size.', while the correct option 'The amount the cost increases for each additional serving.' is index 1. Students who answer correctly lose credit, so it needs $answer[2] = 1. Also, 'number' would be a more suitable answer type than 'numfunc' for the numeric parts.
- `bank-28` / gemma4-31b: 3 — Part (a) is graded as an exact string, "n * log_b(x)", and the student is never told the format, so correct answers like "3log_2(5)" or "3*log_2(5)" would be marked wrong. Fix by giving format instructions or by redesigning the part, for example asking for the exponent n as a number, or using a multiple-choice or matching format. Part (b) is mathematically correct but should use the "number" answer type instead of "numfunc". The solutionguide also has stray "$n$" text that will print a leftover dollar sign.
- `bank-27` / gemma4-31b: 3 — The formulas and the normal-CDF approximation are correct, but the question never says how to round, and part (c) is graded against a 4-decimal answer, so a student who rounds z to 2 decimals and uses a table can be marked wrong. The c offset is not scaled to sigmaSum (sigmaSum can be about 30 while the offset reaches 150), so z can reach about ±5 and the answer becomes 0.0000 or 1.0000; scale the offset in z-units and add a rounding instruction and tolerance.
- `edge-picture` / gemma4-31b: 2 — No $answerformat (e.g. "reducedfraction") is set, so decimals and unreduced fractions are accepted, which breaks the 'must enter a fraction' requirement. The graph is also likely broken: showplot() is written inside backticks in the question text, so it renders as math rather than calling the function, and the line equation is built from decimal strings (e.g. '+ -1.5'). Giving both points in the text also removes the need to read the graph. The slope itself is computed correctly.
- `edge-dropdown` / gemma4-31b: 2 — Part (a) is graded as an exact string, 'string' rather than 'interval' type, against '(-inf, $sol_val)'. Whenever num/denom isn't an integer (e.g. 7/3) the expected answer is a long truncated decimal like 2.3333333333333. A student who correctly enters (-inf, 7/3) is marked wrong, and spacing differences such as '(-inf,2.5)' also fail. Use the 'interval' answer type (with a fraction or decimal tolerance), or choose num as a multiple of a so the answer is always an integer.
- `bank-06` / deepseek-v4.1-flash: 3 — The answers are right at every seed: the invNorm values for 0.90, 0.95, 0.96, 0.98, 0.99 and for 0.90, 0.95, 0.975, 0.99, 0.995 are correct, and the choices are distinct. The worked solution's Step 3 is wrong, though: $tailPct = (100-$mPct)/2 is already the per-tail percent, but the text calls it the total split between the tails and halves it again (80% gives '10% to split... 5% in each tail' when it should be 20% and 10%). Fix by defining $tailPct = 100 - $mPct and keeping the /2 for each tail.
- `edge-equation` / gemma4-31b: 3 — The computed answer is correct, but many seeds give fractional slopes or intercepts (e.g. 7/3) and the prompt never says to enter exact fractions rather than rounded decimals, so a student who types 2.33 could be marked wrong. The solution guide also shows b rounded to 3 decimals next to an exact fractional slope, and it prints '+ -3' for negative b, so it should build b as a reduced fraction and handle signs.
- `edge-feedback-solution` / gemma4-31b: 3 — The backticks in the question text are misplaced: `$a(`$b - $c`)^2` puts only "a(" and ")^2" in math mode and leaves "b - c" as plain text, so the expression renders in mixed fonts and may look garbled. Wrap the whole expression in one pair, `$a($b - $c)^2`. The answer (a*(b-c)^2, with b-c from 1 to 14) and the solution steps are correct.
- `bank-05` / claude-sonnet-5.5: 3 — The content is sound: classifications, confounding and bias choices, and justified and unjustified conclusions are correct at all four study variants. But the teacher asked for free response, where students name the bias and state the conclusions, and every part is converted to a pick-from-list choices part. The only justification is a code comment the teacher may not see. Either make parts (b)-(d) essay or manually graded, or tell the teacher about the change.
- `bank-14` / deepseek-v4.1-flash: 3 — The graded answers are right at every seed: s3 leaves, the optimum is (8+a, 6+a), P = 78+11a, and 1 or 2 molds are unused. But Step 4 of the solution guide mixes units: $waxUsed = 2x+y and $laborUsed = x+2y are in the scaled constraint units, yet are shown as ounces and minutes against $waxOz and $laborMin (for a=0, 'wax used is 22 ounces against 44 on hand'). That contradicts its claim that both constraints are tight. Fix by using 4x+2y and 5x+10y, or by comparing against $waxLimit and $laborLimit.
- `edge-select-all` / deepseek-v4.1-flash: 3 — Statements 0, 1 and 2 have their verdicts hardcoded ($t[0]=1, $t[1]=1, $t[2]=0) rather than computed from the data. The comparison statements are worded to be true, and the mode statement 'is $a' is always false, so a student can learn the pattern without calculating. The request asked for every verdict to be computed, so make the comparison direction and the mode claim random, then compute each verdict from the data and enforce at least two true. The statistics, sorting and solution text are otherwise correct.
- `edge-money` / deepseek-v4.1-flash: 2 — The principal is formatted wrongly in the question text. $pRest is the hundreds digit, but it is zero-padded and appended after the comma, so a $1,500 deposit shows as '1,05 dollars' and $8,000 as '8,00'. The student sees a principal that doesn't match the one used for the answer. Format it as thousands + ',' + three digits, or use a number-format function. The compound-interest computation and the future-value formatting are otherwise correct.

## Problems left on failing runs

- `bank-01` / gemma4-31b: Caught error while evaluating the code in this question: syntax error, unexpected token ";" on line 56 of Common Control (seeds 4242, 7, 99)
- `bank-02` / gemma4-31b: Caught error while evaluating the code in this question: syntax error, unexpected token ";" on line 85 of Common Control (seeds 4242, 7, 99)
- `bank-04` / gemma4-31b: Caught error while evaluating the code in this question: syntax error, unexpected token ";" on line 87 of Common Control (seeds 4242, 7, 99)
- `bank-06` / gemma4-31b: Caught error while evaluating the code in this question: syntax error, unexpected token ";" on line 51 of Common Control (seeds 4242, 7, 99)
- `bank-15` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `bank-30` / gemma4-31b: Possible error in diffrands: Need min&lt;max.  (seeds 4242, 7, 99)
- `edge-select-all` / gemma4-31b: Eeek.. unallowed macro implode (seeds 4242, 7, 99)
- `edge-matrix` / gemma4-31b: part 0: correct answer scored 0 (seeds 4242, 7, 99)
