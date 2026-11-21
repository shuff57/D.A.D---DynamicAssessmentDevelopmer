// === NAME - DESCRIPTION: Slope between two random integer points and classification of the line — two-part: numeric slope plus multiple choice increasing/decreasing/horizontal/vertical, with the x-coordinates guaranteed distinct so the line is never vertical ===
// === SET QUESTION TYPE TO: multipart ===

// === COMMON CONTROL ===
$slope = randfrom(array(-4,-3,-2,-1,0,1,2,3,4));
$xA = rand(-5,3);
$runLen = rand(1,4);
$xB = $xA + $runLen;
$yA = rand(-6,6);
$yB = $yA + $slope*$runLen;

$anstypes = "number,choices";

$answer[0] = $slope;
if ($slope > 0) { $answer[1] = 0; }
elseif ($slope < 0) { $answer[1] = 1; }
else { $answer[1] = 2; }

$questions[1] = array("Increasing","Decreasing","Horizontal","Vertical");

// === QUESTION TEXT ===
<p>Two points that lie on the same straight line are given below.</p>
<p>Point A: `($xA, $yA)`</p>
<p>Point B: `($xB, $yB)`</p>
<p><b>Part a.</b> Find the slope of the line that passes through A and B. Enter an integer or a fraction.</p>
<p>Slope: $answerbox[0]</p>
<p><b>Part b.</b> As you move from left to right along this line, is the line increasing, decreasing, horizontal, or vertical?</p>
<p>$answerbox[1]</p>

// === ANSWER ===
$riseLen = $yB - $yA;
if ($slope > 0) { $kindWord = "increasing"; $signWord = "positive"; }
elseif ($slope < 0) { $kindWord = "decreasing"; $signWord = "negative"; }
else { $kindWord = "horizontal"; $signWord = "zero"; }

$solutionguide = '<details><summary>Worked solution</summary>';
$solutionguide .= '<p><b>Part a.</b> Slope is the change in y divided by the change in x. ';
$solutionguide .= 'Moving from point A to point B, the change in y is `' . $riseLen . '` and the change in x is `' . $runLen . '`. ';
$solutionguide .= 'Slope `= ' . $riseLen . ' / ' . $runLen . ' = ' . $slope . '`.</p>';
$solutionguide .= '<p><b>Part b.</b> The slope is ' . $signWord . ', so as x increases the line is ' . $kindWord . '. ';
$solutionguide .= 'The two points have different x-coordinates, so the line is not vertical.</p>';
$solutionguide .= '</details>';

