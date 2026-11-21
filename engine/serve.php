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
 * The question's own variables after its code runs, read the way ScoreEngine reads them: control
 * interpreted and evaluated at the seed, then the answer field at seed+1. The displayed answers are
 * formatted for reading ("4 × 3 = 12", "xge5"), so the self-test types $answer instead, which is
 * what the author expects a student to enter. Call after dad_render, which loads the macros.
 */
function dad_raw_vars(array $qdata, int $seed): array {
    $code = interpret('control', $qdata['qtype'], $qdata['control'], 1, [DAD_QSID]);
    $answerCode = empty($qdata['answer']) ? '' : interpret('answer', $qdata['qtype'], $qdata['answer'], 1, [DAD_QSID]);
    ob_start();
    try {
        $vars = dad_eval_question($code, $answerCode, $seed);
    } catch (Throwable $t) {
        ob_end_clean();
        return ['_error' => $t->getMessage() . ' @ line ' . $t->getLine()];
    }
    ob_end_clean();
    $out = [];
    foreach (['answer', 'answers', 'anstypes', 'questions', 'answersize', 'answerformat', 'displayformat'] as $v) {
        if (array_key_exists($v, $vars)) $out[$v] = $vars[$v];
    }
    if (isset($out['anstypes']) && !is_array($out['anstypes'])) {
        $out['anstypes'] = array_map('trim', explode(',', $out['anstypes']));
    }
    return $out;
}

/**
 * Runs question code in a scope holding only what ScoreEngine's scope offers question code, so a
 * question's own variable names cannot collide with ours. Every local here is double-underscored.
 */
function dad_eval_question(string $__control, string $__answer, int $__seed): array {
    $RND = $GLOBALS['RND'];
    $qnidx = 0; $thisq = 1; $attemptn = 0; $currentseed = $__seed;
    $graphdispmode = 1; $drawentrymode = 1;
    $stuanswers = []; $stuanswersval = []; $scorenonzero = []; $scoreiscorrect = [];
    $RND->srand($__seed);
    if ($__control !== 'error;') eval($__control);
    $RND->srand($__seed + 1);
    if ($__answer !== '') eval($__answer);
    return array_filter(get_defined_vars(), fn($k) => !str_starts_with($k, '__'), ARRAY_FILTER_USE_KEY);
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

/** Collapse whitespace, drop real tags and AsciiMath backticks, for comparing display text.
 *  Only "<" followed by a letter, "/" or "!" is a tag, so "X <= k" keeps its "<=". */
function dad_norm(string $s): string {
    $s = preg_replace('/<[a-z\/!][^>]*>/i', ' ', $s);
    $s = html_entity_decode($s, ENT_QUOTES | ENT_HTML5, 'UTF-8');
    return trim(preg_replace('/\s+/u', ' ', str_replace('`', '', $s)));
}

/** A display answer turned into what a student types: AsciiMath words to symbols, × to *. */
function dad_display_to_typed(string $display): string {
    $t = ' ' . dad_norm($display) . ' ';
    $t = preg_replace(['/(?<=[\s\d)a-z])le(?=[\s\d(-])/', '/(?<=[\s\d)a-z])ge(?=[\s\d(-])/',
                       '/(?<=[\s\d)a-z])lt(?=[\s\d(-])/', '/(?<=[\s\d)a-z])gt(?=[\s\d(-])/',
                       '/(?<=[\s\d)a-z])ne(?=[\s\d(-])/'],
                      ['<=', '>=', '<', '>', '!='], $t);
    $t = str_replace(['×', '÷', '≤', '≥', '≠', '−'], ['*', '/', '<=', '>=', '!=', '-'], $t);
    return trim(preg_replace('/\s+/u', ' ', $t));
}

/** IMathAS shows several accepted answers as "A or B or C"; a student types one of them.
 *  Strip tags only when the value looks like HTML; "y <= -4x-2" must keep its "<". */
function dad_first_alternative(string $value): string {
    $clean = preg_match('/<[a-z\/!][^>]*>/i', $value) ? dad_norm($value) : trim(preg_replace('/\s+/u', ' ', str_replace('`', '', $value)));
    $parts = preg_split('/\s+or\s+/', $clean);
    return $parts[0] ?? '';
}

const DAD_MANUAL_TYPES = ['essay', 'file', 'draw'];

/**
 * For each part, the form fields a student would try, best guess first, from the rendered page,
 * IMathAS's displayed answers and the question's own variables. Returns [candidates, problems,
 * untested]: candidates[pn] is a list of field maps. Parts a machine cannot judge (essay, upload,
 * drawing, options that are only pictures) are untested: a teacher must look, but they are not
 * failures. A part we should be able to answer but cannot is a problem.
 */
function dad_student_candidates(array $render, bool $multi, string $qtype, array $raw = []): array {
    $cands = []; $problems = []; $untested = [];
    $doc = new DOMDocument();
    libxml_use_internal_errors(true);
    $doc->loadHTML('<?xml encoding="utf-8"?><div>' . $render['html'] . '</div>');
    libxml_clear_errors();
    $xp = new DOMXPath($doc);
    $labelText = function (DOMElement $in) use ($xp, $doc): string {
        $label = $xp->query("//label[@for='" . $in->getAttribute('id') . "']")->item(0);
        return $label ? dad_norm($doc->saveHTML($label)) : '';
    };
    $rawPart = function (string $var, int $pn) use ($raw, $multi) {
        if (!array_key_exists($var, $raw)) return null;
        $v = $raw[$var];
        if ($multi) return is_array($v) ? ($v[$pn] ?? null) : null;
        return $v;
    };
    // Every part, not only the ones IMathAS shows an answer for.
    $parts = array_keys($render['answers']);
    $nparts = max(count($render['answeights']), $multi && isset($raw['anstypes']) ? count($raw['anstypes']) : 1);
    for ($i = 0; $i < $nparts; $i++) if (!in_array($i, $parts)) $parts[] = $i;
    sort($parts);

    foreach ($parts as $pn) {
        $pn = (int)$pn;
        $ans = $render['answers'][$pn] ?? '';
        $key = $multi ? 1000 + $pn : 0;
        $name = "qn$key";
        $type = $render['jsparams'][$key]['qtype'] ?? $render['jsparams'][(string)$key]['qtype']
             ?? ($multi ? ($raw['anstypes'][$pn] ?? null) : null) ?? $qtype;
        if (in_array($type, DAD_MANUAL_TYPES, true)) { $untested[] = $pn; continue; }

        if ($type === 'choices' || $type === 'multans') {
            $inputs = iterator_to_array($xp->query("//input[@name='$name' or starts-with(@name,'{$name}[')]"));
            $options = iterator_to_array($xp->query("//select[@name='$name']/option[@value!='NA']"));
            $texts = array_map(fn($el) => $el->tagName === 'option' ? dad_norm($el->textContent) : $labelText($el), array_merge($inputs, $options));
            if ($texts && !array_filter($texts, fn($t) => $t !== '')) { $untested[] = $pn; continue; }   // picture options
            // Question defects a student would trip on, whatever the answer key says.
            $dups = array_unique(array_diff_assoc($texts, array_unique($texts)));
            if ($dups) $problems[] = "part $pn ($type): duplicate options " . json_encode(array_values($dups)) . ' - picking the other copy is marked wrong';
            $qs = $rawPart('questions', $pn);
            if ($options && is_array($qs)) foreach ($qs as $q) {
                if (preg_match('/<[a-z\/][^>]*>|`/i', (string)$q)) { $problems[] = "part $pn (choices): a dropdown option holds HTML or backtick math, which a dropdown shows as raw text"; break; }
            }
        }

        if ($type === 'choices') {
            $wants = [dad_norm((string)$ans)];
            $qs = $rawPart('questions', $pn); $ai = $rawPart('answer', $pn);
            if (is_array($qs) && is_scalar($ai) && isset($qs[(int)$ai])) $wants[] = dad_norm((string)$qs[(int)$ai]);
            $squash = fn($t) => preg_replace('/\s+/u', '', $t);
            $wantsSq = array_map($squash, $wants);
            $value = null;
            foreach ($inputs as $in) if (in_array($squash($labelText($in)), $wantsSq, true)) { $value = $in->getAttribute('value'); break; }
            if ($value === null) foreach ($options as $opt) if (in_array($squash(dad_norm($opt->textContent)), $wantsSq, true)) { $value = $opt->getAttribute('value'); break; }
            // With raw access, the answer index maps straight to the option whose text is $questions[$answer].
            if ($value === null) { $problems[] = "part $pn (choices): no option matches the correct answer"; continue; }
            $cands[$pn] = [[$name => $value]];
        } elseif ($type === 'multans') {
            $want = array_values(array_filter(array_map('dad_norm', preg_split('/<br\s*\/?>|\n/i', (string)$ans))));
            $picked = [];
            foreach ($inputs as $in) {
                if (in_array($labelText($in), $want, true)) {
                    $picked[(int)substr($in->getAttribute('name'), strlen($name) + 1)] = $in->getAttribute('value');
                }
            }
            if (!$want || count($picked) !== count($want)) { $problems[] = "part $pn (multans): matched " . count($picked) . " of " . count($want) . " correct options"; continue; }
            $cands[$pn] = [[$name => $picked]];
        } elseif ($type === 'matching') {
            $letters = preg_split('/\s+/', trim(dad_norm((string)$ans)));
            $fields = [];
            foreach ($letters as $i => $l) {
                if (!preg_match('/^[a-z]$/', $l)) { $problems[] = "part $pn (matching): unreadable answer \"$l\""; continue 2; }
                $fields["$name-$i"] = (string)(ord($l) - 97);
            }
            $cands[$pn] = [$fields];
        } elseif (in_array($type, ['matrix', 'calcmatrix'], true)) {
            // With a fixed size ("matrixsize"), each cell is its own box qnK-0, qnK-1, ... row by row;
            // without one, the student types the whole matrix "[(1,2),(3,4)]" into one box.
            $r = $rawPart('answer', $pn);
            $m = dad_first_alternative(is_scalar($r) && trim((string)$r) !== '' ? (string)$r : (string)$ans);
            $size = $render['jsparams'][$key]['matrixsize'] ?? null;
            if ($size) {
                $cells = preg_split('/\s*,\s*/', trim(preg_replace('/[\[\]()]/', ' ', str_replace('),(', ',', $m)), " ,"));
                $fields = [];
                foreach ($cells as $i => $c) $fields["$name-$i"] = $c;
                $cands[$pn] = [$fields];
            } else {
                $cands[$pn] = [[$name => $m]];
            }
        } elseif (is_array($ans)) {
            $fields = [];
            foreach (array_values($ans) as $i => $v) $fields["$name-$i"] = dad_first_alternative((string)$v);
            $cands[$pn] = [$fields];
        } else {
            // Typed answers: the author's $answer first, then the display form as typed.
            $tries = [];
            $r = $rawPart('answer', $pn);
            if (is_scalar($r) && trim((string)$r) !== '') $tries[] = dad_first_alternative((string)$r);
            if (trim((string)$ans) !== '') {
                $tries[] = dad_first_alternative(dad_display_to_typed((string)$ans));
                $tries[] = dad_first_alternative((string)$ans);
            }
            $tries = array_values(array_unique(array_filter($tries, fn($t) => $t !== '')));
            if (!$tries) { $problems[] = "part $pn ($type): no correct answer to try"; continue; }
            $cands[$pn] = array_map(fn($t) => [$name => $t], array_slice($tries, 0, 4));
        }
    }
    return [$cands, $problems, $untested];
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

if ($path === '/api/vars') {
    // The question's variables after its code runs: what the self-test types, and a debugging aid.
    $seed = (int)($body['seed'] ?? 4242);
    $render = dad_render($dbh, $qdata, $seed);
    dad_json(['vars' => dad_raw_vars($qdata, $seed), 'displayed' => $render['answers'],
              'parts' => array_filter($render['jsparams'], fn($k) => is_numeric($k), ARRAY_FILTER_USE_KEY)]);
}

if ($path === '/api/selftest') {
    $seeds = array_slice(array_map('intval', (array)($body['seeds'] ?? [4242, 1, 2, 3, 5])), 0, 20);
    $runs = [];
    $pass = true;
    $untestedAll = [];
    foreach ($seeds as $seed) {
        $render = dad_render($dbh, $qdata, $seed);
        $multi = dad_is_multipart($qdata, $render['answeights']);
        $raw = dad_raw_vars($qdata, $seed);
        [$cands, $problems, $untested] = dad_student_candidates($render, $multi, $qdata['qtype'], $raw);
        if (isset($raw['_error'])) $problems[] = 'could not read the question variables: ' . $raw['_error'];
        // Like a student: submit the best guess for every part, then retry only the parts that
        // missed with their next candidate form, up to four rounds.
        $pick = array_fill_keys(array_keys($cands), 0);
        for ($round = 0; $round < 4; $round++) {
            $post = [];
            foreach ($cands as $pn => $list) $post += $list[$pick[$pn]];
            $correct = dad_score($dbh, $qdata, $seed, [], $multi, $post);
            $advanced = false;
            foreach ($cands as $pn => $list) {
                if ((float)($correct['raw'][$pn] ?? 0) < 0.98 && $pick[$pn] + 1 < count($list)) { $pick[$pn]++; $advanced = true; }
            }
            if (!$advanced) break;
        }
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
