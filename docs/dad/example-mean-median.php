// === NAME - DESCRIPTION: Mean, median, and properties of a seven-score quiz data set ===
// === SET QUESTION TYPE TO: multipart ===

// === COMMON CONTROL ===
$quizScores = array();
for ($i = 0; $i < 7; $i++) {
  $quizScores[$i] = rand(50, 100);
}
$sortedScores = array();
for ($i = 0; $i < 7; $i++) {
  $sortedScores[$i] = $quizScores[$i];
}
for ($i = 0; $i < 6; $i++) {
  for ($j = 0; $j < 6 - $i; $j++) {
    if ($sortedScores[$j] > $sortedScores[$j + 1]) {
      $swapVal = $sortedScores[$j];
      $sortedScores[$j] = $sortedScores[$j + 1];
      $sortedScores[$j + 1] = $swapVal;
    }
  }
}
$qSum = 0;
for ($i = 0; $i < 7; $i++) {
  $qSum = $qSum + $quizScores[$i];
}
$qMean = $qSum / 7;
$qMedian = $sortedScores[3];
$qMeanRounded = round($qMean, 2);
$qHasRepeat = false;
for ($i = 1; $i < 7; $i++) {
  if ($sortedScores[$i] == $sortedScores[$i - 1]) {
    $qHasRepeat = true;
  }
}
$qAboveMean = 0;
for ($i = 0; $i < 7; $i++) {
  if ($quizScores[$i] > $qMean) {
    $qAboveMean = $qAboveMean + 1;
  }
}
$s1 = $quizScores[0];
$s2 = $quizScores[1];
$s3 = $quizScores[2];
$s4 = $quizScores[3];
$s5 = $quizScores[4];
$s6 = $quizScores[5];
$s7 = $quizScores[6];
$t1 = $sortedScores[0];
$t2 = $sortedScores[1];
$t3 = $sortedScores[2];
$t4 = $sortedScores[3];
$t5 = $sortedScores[4];
$t6 = $sortedScores[5];
$t7 = $sortedScores[6];
$anstypes = 'number,number,multans';
$answer[0] = $qMeanRounded;
$answer[1] = $qMedian;
$questions[2] = array(
  'The mean of the seven scores is greater than the median.',
  'The median of the seven scores is greater than the mean.',
  'At least two of the seven scores are equal to each other.',
  'All seven scores are different from each other.',
  'More than half of the seven scores are greater than the mean.'
);
$qCorrect = '';
if ($qAboveMean > 3) {
  if ($qMean > $qMedian) {
    if ($qHasRepeat) {
      $qCorrect = '0,2,4';
    } else {
      $qCorrect = '0,3,4';
    }
  } else if ($qMean < $qMedian) {
    if ($qHasRepeat) {
      $qCorrect = '1,2,4';
    } else {
      $qCorrect = '1,3,4';
    }
  } else {
    if ($qHasRepeat) {
      $qCorrect = '2,4';
    } else {
      $qCorrect = '3,4';
    }
  }
} else {
  if ($qMean > $qMedian) {
    if ($qHasRepeat) {
      $qCorrect = '0,2';
    } else {
      $qCorrect = '0,3';
    }
  } else if ($qMean < $qMedian) {
    if ($qHasRepeat) {
      $qCorrect = '1,2';
    } else {
      $qCorrect = '1,3';
    }
  } else {
    if ($qHasRepeat) {
      $qCorrect = '2';
    } else {
      $qCorrect = '3';
    }
  }
}
$answer[2] = $qCorrect;

// === QUESTION TEXT ===
<p>A teacher records the quiz scores, out of 100 points, for seven students:</p>
<p><b>$s1, $s2, $s3, $s4, $s5, $s6, $s7</b></p>
<p><b>Part a.</b> Find the mean of the seven scores. Round your answer to two decimal places.</p>
<p>Mean = $answerbox[0]</p>
<p><b>Part b.</b> Find the median of the seven scores.</p>
<p>Median = $answerbox[1]</p>
<p><b>Part c.</b> Select every statement below that is true for this data set. More than one statement may be true.</p>
<p>$answerbox[2]</p>

// === ANSWER ===
$qCmpText = 'equal to the median';
if ($qMean > $qMedian) {
  $qCmpText = 'greater than the median';
}
if ($qMean < $qMedian) {
  $qCmpText = 'less than the median';
}
$qRepText = 'No two of the seven scores are the same, so the statement that at least two scores are equal is false and the statement that all seven are different is true.';
if ($qHasRepeat) {
  $qRepText = 'At least two of the seven scores are equal to each other, so the statement that at least two scores are equal is true and the statement that all seven are different is false.';
}
$qAboveText = 'Exactly ' . $qAboveMean . ' of the seven scores are greater than the mean, and four or more would be needed for more than half, so that statement is false.';
if ($qAboveMean > 3) {
  $qAboveText = 'Exactly ' . $qAboveMean . ' of the seven scores are greater than the mean, and four or more is more than half, so that statement is true.';
}
$solutionguide = '<p><b>Step 1. Find the mean.</b> The seven scores add to ' . $qSum . '. Dividing that total by 7 gives ' . $qMeanRounded . ' to two decimal places.</p><p><b>Step 2. Find the median.</b> In order the scores are ' . $t1 . ', ' . $t2 . ', ' . $t3 . ', ' . $t4 . ', ' . $t5 . ', ' . $t6 . ', ' . $t7 . '. With seven values in order the median is the fourth one, so the median is ' . $qMedian . '.</p><p><b>Step 3. Check each statement.</b> The mean is ' . $qMeanRounded . ', which is ' . $qCmpText . '. ' . $qRepText . ' ' . $qAboveText . '</p>';

