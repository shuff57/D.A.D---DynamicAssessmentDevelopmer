<?php
/**
 * D.A.D engine: stateless IMathAS. One question per request, no database, no login.
 *
 *   POST /                text/plain mom question  -> self-contained HTML page (the VPS sandbox's
 *                         original contract, kept so steve-desktop's health check keeps working).
 *                         ?seed=N picks the seed (default 4242).
 *   POST /api/render      {"src", "seed"?, "showans"?}              -> {html, jsparams, answers, answeights, errors}
 *   POST /api/score       {"src", "seed"?, "answers": {pn: value}}  -> {scores, raw, total, answeights, errors}
 *   POST /api/selftest    {"src", "seeds"?: [..]}                   -> per seed: the question's own correct
 *                         answers scored (should be full marks) and blanks scored (should be 0).
 *
 * "src" is the mom format (see momformat.php). Instead of "src", a body may carry "question":
 * {"qtype", "control", "qtext", "answer"?} in IMathAS's own fields.
 */
error_reporting(E_ERROR | E_PARSE);
ini_set('display_errors', '0');

$path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH);
$method = $_SERVER['REQUEST_METHOD'] ?? 'GET';

// The engine is meant to sit behind the D.A.D API. The open CORS policy is only for local
// development and the legacy endpoint; set DAD_ENGINE_CORS=0 to drop it.
if (getenv('DAD_ENGINE_CORS') !== '0') {
    header('Access-Control-Allow-Origin: *');
    header('Access-Control-Allow-Methods: POST, OPTIONS');
    header('Access-Control-Allow-Headers: Content-Type');
}
if ($method === 'OPTIONS') { http_response_code(204); exit; }
if ($path === '/health') { header('Content-Type: text/plain'); echo "ok\n"; exit; }
if ($method !== 'POST') {
    header('Content-Type: text/html; charset=utf-8');
    echo '<!doctype html><h1>D.A.D engine</h1><p>POST a question to /, /api/render, /api/score or /api/selftest.</p>';
    exit;
}

// --- IMathAS bootstrap, without init.php (which wants a database and a session). ---
if (!function_exists('_')) { function _($s) { return $s; } }
spl_autoload_register(function ($c) {
    if (strncmp($c, 'IMathAS\\', 8) === 0) {
        $f = __DIR__ . '/' . str_replace('\\', '/', substr($c, 8)) . '.php';
        if (is_file($f)) require_once $f;
    }
});
$GLOBALS['imasroot'] = '';
$GLOBALS['staticroot'] = '';
$GLOBALS['CFG'] = [];
$GLOBALS['myrights'] = 100;          // teacher view: IMathAS shows question-code errors to authors
$_SESSION = ['userprefs' => ['graphdisp' => 1, 'drawentry' => 1, 'mathdisp' => 1, 'useed' => 0],
             'graphdisp' => 1, 'mathdisp' => 1, 'drawentry' => 1];
require_once __DIR__ . '/includes/Rand.php';
require_once __DIR__ . '/includes/sanitize.php';
require_once __DIR__ . '/filter/filter.php';
require_once __DIR__ . '/assess2/AssessStandalone.php';
require_once __DIR__ . '/momformat.php';
$GLOBALS['RND'] = new Rand();
$dbh = new PDO('sqlite::memory:');

const DAD_QSID = 1;

function dad_json($data, int $status = 200): void {
    http_response_code($status);
    header('Content-Type: application/json; charset=utf-8');
    echo json_encode($data, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_PARTIAL_OUTPUT_ON_ERROR);
    exit;
}

function dad_question_from_body(array $body): array {
    if (isset($body['src']) && is_string($body['src'])) return dad_parse_mom($body['src']);
    if (isset($body['question']) && is_array($body['question'])) {
        $q = $body['question'];
        return array_merge(dad_parse_mom(''), [
            'qtype' => (string)($q['qtype'] ?? 'multipart'),
            'control' => (string)($q['control'] ?? ''),
            'qtext' => (string)($q['qtext'] ?? ''),
            'answer' => (string)($q['answer'] ?? ''),
        ]);
    }
    dad_json(['error' => 'body needs "src" (mom format) or "question"'], 400);
}

/** A fresh standalone runner for one question at one seed. */
function dad_runner(PDO $dbh, array $qdata, int $seed): AssessStandalone {
    $a = new AssessStandalone($dbh);
    $a->setQuestionData(DAD_QSID, $qdata);
    $a->setState(['seeds' => [0 => $seed], 'qsid' => [0 => DAD_QSID]]);
    return $a;
}

/** Render; the correct answers come back too (IMathAS's "includeans"). */
function dad_render(PDO $dbh, array $qdata, int $seed, bool $showans = false): array {
    $a = dad_runner($dbh, $qdata, $seed);
    ob_start();
    try {
        $out = $a->displayQuestion(0, ['includeans' => true, 'showans' => $showans]);
    } catch (Throwable $t) {
        ob_end_clean();
        return ['html' => '', 'jsparams' => [], 'answers' => [], 'answeights' => [],
                'errors' => ['RENDER ERROR: ' . $t->getMessage() . ' @ ' . basename($t->getFile()) . ':' . $t->getLine()]];
    }
    $stray = trim(ob_get_clean());
    $q = $a->getQuestion();
    $errors = array_values(array_filter(array_map('strval', (array)($out['errors'] ?? []))));
    if ($stray !== '') $errors[] = 'output: ' . mb_substr(strip_tags($stray), 0, 500);
    return [
        'html' => (string)$out['html'],
        'jsparams' => $out['jsparams'] ?? [],
        'answers' => $q ? $q->getCorrectAnswersForParts() : [],
        'answeights' => $q ? $q->getAnswerPartWeights() : [],
        'errors' => $errors,
    ];
}

/**
 * Score answers. $answers maps part number => value (a string, or a list for matrix/matching parts).
 * IMathAS reads answers from $_POST: single-part "qn0", multipart "qn1000"+part.
 */
function dad_score(PDO $dbh, array $qdata, int $seed, array $answers, bool $multipart, ?array $post = null): array {
    $a = dad_runner($dbh, $qdata, $seed);
    $_POST = $post ?? [];
    if ($post === null) foreach ($answers as $pn => $val) {
        $key = $multipart ? 'qn' . (1000 + (int)$pn) : 'qn0';
        if (is_array($val)) {
            foreach (array_values($val) as $i => $v) $_POST["$key-$i"] = (string)$v;
        } else {
            $_POST[$key] = (string)$val;
        }
    }
    ob_start();
    try {
        $r = $a->scoreQuestion(0);
    } catch (Throwable $t) {
        ob_end_clean();
        $_POST = [];
        return ['scores' => [], 'raw' => [], 'total' => 0, 'answeights' => [],
                'errors' => ['SCORE ERROR: ' . $t->getMessage() . ' @ ' . basename($t->getFile()) . ':' . $t->getLine()]];
    }
    ob_end_clean();
    $_POST = [];
    return [
        'scores' => $r['scores'],
        'raw' => $r['raw'],
        'total' => round(array_sum($r['scores']), 4),
        'answeights' => $r['answeights'],
        'errors' => array_values(array_filter(array_map('strval', (array)$r['errors']))),
    ];
}

function dad_is_multipart(array $qdata, array $answeights): bool {
    return in_array($qdata['qtype'], ['multipart', 'conditional'], true) || count($answeights) > 1;
}

/** Collapse whitespace, drop tags and AsciiMath backticks, for comparing display text. */
function dad_norm(string $s): string {
    $s = html_entity_decode(strip_tags($s), ENT_QUOTES | ENT_HTML5, 'UTF-8');
    return trim(preg_replace('/\s+/u', ' ', str_replace('`', '', $s)));
}

/**
 * Build the form fields a student would submit to get each part right, from the rendered page and
 * IMathAS's displayed correct answers. Returns [post map, problems, untested part numbers].
 * Parts a machine cannot judge (essay, file upload, drawing) are listed as untested: they need a
 * teacher's look, but they are not failures. A part we should be able to answer but cannot is a
 * problem, so the self-test reports it instead of guessing.
 */
const DAD_MANUAL_TYPES = ['essay', 'file', 'draw'];

function dad_student_post(array $render, bool $multi, string $qtype): array {
    $post = []; $problems = []; $untested = [];
    $doc = new DOMDocument();
    libxml_use_internal_errors(true);
    $doc->loadHTML('<?xml encoding="utf-8"?><div>' . $render['html'] . '</div>');
    libxml_clear_errors();
    $xp = new DOMXPath($doc);
    $labelText = function (DOMElement $in) use ($xp, $doc): string {
        $label = $xp->query("//label[@for='" . $in->getAttribute('id') . "']")->item(0);
        return $label ? dad_norm($doc->saveHTML($label)) : '';
    };
    foreach ($render['answers'] as $pn => $ans) {
        $key = $multi ? 1000 + (int)$pn : 0;
        $type = $render['jsparams'][$key]['qtype'] ?? $render['jsparams'][(string)$key]['qtype'] ?? $qtype;
        $name = "qn$key";
        if (in_array($type, DAD_MANUAL_TYPES, true)) {
            $untested[] = (int)$pn;
        } elseif ($type === 'choices') {
            // Radio buttons, or a dropdown when the question uses displayformat="select".
            $want = dad_norm((string)$ans);
            $value = null;
            foreach ($xp->query("//input[@name='$name']") as $in) {
                if ($labelText($in) === $want) { $value = $in->getAttribute('value'); break; }
            }
            if ($value === null) foreach ($xp->query("//select[@name='$name']/option") as $opt) {
                if (dad_norm($opt->textContent) === $want) { $value = $opt->getAttribute('value'); break; }
            }
            if ($value === null) { $problems[] = "part $pn (choices): no option matches the correct answer"; continue; }
            $post[$name] = $value;
        } elseif ($type === 'multans') {
            // Checkboxes named qnK[i]; IMathAS shows the correct ones joined by <br/>.
            $want = array_values(array_filter(array_map('dad_norm', preg_split('/<br\s*\/?>|\n/i', (string)$ans))));
            $picked = [];
            foreach ($xp->query("//input[starts-with(@name,'{$name}[')]") as $in) {
                if (in_array($labelText($in), $want, true)) {
                    $picked[(int)substr($in->getAttribute('name'), strlen($name) + 1)] = $in->getAttribute('value');
                }
            }
            if (count($picked) !== count($want)) { $problems[] = "part $pn (multans): matched " . count($picked) . " of " . count($want) . " correct options"; continue; }
            $post[$name] = $picked;
        } elseif ($type === 'matching') {
            // IMathAS shows the answer as the letter for each prompt in display order: "a d f e b h".
            $letters = preg_split('/\s+/', trim(dad_norm((string)$ans)));
            foreach ($letters as $i => $l) {
                if (!preg_match('/^[a-z]$/', $l)) { $problems[] = "part $pn (matching): unreadable answer \"$l\""; continue 2; }
                $post["$name-$i"] = (string)(ord($l) - 97);
            }
        } elseif (is_array($ans)) {
            foreach (array_values($ans) as $i => $v) $post["$name-$i"] = dad_first_alternative((string)$v);
        } else {
            $post[$name] = dad_first_alternative((string)$ans);
        }
    }
    return [$post, $problems, $untested];
}

/** IMathAS shows several accepted answers as "A or B or C"; a student types one of them. */
function dad_first_alternative(string $display): string {
    $parts = preg_split('/\s+or\s+/', dad_norm($display));
    return $parts[0] ?? '';
}

// --- Routes ---

if ($path === '/' || $path === '') {
    $qdata = dad_parse_mom(file_get_contents('php://input'));
    $seed = isset($_GET['seed']) ? (int)$_GET['seed'] : 4242;
    $r = dad_render($dbh, $qdata, $seed);
    header('Content-Type: text/html; charset=utf-8');
    $errs = $r['errors'];
    ?>
<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<script>window.MathJax={loader:{load:["input/asciimath","input/tex","output/chtml"]},asciimath:{delimiters:[["`","`"]]},tex:{inlineMath:[["\\(","\\)"],["$","$"]]}};</script>
<script src="https://cdn.jsdelivr.net/npm/mathjax@3.2.2/es5/startup.js" id="MathJax-script" async></script>
<style>body{font:16px/1.5 -apple-system,Segoe UI,Roboto,sans-serif;max-width:760px;margin:1.5rem auto;padding:0 1rem;color:#1a1a1a}input[type=text]{font:inherit;padding:.25rem .4rem;border:1px solid #999;border-radius:4px}.qerr{color:#b00;font:13px monospace}</style>
</head><body>
<?php if (!empty($errs)) { echo '<div class="qerr">' . htmlspecialchars(implode(' | ', $errs)) . '</div>'; } ?>
<?php echo $r['html']; ?>
</body></html>
<?php
    exit;
}

$body = json_decode(file_get_contents('php://input'), true);
if (!is_array($body)) dad_json(['error' => 'body must be JSON'], 400);
$qdata = dad_question_from_body($body);

if ($path === '/api/render') {
    $seed = (int)($body['seed'] ?? 4242);
    dad_json(dad_render($dbh, $qdata, $seed, !empty($body['showans'])));
}

if ($path === '/api/score') {
    $seed = (int)($body['seed'] ?? 4242);
    $answers = is_array($body['answers'] ?? null) ? $body['answers'] : [];
    $render = dad_render($dbh, $qdata, $seed);
    dad_json(dad_score($dbh, $qdata, $seed, $answers, dad_is_multipart($qdata, $render['answeights'])));
}

if ($path === '/api/selftest') {
    $seeds = array_slice(array_map('intval', (array)($body['seeds'] ?? [4242, 1, 2, 3, 5])), 0, 20);
    $runs = [];
    $pass = true;
    $untestedAll = [];
    foreach ($seeds as $seed) {
        $render = dad_render($dbh, $qdata, $seed);
        $multi = dad_is_multipart($qdata, $render['answeights']);
        [$post, $problems, $untested] = dad_student_post($render, $multi, $qdata['qtype']);
        $correct = dad_score($dbh, $qdata, $seed, [], $multi, $post);
        $blank = dad_score($dbh, $qdata, $seed, [], $multi, []);
        if ($render['errors']) $problems[] = 'render errors';
        if ($correct['errors']) $problems[] = 'score errors';
        if (!$render['answers']) $problems[] = 'no correct answers exposed';
        // Judge each machine-gradable part on its own; essay/file/draw parts are left to the teacher.
        foreach ((array)$correct['raw'] as $pn => $raw) {
            if (in_array((int)$pn, $untested, true)) continue;
            if ((float)$raw < 0.98) $problems[] = "part $pn: correct answer scored " . round((float)$raw, 3);
        }
        foreach ((array)$blank['raw'] as $pn => $raw) {
            if (in_array((int)$pn, $untested, true)) continue;
            if ((float)$raw > 0.001) $problems[] = "part $pn: blank answer scored " . round((float)$raw, 3);
        }
        if ($problems) $pass = false;
        $untestedAll = array_values(array_unique(array_merge($untestedAll, $untested)));
        $runs[] = [
            'seed' => $seed,
            'ok' => !$problems,
            'problems' => $problems,
            'answers' => $render['answers'],
            'submitted' => $post,
            'correct' => $correct,
            'blank' => ['total' => $blank['total'], 'raw' => $blank['raw']],
            'render_errors' => $render['errors'],
        ];
    }
    // "untested" parts (essay, file upload, drawing) passed nothing and failed nothing: a teacher must look.
    dad_json(['pass' => $pass, 'qtype' => $qdata['qtype'], 'untested_parts' => $untestedAll, 'runs' => $runs]);
}

dad_json(['error' => "no route $path"], 404);
