# Question-writing harness

40 cases (30 from the bank, 10 hand-written edge cases), engine self-test at seeds 4242, 7, 99.

| Model | Passed | First try | Mean tries (passing) | Median time | Judge mean (1-5) | Judged 4+ | Matches request | Clear | Math right | Restarts | Critic rounds | Lookups | Pre-flight catches |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| deepseek-v4.1-flash-think-off | 30/40 | 15/40 | 1.57 | 6 s | 3.13 | 11/40 | 25/30 | 21/30 | 19/30 | 0 | 0 | 0 | 16 |

The judge only sees questions that passed the engine. "Passed" is the engine's verdict; the judge columns are one model's opinion.

## Cases

| Case | deepseek-v4.1-flash-think-off |
|---|---|
| `bank-01` | pass, 1 try, judge 2 |
| `bank-02` | pass, 3 tries, judge 3 |
| `bank-03` | pass, 1 try, judge 2 |
| `bank-04` | pass, 1 try, judge 4 |
| `bank-05` | fail (1 left) |
| `bank-06` | pass, 2 tries, judge 4 |
| `bank-07` | pass, 2 tries, judge 2 |
| `bank-08` | pass, 1 try, judge 2 |
| `bank-09` | pass, 1 try, judge 2 |
| `bank-10` | pass, 1 try, judge 3 |
| `bank-11` | pass, 1 try, judge 5 |
| `bank-12` | fail (1 left) |
| `bank-13` | pass, 2 tries, judge 5 |
| `bank-14` | pass, 2 tries, judge 2 |
| `bank-15` | fail (2 left) |
| `bank-16` | pass, 1 try, judge 3 |
| `bank-17` | pass, 2 tries, judge 3 |
| `bank-18` | fail (1 left) |
| `bank-19` | pass, 2 tries, judge 4 |
| `bank-20` | pass, 1 try, judge 4 |
| `bank-21` | pass, 2 tries, judge 4 |
| `bank-22` | pass, 2 tries, judge 2 |
| `bank-23` | pass, 1 try, judge 3 |
| `bank-24` | fail (6 left) |
| `bank-25` | pass, 1 try, judge 5 |
| `bank-26` | pass, 1 try, judge 2 |
| `bank-27` | pass, 3 tries, judge 3 |
| `bank-28` | fail (4 left) |
| `bank-29` | pass, 2 tries, judge 4 |
| `bank-30` | pass, 2 tries, judge 2 |
| `edge-picture` | pass, 1 try, judge 2 |
| `edge-select-all` | fail (3 left) |
| `edge-matching` | pass, 1 try, judge 3 |
| `edge-dropdown` | fail (1 left) |
| `edge-money` | pass, 2 tries, judge 3 |
| `edge-matrix` | fail (1 left) |
| `edge-equation` | pass, 2 tries, judge 4 |
| `edge-distractors` | fail (2 left) |
| `edge-string` | pass, 1 try, judge 2 |
| `edge-feedback-solution` | pass, 2 tries, judge 5 |

## Judge notes on questions scoring 3 or less

- `bank-03` / deepseek-v4.1-flash-think-off: 2 — Part (c) answer key is wrong: $answer[2] = 1 marks the second choice ('percentages do not add up to 100%') as correct, but choices are 0-indexed and the double-counting explanation is index 0, so it should be $answer[2] = 0. Parts (a) and (b) are correct at every seed. Minor polish: some context/event pairings are odd, e.g. a high school student who 'takes the bus to work'.
- `bank-01` / deepseek-v4.1-flash-think-off: 2 — Parts 2 and 3 use fixed relations, {(1,2),(2,3),(3,4),(4,5)} and 1→5, 2→6, 3→7, 4→8, which are both functions. The code sets $answer[1] and $answer[2] equal to $answer[0], so whenever $isFunc==0 they are keyed 'No', which is wrong in about half of seeds. Fix by hard-coding $answer[1]=0 and $answer[2]=0, or randomize each part separately. Also, the ordered-pair case outputs \{ \} outside $ math delimiters, so the braces may not render.
- `bank-02` / deepseek-v4.1-flash-think-off: 3 — The request says the same table is drawn twice (counts and percentages), but only the counts graph is drawn and part (c) is purely hypothetical. Separately, the rejection loop can never succeed: the first three counts are at most 44 each, so c3 is always at least 68, never in 24-44. Senior is therefore always the tallest bar, at 34-64%, and the group size is always 200. The answers stay correct, but the variety is far narrower than the code intends.
- `bank-07` / deepseek-v4.1-flash-think-off: 2 — When $slopeSign is 1 (negative slope, about half of seeds), y = slope*x + intercept goes far below 0 (e.g. -2*10 + 3 = -17), but the y-axis is fixed at minY = 0, so most points and the fitted line are drawn below or off the plot and the picture is unusable. Fix by computing minY from min($ys) and the line endpoints, or by shifting the intercept so all y values stay positive. Also, $noshuffle = "questions" is not a valid setting for choices (use "all" if the order should be fixed).
- `bank-09` / deepseek-v4.1-flash-think-off: 2 — The answer key is wrong: part (b) asks for the exponent e with base^e = val, which is $exp, but $answer[1] is set to $k. Part (c) is also broken: $answer[2] = $n is not always the smaller factor (e.g. k=2, n=4 gives 8 = 2·4, so the answer should be 2), and factorizations like 12 = 2·6 = 3·4 are not unique. Parts (b) and (c) were not requested, (b) just repeats (a), and exponents up to 20 make arguments as large as 4^20 (about 1.1 trillion), which students cannot reasonably recognize as a power. Simplify to one part, with a smaller exponent range and a displayed argument that is a manageable power.
- `bank-08` / deepseek-v4.1-flash-think-off: 2 — The answer key is inverted. MyOpenMath choices answers are 0-indexed, and the choices are ("Full credit", "Not full credit"), so $verdicts of 1,0,0,0 marks Response 1 as Not full credit and Responses 2-4 as Full credit, the opposite of the intended key and the worked solution. A second error: the n=25 scenario (strongly right-skewed) makes Response 1's and the solution's claim that 'n = 25 ≥ 30' false, so the CLT justification fails there. Smaller issues: the Response 2 'shape is wrong' verdict is shaky for the roughly symmetric scenarios, and the correct response is always Response 1.
- `bank-10` / deepseek-v4.1-flash-think-off: 3 — Part (b) asks 'Which formula gives the number of handshakes' but its choices are numeric values (nC2, n², n(n-1), 2n-1 evaluated), not formulas, so it doesn't match its own wording and just repeats the answer to (a). Fix by making the choices formula text such as n(n-1)/2, n², n(n-1), n+(n-1), and delete the ~1000 unused $wrong variables.
- `bank-14` / deepseek-v4.1-flash-think-off: 2 — The optimality check is wrong, so some seeds give a wrong answer. It tests profit at the unconstrained wax and time intercepts, which are often infeasible, instead of checking that the profit vector lies between the two constraint normals. For example, with a11=2, a12=3, a21=3, a22=2, p1=9, p2=4, the code gives x=y=10 and P=130. The true optimum is x≈16.7, y=0, P=150. The guard loop can also exit after 200 tries without a valid draw. Part (d) is always 0, which is trivial.
- `bank-16` / deepseek-v4.1-flash-think-off: 3 — The five scenarios are unambiguous and each correctly matches its method, and the solution explanation is accurate. However, the code defines $methods and $answer, which MyOpenMath matching does not use; it expects $answers (the choice list) and $matchlist (the correct index for each question). Set $answers = $methods and $matchlist = $order, or rename the variables, so the dropdown choices and answer key are wired explicitly.
- `bank-17` / deepseek-v4.1-flash-think-off: 3 — The IQR, fences and outlier decision are computed correctly. However, the lower-outlier branch (obs = fenceLow - 2k) often gives a negative new observation, e.g. q1=30, iqr=24 gives a fence of -6 and obs ≤ -8. The non-outlier branch can do the same when fenceLow is negative. A negative wait time, commute time, spending amount or text count is impossible, so keep obs positive (e.g. use only the upper outlier when fenceLow < 2, or widen q1). Part (d), which only asks students to retype the given observation, is filler and could be dropped.
- `bank-22` / deepseek-v4.1-flash-think-off: 2 — The keyed reasons are wrong for two scenarios: 'customers arriving in an hour' and 'flip until first head' are keyed to 'probability of success is not the same on every trial' (index 2), but their real flaw is that the number of trials is not fixed (index 0, which is never keyed). Parts (e)-(g) also repeat only the three non-binomial scenarios, which gives away which one in (a)-(d) is binomial, and the 'not random sample' reason for the brand A survey is debatable.
- `bank-23` / deepseek-v4.1-flash-think-off: 3 — The question text mixes LaTeX with MyOpenMath's math syntax, so the notation will likely display as raw code: `$\hat{p} = $phat` leaves an unpaired literal $, and `\hat{p}`/`\sqrt{...}` sit inside AsciiMath backticks. Rewrite these in AsciiMath (`hat p`, `sqrt(...)`) or with proper LaTeX delimiters. Also, the 'given' 95% interval is built from the null-hypothesis SE rather than the phat SE, so it is not a standard CI, though the answers stay consistent with it.
- `bank-27` / deepseek-v4.1-flash-think-off: 3 — The setup and formulas are right, but $cOffset = rand(1,6) puts the cutoff 1 to 6 SDs above the mean of the sum. For offsets 4 to 6 the answer rounds to 0.0000, and offset 3 gives about 0.0013, so most variants are trivial or degenerate, and the solution shows '1 - 1.0000 = 0'. Use a fractional offset such as rand(-25,25)/10, which makes the cutoff land near the mean so the answers are real probabilities.
- `bank-26` / deepseek-v4.1-flash-think-off: 2 — In part (b), choices 0 and 1 are the same correct statement, since $dir2 ('increase') only differs from $dir ('increases') by grammar. A student can fairly argue two options are right, so replace choice 1 with a real distractor, such as the opposite direction. Minor polish: units are plural, giving awkward phrasing like 'points per hours' and 'each additional hours of hours studied'.
- `bank-30` / deepseek-v4.1-flash-think-off: 2 — Part (d) asks for a third non-empty subset different from S, but {e1, e2} has only two (the singletons), so it cannot be answered. Parts (b)-(d) also accept S itself and don't enforce distinct answers, contradicting the wording. Also, \(S = {a, b}\) in LaTeX drops the braces, so the set displays as 'a, b' unless written \{ \}. Fix by asking for the two proper non-empty subsets, then listing all three in (e).
- `edge-matching` / deepseek-v4.1-flash-think-off: 3 — The definitions and the intended pairings are correct, but the sixth description (the sampling frame) is never put into $answers, and $matchset is not a variable the matching type uses. Students will likely see only 5 descriptions, so the last term is still free by elimination and 'One description is not used' is false. Fix by adding $answers[5] = $descs[5] (the extra answer becomes the distractor) and dropping $matchset.
- `edge-picture` / deepseek-v4.1-flash-think-off: 2 — The code never calls showplot(); it only sets made-up variables like $showplot = 1 and $plotpoints, so the text 'graph below' would show a stray '1' and no graph, and the student can't see the points. It needs something like $showplot = showplot(array("$m*(x-($x1))+$y1"), -6,6,-6,6, ...) with the two lattice points marked. The slope arithmetic and the lowest-terms reduction are correct for every seed.
- `edge-money` / deepseek-v4.1-flash-think-off: 3 — The math and wording are right, but no tolerance is set, so IMathAS defaults to 0.1% relative error. A student can be several dollars off, or even use annual compounding at low rates and short terms (e.g. 2.5% for 2 years), and still be marked correct. Since the teacher asked for the answer to the cent, add $abstolerance = 0.01 (or a similarly tight tolerance).
- `edge-string` / deepseek-v4.1-flash-think-off: 2 — $answer is hard-coded to "experiment or observational study" instead of $correct. IMathAS treats " or " as a list of accepted answers, so both words are accepted for every scenario and a wrong answer earns full credit. Set $answer = $correct; the rest of the question is fine.

## Problems left on failing runs

- `bank-05` / deepseek-v4.1-flash-think-off: part 0 (choices): duplicate options ["an observational study","an experiment"] - picking the other copy is marked wrong (seeds 4242, 7, 99)
- `bank-12` / deepseek-v4.1-flash-think-off: Caught error while evaluating the code in this question: Unsupported operand types: int * string on line 37 of Common Control (seeds 4242, 7, 99)
  The code around that line:
     36 | $totCam = $camU
- `bank-18` / deepseek-v4.1-flash-think-off: part 1: correct answer scored 0 (seeds 4242, 7, 99)
- `bank-15` / deepseek-v4.1-flash-think-off: Caught warning in the question code: Array to string conversion on line 2 in Question Text (seeds 4242, 7, 99)
  The code around that line:
      1 | $M = array(array(1,0,0), array(0,1,0), array(0,0,1
- `bank-24` / deepseek-v4.1-flash-think-off: part 0 (choices): duplicate options ["Group A, because it has the smaller standard deviation","Neither, because the means are equal","Group B, because it has the larger standard deviation","Group A, b
- `bank-28` / deepseek-v4.1-flash-think-off: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-select-all` / deepseek-v4.1-flash-think-off: part 0 (multans): matched 0 of 0 correct options (seeds 4242, 7, 99)
- `edge-dropdown` / deepseek-v4.1-flash-think-off: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-matrix` / deepseek-v4.1-flash-think-off: part 0: correct answer scored 0 (seeds 4242, 7, 99)
- `edge-distractors` / deepseek-v4.1-flash-think-off: part 0 (choices): no option matches the correct answer (seeds 4242, 7, 99)
