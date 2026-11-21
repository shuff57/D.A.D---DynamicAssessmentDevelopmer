# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| openrouter-free | 33/40 | 15/40 | 1.85 | 168 s | 3.21 | 11/40 | 27/33 | 22/33 | 26/33 | 0 | 0 | 30 | 3 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | openrouter-free |
|---|---|
| `bank-01` | pass, 1 try, judge 4 |
| `bank-02` | pass, 2 tries, judge 3 |
| `bank-03` | error |
| `bank-04` | pass, 2 tries, judge 2 |
| `bank-05` | pass, 4 tries, judge 2 |
| `bank-06` | pass, 1 try, judge 3 |
| `bank-07` | pass, 2 tries, judge 2 |
| `bank-08` | pass, 2 tries, judge 2 |
| `bank-09` | pass, 2 tries, judge 3 |
| `bank-10` | pass, 1 try, judge 5 |
| `bank-11` | pass, 1 try, judge 4 |
| `bank-12` | pass, 1 try, judge 4 |
| `bank-13` | pass, 3 tries, judge 4 |
| `bank-14` | pass, 1 try, judge 4 |
| `bank-15` | pass, 1 try, judge 2 |
| `bank-16` | fail (2 left) |
| `bank-17` | pass, 4 tries, judge 4 |
| `bank-18` | fail (1 left) |
| `bank-19` | pass, 2 tries, judge 3 |
| `bank-20` | error |
| `bank-21` | pass, 4 tries, judge 4 |
| `bank-22` | pass, 3 tries, judge 4 |
| `bank-23` | pass, 1 try, judge 3 |
| `bank-24` | pass, 1 try, judge 2 |
| `bank-25` | pass, 1 try, judge 3 |
| `bank-26` | pass, 2 tries, judge 3 |
| `bank-27` | pass, 2 tries, judge 3 |
| `bank-28` | pass, 2 tries, judge 3 |
| `bank-29` | pass, 2 tries, judge 3 |
| `bank-30` | pass, 1 try, judge 3 |
| `edge-picture` | fail (1 left) |
| `edge-select-all` | pass, 2 tries, judge 3 |
| `edge-matching` | pass, 1 try, judge 3 |
| `edge-dropdown` | fail (1 left) |
| `edge-money` | pass, 1 try, judge 3 |
| `edge-matrix` | pass, 4 tries, judge 3 |
| `edge-equation` | pass, 2 tries, judge 3 |
| `edge-distractors` | fail (2 left) |
| `edge-string` | pass, 1 try, judge 5 |
| `edge-feedback-solution` | pass, 1 try, judge 4 |

## Judge notes on questions scoring 3 or less

- `bank-02` / openrouter-free: 3 — The math and the answer key are correct (24, 50, and the first choice), but the request describes the same table drawn twice, as counts and as percentages, with the group size recovered from the graphs. This version shows no graphs and just gives the counts, so part (a) is a trivial sum and the question never uses the two-graph idea. Add the two bar graphs (e.g. with a plot or a table of the percentages), have students recover N from them, and tell students to enter part (b) as a number of percent such as 50.
- `bank-04` / openrouter-free: 2 — The teacher asked for three expected cells, but a stray '/// [QUESTION 2]' block asks for a fourth cell with $answerbox[3]. No $answer[3] or third anstype exists for it, so that part cannot be answered or scored. The opening sentence also contains leftover string-concatenation code ("' . '' . 'a sample of ' . $N . ' cases"), which would display literally. Separately, E = 20ab/n is often a non-terminating decimal, so 'enter exact values' is unreliable. Remove the fourth part, fix the sentence, and choose N so the expected counts come out as integers or terminating decimals.
- `bank-06` / openrouter-free: 3 — The answers are right at every seed: left area 1-α, z = invNorm(1-α), and the middle 1-2α is bounded by ±z_α. But the question text has an unbalanced backtick in `α = `$alpha`.`, which flips math/text parity for the rest of the text, so the answer boxes and symbols render garbled. Write it as `α = $alpha`. The solution guide has the same problem and also prints unrounded critZ values. Part (c) says "this same z-value," which gives away the idea, and it does not say whether to enter the positive or negative bound.
- `bank-05` / openrouter-free: 2 — Part (a) sets $answer[0] = "observational or experimental" (or the reverse). In a MyOpenMath string answer, "or" lists acceptable answers, so both choices are marked correct and the part cannot be wrong; it should be $answer[0] = $studyClass. Also, $desc is wrapped in backticks in the question text, which triggers math rendering and will garble the scenario. The solution guide also has stray wording ("drug notification etc") and the bias lists lack punctuation.
- `bank-07` / openrouter-free: 2 — The teacher asked for a scatterplot with a line, but the question gives only a text description and no graph. The description gives the answer away, so students never read a plot, which is the skill being assessed. Add a generated plot (e.g. with showplot/ draw commands, using random data for each pattern) and drop the verbal description.
- `bank-09` / openrouter-free: 3 — randfrom(3) is a list containing only 3, so $base is always 4 and bases 2 and 3 never appear. Use randfrom(2,3,4) or rand(2,4) instead. Also wrap the expression in backticks (`log_{$base}($arg)`) so it renders as math, and check that ** is supported (^ is the usual operator).
- `bank-15` / openrouter-free: 2 — The (1,3) entry of A^{-1} is wrong: it should be r1*r3 - r2, but the code uses r2 + r1*r3 (check: row 1 of A times column 3 of A^{-1} gives 0 only with the correct sign). This also makes x1 wrong at every seed and repeats the error in the solution guide. Also, X is a 3x1 column but the answer is built as a 1x3 row [[x1,x2,x3]], so the entry format is unclear and may not match.
- `bank-08` / openrouter-free: 2 — The teacher asked for a grading checklist/rubric, but none is shown. Students must choose Fully correct, Partially correct or Incorrect with no criteria, so Response 3 (partial) and Response 4 (incorrect vs. partial) are judgment calls. The stem also never says the data are weekly screen time, so that context first appears inside the responses. Add a checklist of criteria and the variable and time unit to the stem.
- `bank-19` / openrouter-free: 3 — Part (c) asks what a positive residual indicates, so the answer is always 'above' (index 0). The code sets $answer[2] = ($e > 0) ? 0 : 1, so whenever e is negative it marks 'below' correct and penalizes students who answer correctly. Fix: $answer[2] = 0. Minor polish: the line can display as '0 + 1x', and the blanks are redundant alongside $answerbox.
- `bank-23` / openrouter-free: 3 — In the 'inside' branch, rand($lower*10, $upper*10)/10 can return exactly $lower or $upper, so the null value lands on a CI endpoint. There the p-value equals α and the decision is ambiguous, yet the code marks 'Fail to reject' correct. Draw the inside value strictly between the endpoints, e.g. rand($lower*10+1, $upper*10-1)/10.
- `bank-24` / openrouter-free: 2 — The teacher asked for interpreting equal means with different SDs to conclude which group is more consistent, but this only asks students to copy the larger of two given numbers. Group 2 is always the larger (sd1 is 4-10, sd2 is 11-20), so the answer is always $sd2 and the ternary is pointless. 'The groups below' refers to nothing, and no mean is given. Redesign it so students decide which group is more consistent or has more spread, for example by choosing a group or explaining why.
- `bank-25` / openrouter-free: 3 — The computed answers (75, 88.89, 95.06) and the correct choice (index 1, 'at least the minimum') are right, but the worked solution has a typo that students will see ('1 - 1/00' should be '1 - 1/9'). The question text also says 'lapped percentages', which should read 'percentages', and the third distractor ('exact percentage of the population mean') is nonsensical and should be replaced.
- `bank-26` / openrouter-free: 3 — The equation string has three backticks (`exam score = 2.5 \cdot `hours of study + 40`), so the math delimiters are unbalanced and the displayed equation will be garbled. Also, several seed combinations predict scores above 100 (e.g., 4.5*12+60 = 114, 3.5*15+60 = 112.5), which is unrealistic for an exam score. Lower the slope or intercept, or limit the x values, so predictions stay at or below 100.
- `bank-28` / openrouter-free: 3 — The math is correct and it matches the power rule request, but numfunc grades by numerical equivalence, so a student can type the given expression ln(x^n) back and get full credit without expanding. Add a restriction such as $requiretimes = "^,0" to forbid the exponent symbol in the answer.
- `bank-27` / openrouter-free: 3 — The setup is right: sum mean n*mu, sum SD sqrt(n)*sigma, and P(sum > c) = 1 - Phi(z). But $c is never rounded, so it prints with a long irrational decimal (sqrt(n)*sigma*z), and students are not told how to round their probability. A reltolerance of 0.001 is tighter than a z-table answer, so correct work can be marked wrong. Round $c (e.g. to 1-2 decimals) and compute $prob from the rounded value. Then add a rounding instruction such as 4 decimals with a matching tolerance, and make $solutionguide show the z-score step with rounded values.
- `bank-29` / openrouter-free: 3 — The counts are right (C(n,k) and C(n-1,k-1)) and the question is clear, but $solutionguide is in single quotes, so PHP won't substitute $n, $k, $total or $with and students will likely see the raw variable names. Switch to double quotes, and use computed values such as {$n-1} (or precompute $n1 and $k1) so the text shows 8 and 3 rather than 9-1 and 4-1.
- `bank-30` / openrouter-free: 3 — The math is right (4 subsets; {1}, {2}, {1,2}), but Part (b) is one exact-string match. It is order-dependent and space-sensitive, so a correct answer like "{1},{2},{1,2}" or "{2}, {1}, {1,2}" is marked wrong, and the prompt gives no format or order. Add remove_whitespace and accept the permutations, or tell students the exact format to use.
- `edge-matching` / openrouter-free: 3 — The content is right and the sixth description (sample size) is a good distractor. But the text says to write the description's number while the descriptions are unnumbered, and the key is 0-indexed. The hard-coded descriptions are also listed in the same order as the terms, so the answer is simply 1-2-3-4-5. Number or letter the descriptions and shuffle their order, or use the built-in matching display.
- `edge-select-all` / openrouter-free: 3 — When two values tie for the highest frequency (e.g. two pairs, which is common with 8 values drawn from 5-20), the code still marks "The mode is X" true for the smallest tied value, but the data is bimodal, so that statement should not count as true. Statements 0 and 1 are also always true by construction, so the true/false pattern varies less than the teacher asked for.
- `edge-money` / openrouter-free: 3 — The formula and rounding are correct, but the default relative tolerance of 0.1% accepts answers off by many cents, so cent precision is not enforced. Set $abstolerance = 0.005 or $reqdecimals = 2. Also fill in $solutionguide, which is empty, and write the principal as \$$P so it shows a dollar sign (currently 'a 5234 principal').
- `edge-equation` / openrouter-free: 3 — The worked solution's Step 1 is wrong: the slope numerator uses the x-values (px2 - px1) instead of the y-values (py2 - py1). The points are also printed without parentheses ("3, 5 and 7, 9"), which is confusing. Slopes like 1/3 print as long repeating decimals, and negatives show as "+ -3" in the solution. The computed answer is right, and numfunc accepts equivalent forms.
- `edge-matrix` / openrouter-free: 3 — The products and the answer string are correct, and the 2x2 matrix entry fits the request. But $displayA and $displayB use space-separated entries like "[ [a b c] [d e f] ]", which AsciiMath won't draw as a matrix, and "2 -3" can read as subtraction. Use comma-separated AsciiMath such as [[a,b,c],[d,e,f]] so the matrices render properly.

## Problems left on failing runs

- `bank-03` / openrouter-free: The AI did not answer: The reply had no message content.
- `bank-16` / openrouter-free: part 0 (choices): no option matches the correct answer (seeds 4242, 7, 99)
- `bank-20` / openrouter-free: The AI did not answer: The reply had no message content.
- `bank-18` / openrouter-free: part 1: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-dropdown` / openrouter-free: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-picture` / openrouter-free: Invalid requiretimes - check format (seeds 4242, 7, 99)
- `edge-distractors` / openrouter-free: part 0 (choices): no option matches the correct answer (seeds 4242, 7, 99)
