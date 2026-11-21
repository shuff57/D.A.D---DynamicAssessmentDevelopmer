// === NAME - DESCRIPTION: Quiz scores: mean, median, and which statements about center and repeats are true ===
// === SET QUESTION TYPE TO: multipart ===

// === COMMON CONTROL ===
$quizScores = array();
for ($i = 0; $i < 6; $i++) { $quizScores[$i] = rand(50, 100); }

$sum6 = 0;
for ($i = 0; $i < 6; $i++) { $sum6 = $sum6 + $quizScores[$i]; }

$need = (7 - ($sum6 % 7)) % 7;
$lastScore = 50 + (((($need - 1) % 7) + 7) % 7);
$quizScores[6] = $lastScore;

for ($i = 6; $i > 0; $i--) {
  $j = rand(0, $i);
  $hold = $quizScores[$i];
  $quizScores[$i] = $quizScores[$j];
  $quizScores[$j] = $hold;
}

$sum = 0;
for ($i = 0; $i < 7; $i++) { $sum = $sum + $quizScores[$i]; }
$meanVal = $sum / 7;

$medianVal = 50;
for ($v = 50; $v <= 100; $v++) {
  $le = 0;
  $ge = 0;
  for ($i = 0; $i < 7; $i++) {
    if ($quizScores[$i] <= $v) { $le = $le + 1; }
    if ($quizScores[$i] >= $v) { $ge = $ge + 1; }
  }
  if ($le >= 4 && $ge >= 4) { $medianVal = $v; }
}

$hasDup = 0;
for ($i = 0; $i < 7; $i++) {
  for ($j = $i + 1; $j < 7; $j++) {
    if ($quizScores[$i] == $quizScores[$j]) { $hasDup = 1; }
  }
}

$aboveMed = 0;
$belowMed = 0;
for ($i = 0; $i < 7; $i++) {
  if ($quizScores[$i] > $medianVal) { $aboveMed = $aboveMed + 1; }
  if ($quizScores[$i] < $medianVal) { $belowMed = $belowMed + 1; }
}

$meanInData = 0;
for ($i = 0; $i < 7; $i++) {
  if ($quizScores[$i] == $meanVal) { $meanInData = 1; }
}

$scoreList = "";
for ($i = 0; $i < 7; $i++) {
  $val = $quizScores[$i];
  if ($i == 0) { $scoreList = "$val"; } else { $scoreList = "$scoreList, $val"; }
}

$sortedList = "";
$firstOut = 1;
for ($v = 50; $v <= 100; $v++) {
  for ($i = 0; $i < 7; $i++) {
    if ($quizScores[$i] == $v) {
      if ($firstOut == 1) { $sortedList = "$v"; $firstOut = 0; }
      else { $sortedList = "$sortedList, $v"; }
    }
  }
}

$stmt = array();
$stmt[0] = "The mean is greater than the median.";
$stmt[1] = "The mean is less than the median.";
$stmt[2] = "The mean is equal to the median.";
$stmt[3] = "Every score appears exactly once in the list.";
$stmt[4] = "At least one score appears more than once in the list.";
$stmt[5] = "The mean is one of the seven quiz scores.";
$stmt[6] = "The number of scores above the median equals the number of scores below the median.";

$ok = array();
$ok[0] = 0;
if ($meanVal > $medianVal) { $ok[0] = 1; }
$ok[1] = 0;
if ($meanVal < $medianVal) { $ok[1] = 1; }
$ok[2] = 0;
if ($meanVal == $medianVal) { $ok[2] = 1; }
$ok[3] = 0;
$ok[4] = 0;
if ($hasDup == 1) { $ok[4] = 1; } else { $ok[3] = 1; }
$ok[5] = $meanInData;
$ok[6] = 0;
if ($aboveMed == $belowMed) { $ok[6] = 1; }

$ansC = "";
$firstC = 1;
for ($i = 0; $i < 7; $i++) {
  if ($ok[$i] == 1) {
    if ($firstC == 1) { $ansC = "$i"; $firstC = 0; }
    else { $ansC = "$ansC,$i"; }
  }
}

$anstypes = "number,number,multans";
$questions[2] = $stmt;
$answer[0] = $meanVal;
$answer[1] = $medianVal;
$answer[2] = $ansC;

// === QUESTION TEXT ===
<p>A student took seven quizzes in a statistics class. The seven quiz scores, in the order they were recorded, are</p>

<p style="text-align:center;"><b>$scoreList</b></p>

<p>a. Find the mean of the seven scores. Round to two decimal places if needed.</p>

<p>Mean = $answerbox[0]</p>

///
<p>The seven quiz scores are</p>

<p style="text-align:center;"><b>$scoreList</b></p>

<p>b. Find the median of the seven scores.</p>

<p>Median = $answerbox[1]</p>

///
<p>The seven quiz scores are</p>

<p style="text-align:center;"><b>$scoreList</b></p>

<p>c. Select every statement below that is true for this data set. More than one statement may be true.</p>

<p>$answerbox[2]</p>

// === ANSWER ===
$relWord = "equal to";
if ($meanVal > $medianVal) { $relWord = "greater than"; }
if ($meanVal < $medianVal) { $relWord = "less than"; }

$dupWord = "No score repeats, so every score appears exactly once.";
if ($hasDup == 1) { $dupWord = "At least one score repeats, so not every score appears exactly once."; }

$meanWord = "The mean is not one of the seven scores.";
if ($meanInData == 1) { $meanWord = "The mean does happen to be one of the seven scores."; }

$symWord = "The same number of scores lie above the median as below it.";
if ($aboveMed != $belowMed) { $symWord = "The numbers of scores above and below the median are not equal."; }

$trueList = "";
$firstT = 1;
for ($i = 0; $i < 7; $i++) {
  if ($ok[$i] == 1) {
    $one = $stmt[$i];
    if ($firstT == 1) { $trueList = "<br>&bull; $one"; $firstT = 0; }
    else { $trueList = "$trueList<br>&bull; $one"; }
  }
}

$solutionguide = "<p><b>Step 1 &mdash; Order the data.</b> From smallest to largest the seven quiz scores are $sortedList</p><p><b>Step 2 &mdash; Find the mean.</b> The seven scores add to $sum, and dividing by 7 gives a mean of $meanVal, which is exact to two decimal places.</p><p><b>Step 3 &mdash; Find the median.</b> With seven ordered scores the median is the fourth score in the ordered list, so the median is $medianVal</p><p><b>Step 4 &mdash; Test each statement against the scores.</b> The mean is $relWord the median. $dupWord $meanWord $symWord</p><p><b>Answer:</b> mean = $meanVal, median = $medianVal. The true statements are:$trueList</p>";

