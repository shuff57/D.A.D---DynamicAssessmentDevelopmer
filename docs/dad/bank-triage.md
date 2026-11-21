# Question bank triage (2026-11-20)

Engine self-test of `mom-content` (1,239 questions) at seeds 4242, 7 and 99.

- **1,167 pass**: every machine-gradable part scores full marks with its correct answer and 0 when blank, at every seed.
- 180 of those also have essay, upload, drawing or picture-option parts, which a teacher has to check by eye.
- **72 fail**: 57 question defects and 15 to look at. "In use" means the question is in at least one assignment.

These are the repo copies of the questions; a live MyOpenMath copy edited by hand may differ.

Re-run: `engine/run.sh && python3 engine/regress.py <mom-content> --check engine/baseline.json && python3 engine/triage.py <mom-content>`.

## Invented answer type (`multiple_choice`, `numerical`; IMathAS has `choices`, `number`) (17)

- `regression/intro/q3-interpret-slope.php` (seeds 4242, 7, 99) — in use
- `regression/intro/q4-interpret-intercept.php` (seeds 4242, 7, 99) — in use
- `regression/intro/q5-positive-negative.php` (seeds 4242, 7, 99) — in use
- `regression/intro/q6-linear-appropriate.php` (seeds 4242, 7, 99) — in use
- `regression/intro/q8-yhat-notation.php` (seeds 4242, 7, 99) — in use
- `regression/least-squares/q1-least-squares-criterion.php` (seeds 4242, 7, 99) — in use
- `regression/least-squares/q10-conditions-for-regression.php` (seeds 4242, 7, 99) — in use
- `regression/least-squares/q12-association-vs-causation.php` (seeds 4242, 7, 99) — in use
- `regression/least-squares/q3-slope-from-correlation.php` (seeds 4242, 7, 99) — in use
- `regression/least-squares/q4-intercept-from-means.php` (seeds 4242, 7, 99) — in use
- `regression/least-squares/q6-line-passes-through-means.php` (seeds 4242, 7, 99) — in use
- `regression/least-squares/q7-r-squared-interpretation.php` (seeds 4242, 7, 99) — in use
- `regression/residuals-correlation/q3-residual-plot-good-fit.php` (seeds 4242, 7, 99) — in use
- `regression/residuals-correlation/q4-residual-plot-nonlinear.php` (seeds 4242, 7, 99) — in use
- `regression/residuals-correlation/q6-residual-concepts.php` (seeds 4242, 7, 99) — in use
- `regression/residuals-correlation/q8-estimate-r-from-scatter.php` (seeds 4242, 7, 99)
- `trig/sine-sign-by-quadrant.php` (seeds 4242, 7, 99)

## Question text prints a variable the code never sets (a number, picture or table silently missing) (9)

- `im1/arithmetic-decreasing-sequence-find-explicit-equation-and-terms-copy-b.php` (seeds 4242, 7, 99) — in use (qsetid 1530185)
- `im1/determine-the-rigid-transformation-reflection.php` (seeds 4242, 7, 99) — in use (qsetid 1475095)
- `im1/determine-the-rigid-transformation-transformation.php` (seeds 4242, 7, 99) — in use (qsetid 1475098)
- `im1/graph-mx-b-2-slope-m-2.php` (seeds 4242, 7, 99)
- `im1/graph-mx-b-3-slope-m-3.php` (seeds 4242, 7, 99)
- `im1/graph-mx-b-slope-m-5.php` (seeds 4242, 7, 99)
- `im1/graph-y-2x-b-give-table-of-values-graph-at-least-3-points-and-graph-li.php` (seeds 4242, 7, 99) — in use (qsetid 1468909)
- `im3/law-of-sines.php` (seeds 4242, 7, 99) — in use (qsetid 1778160)
- `im3/parent-functions-and-transformations-1-f-x-x-2.php` (seeds 4242, 7, 99) — in use (qsetid 1671770)

## Duplicate choice options (picking the other copy is marked wrong) (14)

- `exp-log/q2-log-evaluation-and-conversion.php` (seeds 7, 99) — in use
- `inequalities/q3-check-solution-system.php` (seeds 4242, 7, 99) — in use
- `linear-programming/q5-lp-application-identify-formulation.php` (seeds 99) — in use
- `stats-tests/hypothesis-testing/q20-hypothesis-symbol-pairs.php` (seeds 4242, 7) — in use
- `stats-tests/hypothesis-testing/q36-proportion-condition-failure.php` (seeds 99) — in use
- `stats-tests/hypothesis-testing/q47-tail-from-alternative.php` (seeds 4242, 7) — in use
- `trig/cosine-features-from-graph-advanced.php` (seeds 4242, 7)
- `trig/cosine-features-from-graph-easy.php` (seeds 4242, 7, 99)
- `trig/cosine-features-from-graph-hard.php` (seeds 4242, 7, 99)
- `trig/cosine-features-from-graph-medium.php` (seeds 4242, 7, 99)
- `trig/sine-features-from-graph-advanced.php` (seeds 4242, 7)
- `trig/sine-features-from-graph-easy.php` (seeds 4242, 7, 99)
- `trig/sine-features-from-graph-hard.php` (seeds 4242, 7, 99)
- `trig/sine-features-from-graph-medium.php` (seeds 4242, 7, 99)

## HTML or backtick math inside a dropdown option (shows as raw text) (8)

- `sampling-distributions/q1-p-hat-conditions.php` (seeds 4242, 7, 99)
- `sampling-distributions/q2-x-bar-conditions.php` (seeds 4242, 7, 99) — in use
- `stats-tests/ci-ht-connection/q9-ht-using-ci-proportion.php` (seeds 4242, 7, 99)
- `stats-tests/hypothesis-testing/q5-pvalue-decision.php` (seeds 4242, 7, 99) — in use
- `stats-tests/hypothesis-testing/q6-null-alt-hypotheses.php` (seeds 4242, 7, 99) — in use
- `stats-tests/hypothesis-testing/q7-ht-one-mean.php` (seeds 4242, 7, 99) — in use
- `stats-tests/hypothesis-testing/q8-ht-one-proportion.php` (seeds 4242, 7, 99) — in use
- `stats-tests/hypothesis-testing/q9-reject-nuanced.php` (seeds 4242, 7, 99) — in use

## A blank answer earns credit (2)

- `finance/annuities/q9-annuity-due-future-value.php` (seeds 4242, 7, 99)
- `finance/compound-interest/q6-solve-rate-compound.php` (seeds 4242, 7, 99)

## Bad `requiretimes` (1)

- `im1/simpify-a-n.php` (seeds 4242, 7, 99)

## A choices part with no `$questions` (1)

- `inequalities/q1-solve-linear-inequality.php` (seeds 4242, 7, 99) — in use

## Wrong arguments to a macro (2)

- `regression/least-squares/q16-slope-intercept-interpret-context.php` (seeds 4242, 7, 99)
- `regression/slope-inference/q9-slope-concept-decisions.php` (seeds 4242, 7, 99)

## Warning in the question code (undefined variable or array offset) (3)

- `descriptive-stats/q1-descriptive-vs-inferential.php` (seeds 4242, 7, 99)
- `probability/binomial/pre-frq-grade-a-binomial-setup.php` (seeds 4242, 7, 99) — in use (qsetid 1877085)
- `probability/expected-value/pre-frq-grade-a-long-run-interpretation.php` (seeds 4242, 7, 99) — in use (qsetid 1877068)

## Needs a look: the question or the self-test (no error, an answer did not score) (15)

- `counting/permutations-combinations.php` (seeds ) — Expecting value: line 1 column 1 (char 0)
- `frq/descriptive-statistics/q6-frequency-distribution-analysis.php` (seeds ) — in use (qsetid 1893149) — Expecting value: line 1 column 1 (char 0)
- `im1/adding-pattern-figure-to-table.php` (seeds 4242, 7, 99) — in use (qsetid 1893443) — part 4 (choices): no option matches the correct answer
- `im1/find-the-mistake-in-a-growing-pattern-rule.php` (seeds 4242, 7, 99) — in use (qsetid 1893405) — part 2 (choices): no option matches the correct answer
- `im1/find-the-mistake-in-simplifying-a-fraction.php` (seeds 4242, 7, 99) — in use (qsetid 1894930) — part 2 (choices): no option matches the correct answer
- `im1/multiplying-pattern-figure-to-table.php` (seeds 4242, 7, 99) — in use (qsetid 1893402) — part 4 (choices): no option matches the correct answer
- `im1/multiplying-pattern-recursive-and-explicit-rules.php` (seeds 4242, 7, 99) — in use (qsetid 1893403) — part 1 (choices): no option matches the correct answer
- `im1/multiplying-versus-adding-graph-and-crossover.php` (seeds 4242, 7, 99) — in use (qsetid 1893404) — part 3 (choices): no option matches the correct answer
- `inequalities/q1-inequality-properties.php` (seeds 4242, 7, 99) — part 2 (choices): no option matches the correct answer
- `stats-tests/confidence-intervals/q19-raw-data-to-interval.php` (seeds ) — in use — Expecting value: line 1 column 1 (char 0)
- `stats-tests/confidence-intervals/q22-proportion-conditions.php` (seeds ) — in use — Expecting value: line 1 column 1 (char 0)
- `stats-tests/hypothesis-testing/q10-conclusion-in-context.php` (seeds 4242, 7, 99) — in use — part 0 (choices): no option matches the correct answer
- `stats-tests/two-sample-inference/q11-two-prop-test-cholesterol.php` (seeds 4242, 7, 99) — part 0 (choices): no option matches the correct answer
- `trig/sine-graph-from-equation.php` (seeds 7) — part 2: correct answer scored 0
- `trig/sine-graph-match-equation.php` (seeds ) — Expecting value: line 1 column 1 (char 0)
