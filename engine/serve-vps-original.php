<?php
// mom render sandbox — POST question PHP (mom formats), get back a self-contained HTML page
// that renders the question as MyOpenMath would. No DB, no login.
error_reporting(E_ERROR | E_PARSE);
header("Access-Control-Allow-Origin: *");
header("Access-Control-Allow-Methods: POST, OPTIONS");
header("Access-Control-Allow-Headers: Content-Type");
if ($_SERVER["REQUEST_METHOD"] === "OPTIONS") { http_response_code(204); exit; }
if ($_SERVER["REQUEST_METHOD"] !== "POST") {
  header("Content-Type: text/html; charset=utf-8");
  echo "<!doctype html><h1>mom render sandbox</h1><p>POST a question .php body to render it.</p>";
  exit;
}

if (!function_exists("_")) { function _($s){ return $s; } }
spl_autoload_register(function($c){ if(strncmp($c,"IMathAS\\",8)===0){ $f=__DIR__."/".str_replace("\\","/",substr($c,8)).".php"; if(is_file($f)) require_once $f; }});
require_once __DIR__."/includes/Rand.php";
require_once __DIR__."/includes/sanitize.php";
require_once __DIR__."/filter/filter.php";
require_once __DIR__."/assess2/questions/QuestionGenerator.php";
use IMathAS\assess2\questions\QuestionGenerator;
use IMathAS\assess2\questions\models\QuestionParams;

// --- Parse the mom question file into IMathAS fields (control / qtext / answer). ---
// Two layouts exist: (1) separate "// === QUESTION TEXT ===" / "=== ANSWER ===" sections;
// (2) the native IMathAS blob living entirely inside "=== COMMON CONTROL (paste ...) ===",
// delimited internally by a "//question text" line then a lone "///" line before the answer.
$src = file_get_contents("php://input");
$parts = preg_split("/^\/\/ === (.+?) ===\s*$/m", $src, -1, PREG_SPLIT_DELIM_CAPTURE);
$sec = [];
for ($i=1;$i<count($parts);$i+=2){ $sec[trim($parts[$i])] = trim($parts[$i+1]); }

$qtype = "multipart";
foreach ($sec as $k=>$v){ if (preg_match("/SET QUESTION TYPE TO:\s*(\w+)/i",$k,$m)) $qtype=$m[1]; }

// Control = first section whose header starts with "COMMON CONTROL" (handles the "(paste ...)" variant).
$control = "";
foreach ($sec as $k=>$v){ if (stripos($k,"COMMON CONTROL")===0){ $control=$v; break; } }
$qtext  = $sec["QUESTION TEXT"] ?? "";
$answer = $sec["ANSWER"] ?? "";

// If there is no separate QUESTION TEXT section, the display lives inside the control blob.
if ($qtext === "") {
  if (preg_match("#^//\s*question text\s*$#mi", $control)) {
    $bits = preg_split("#^//\s*question text\s*$#mi", $control, 2);
    $control = rtrim($bits[0]);
    $after = $bits[1];
    $qa = preg_split("#^///\s*$#m", $after, 2);
    $qtext = trim($qa[0]);
    if (isset($qa[1]) && $answer === "") $answer = trim($qa[1]);
  } elseif (preg_match("#^///\s*$#m", $control)) {
    $qa = preg_split("#^///\s*$#m", $control, 2);
    $control = rtrim($qa[0]);
    $qtext = trim($qa[1]);
  }
}
// The answer/showanswer field just runs as statements after control (vars already defined) — harmless,
// and keeps parity with the known-good single-section path. Display comes from $qtext.
if ($answer !== "") $control .= "\n".$answer;

$seed = isset($_GET["seed"]) ? (int)$_GET["seed"] : 4242;
$rand=new Rand(); $rand->srand($seed); $GLOBALS["RND"]=$rand;
$dbh=new PDO("sqlite::memory:"); $_SESSION=["userprefs"=>[]];
$qdata=["qtype"=>$qtype,"control"=>$control,"qcontrol"=>"","qtext"=>$qtext,"answer"=>"","hasimg"=>0,"extref"=>"","solution"=>"","solutionopts"=>0];
$qp=(new QuestionParams())->setQuestionData($qdata)->setQuestionSeed($seed)
  ->setDbQuestionSetId(0)->setQuestionNumber(0)->setDisplayQuestionNumber(0)
  ->setAllQuestionAnswers([])->setAllQuestionAnswersAsNum([])->setAllQuestionAutosaves([])
  ->setScoreNonZero([])->setScoreIsCorrect([])->setStudentPartAttemptCount([])->setLastRawScores([])
  ->setStudentAttemptNumber(0)->setShowAnswer(0)->setShowHints(0)->setCorrectAnswerWrongFormat([])->setSeqPartDone([]);

$html = ""; $errs = null;
try {
  $gen=new QuestionGenerator($dbh,$rand,$qp); $q=$gen->getQuestion();
  $errs = $q->getErrors();
  $html = $q->getQuestionContent();
} catch (Throwable $t) {
  http_response_code(500);
  $html = "<pre style=\"color:#b00\">RENDER ERROR: ".htmlspecialchars($t->getMessage())."\n @ ".htmlspecialchars($t->getFile()).":".$t->getLine()."</pre>";
}

header("Content-Type: text/html; charset=utf-8");
?>
<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<script>window.MathJax={loader:{load:["input/asciimath","input/tex","output/chtml"]},asciimath:{delimiters:[["`","`"]]},tex:{inlineMath:[["\(","\)"],["$","$"]]}};</script>
<script src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/startup.js" id="MathJax-script" async></script>
<style>body{font:16px/1.5 -apple-system,Segoe UI,Roboto,sans-serif;max-width:760px;margin:1.5rem auto;padding:0 1rem;color:#1a1a1a}input[type=text]{font:inherit;padding:.25rem .4rem;border:1px solid #999;border-radius:4px}.qerr{color:#b00;font:13px monospace}</style>
</head><body>
<?php if (!empty($errs)) { echo "<div class=\"qerr\">".htmlspecialchars(is_array($errs)?implode(" | ",$errs):(string)$errs)."</div>"; } ?>
<?php echo $html; ?>
</body></html>
