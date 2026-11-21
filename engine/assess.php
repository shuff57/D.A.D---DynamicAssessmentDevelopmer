<?php
/**
 * The sandbox as students see MyOpenMath: IMathAS's own assessment player (the assess2 Vue app,
 * served from the static files), with its server calls answered here instead of by MyOpenMath.
 *
 * MyOpenMath's player asks its server for loadassess, startassess, loadquestion, scorequestion,
 * autosave and endassess. The page routes those calls (jQuery, by URL) to the D.A.D engine, which
 * answers them in the shapes MyOpenMath answers (taken from a recorded teacher preview, 2026-11-21)
 * for a one-question assessment built around the question being tried. The question itself is
 * rendered and scored by IMathAS (AssessStandalone), as in the embedded view.
 *
 * POST /api/assess {src | question, static, engine, call?, state?}
 *   without call: {html} - the whole page.
 *   with call {name, post}: the player's JSON answer, plus `_dadstate` (the sandbox's state, which
 *     the page keeps and sends back with the next call; it never reaches the player).
 */

const DAD_ASSESS_QN = 0;
const DAD_ASSESS_POINTS = 1;
const DAD_ASSESS_TRIES = 3;
const DAD_ASSESS_REGENS = 100;

function dad_assess(PDO $dbh, array $qdata, array $body): void {
    $qn = DAD_ASSESS_QN;
    // The worked solution, as MyOpenMath shows it once answers are shown (see student.php).
    if (($qdata['solution'] ?? '') === '' && preg_match('/^\s*\$solutionguide\s*\.?=/m', $qdata['control'] ?? '')) {
        $qdata['solution'] = '$solutionguide';
    }
    if (($qdata['solution'] ?? '') !== '') $qdata['solutionopts'] = 7;
    // What IMathAS's "Written Example" link reads from a MyOpenMath server (a signed showsoln.php
    // address); the sandbox answers that link itself.
    $_SESSION['secsalt'] = $_SESSION['secsalt'] ?? 'dad-sandbox';
    $GLOBALS['basesiteurl'] = $GLOBALS['basesiteurl'] ?? '';
    $GLOBALS['cid'] = $GLOBALS['cid'] ?? 0;
    $name = trim((string)($body['name'] ?? '')) ?: 'Practice question';

    $st = is_array($body['state'] ?? null) ? $body['state'] : null;
    if ($st === null) {
        $st = ['started' => false, 'regen' => 0, 'try' => 0, 'a2' => dad_assess_fresh($body['seed'] ?? null)];
    }
    $call = is_array($body['call'] ?? null) ? $body['call'] : null;
    if ($call === null) {
        dad_json(['html' => dad_assess_page($body, $name)]);
    }
    $post = is_array($call['post'] ?? null) ? $call['post'] : [];
    $a2 = new AssessStandalone($dbh);
    $a2->setQuestionData(DAD_QSID, $qdata);

    switch ((string)($call['name'] ?? '')) {
        case 'loadassess':
            $out = dad_assess_settings($name, $st);
            break;
        case 'startassess':
            $st['started'] = true;
            $a2->setState($st['a2']);
            $out = dad_assess_dates() + [
                'prev_attempts' => [], 'scored_attempt' => ['score' => 0, 'kept' => 0],
                'available' => 'yes', 'submitby' => 'by_question', 'timelimit' => 0, 'timelimit_type' => 'kick_out',
                'showscores' => 'during', 'intro' => '', 'interquestion_text' => [], 'resources' => [],
                'help_features' => ['message' => false, 'forum' => 0], 'points_possible' => DAD_ASSESS_POINTS,
                'showcat' => 1, 'displaymethod' => 'skip', 'retakewait' => 0,
                'drillsettings' => ['style' => 'time_maxcorrect', 'n' => 10, 'dispnames' => new stdClass()],
                'preview_all' => false, 'show_results' => true, 'has_active_attempt' => true,
                'questions' => [$qn => dad_assess_question($a2, $st, false)],
                'feedback' => '', 'in_practice' => false, 'drawalt' => false,
            ];
            break;
        case 'loadquestion':
            if (!empty($post['regen']) && (string)$post['regen'] !== '0') {
                $st['regen']++;
                $st['try'] = 0;
                $st['a2'] = dad_assess_fresh(null);
            }
            $a2->setState($st['a2']);
            $out = dad_assess_dates() + [
                'available' => 'yes', 'submitby' => 'by_question', 'showscores' => 'during', 'timelimit' => 0,
                'drillsettings' => ['style' => 'time_maxcorrect', 'n' => 10, 'dispnames' => new stdClass()],
                'has_active_attempt' => true, 'saved_autosaves' => false,
                'questions' => [$qn => dad_assess_question($a2, $st, true)],
            ];
            break;
        case 'scorequestion':
            $a2->setState($st['a2']);
            $_POST = $post;   // the answer fields, as the player posted them
            $toscore = json_decode((string)($post['toscoreqn'] ?? '{}'), true) ?: [];
            $parts = [];
            foreach (($toscore[$qn] ?? $toscore[(string)$qn] ?? []) as $pn) $parts[(int)$pn] = true;
            $a2->scoreQuestion($qn, $parts ?: true);
            $st['a2'] = $a2->getState();
            $st['try']++;
            $out = dad_assess_dates() + [
                'available' => 'yes', 'submitby' => 'by_question', 'showscores' => 'during', 'timelimit' => 0,
                'retakewait' => 0, 'drillsettings' => ['style' => 'time_maxcorrect', 'n' => 10, 'dispnames' => new stdClass()],
                'has_active_attempt' => true, 'saved_autosaves' => false,
                'questions' => [$qn => dad_assess_question($a2, $st, true)],
                'showwork_after' => false, 'showwork_cutoff' => 0,
            ];
            break;
        case 'autosave':
            $out = ['autosave' => 'done'];
            break;
        case 'endassess':
            $out = dad_assess_settings($name, $st);
            break;
        case 'showsoln':
            // The "Written Example" help: on MyOpenMath a popup (assessment/showsoln.php) with the
            // worked solution for this student's version.
            $a2->setState($st['a2']);
            $d = $a2->displayQuestion($qn, ['includeans' => true]);
            $out = ['html' => (string)($d['soln'] ?? '')];
            break;
        default:
            $out = ['error' => 'unknown_call'];
    }
    $out['_dadstate'] = $st;
    dad_json($out);
}

/** A new version of the question: a fresh seed and no answers. */
function dad_assess_fresh($seed): array {
    $qn = DAD_ASSESS_QN;
    return [
        'seeds' => [$qn => $seed === null ? rand(1, 9999) : (int)$seed], 'qsid' => [$qn => DAD_QSID],
        'stuanswers' => [], 'stuanswersval' => [],
        'scorenonzero' => [($qn + 1) => -1], 'scoreiscorrect' => [($qn + 1) => -1],
        'partattemptn' => [$qn => []], 'rawscores' => [$qn => []],
        'useda11yalt' => [$qn => false],
    ];
}

function dad_assess_dates(): array {
    $start = time() - 86400;
    return [
        'startdate' => $start, 'enddate' => 2000000000, 'enddate_in' => 2000000000 - time(),
        'startdate_disp' => date('D n/j/y, g:i a', $start), 'enddate_disp' => 'None',
    ];
}

/** What loadassess.php answers: the assessment's settings, as a MyOpenMath homework set. */
function dad_assess_settings(string $name, array $st): array {
    return [
        'name' => $name, 'summary' => '', 'available' => 'yes',
        'timelimit' => 0, 'timelimit_type' => 'kick_out', 'points_possible' => DAD_ASSESS_POINTS,
        'submitby' => 'by_question', 'displaymethod' => 'skip', 'groupmax' => 6, 'isgroup' => 0,
        'showscores' => 'during', 'viewingb' => 'immediately', 'scoresingb' => 'immediately',
        'can_use_latepass' => 0, 'retake_penalty' => ['penalty' => 0, 'n' => 1], 'exceptionpenalty' => 0,
        'exceptionpenaltyinterval' => 0, 'earlybonus' => [0, 0], 'active_late_penalty' => ['penalty' => 0, 'interval' => 0],
        'current_late_penalty_pct' => 0, 'timelimit_multiplier' => 1, 'latepasses_avail' => 0, 'keepscore' => 'best',
        'noprint' => 0, 'overtime_penalty' => 0, 'overtime_grace' => 0, 'showworktype' => 0, 'singleshowwork' => 0,
        'latepass_enddate' => 2000000000, 'latepass_reason' => 0, 'retakewait' => 0,
        'drillsettings' => ['style' => 'time_maxcorrect', 'n' => 10, 'dispnames' => new stdClass()],
        'can_view_all' => false, 'is_teacher' => false, 'userid' => 1, 'is_lti' => false, 'is_diag' => false,
        'has_password' => false, 'has_active_attempt' => !empty($st['started']), 'has_unsubmitted_scored' => false,
        'prev_attempts' => [], 'scored_attempt' => [], 'can_retake' => true, 'showwork_after' => false,
        'showwork_cutoff' => 0, 'stugroupid' => 0, 'userfullname' => 'Student', 'useMQ' => true, 'can_viewingb' => 1,
        'session_life' => 115200, 'srready' => false,
    ] + dad_assess_dates();
}

/** One question as getQuestionObject() gives it. */
function dad_assess_question(AssessStandalone $a2, array $st, bool $withHtml): array {
    $qn = DAD_ASSESS_QN;
    $state = $a2->getState();
    $raw = $state['rawscores'][$qn] ?? [];
    $tries = $st['try'];
    $q = [
        'tries_max' => DAD_ASSESS_TRIES, 'retry_penalty' => 0, 'retry_penalty_after' => 1,
        'showans' => 'after_n', 'showans_aftern' => 1, 'points_possible' => DAD_ASSESS_POINTS, 'extracredit' => 0,
        'questionsetid' => DAD_QSID, 'category' => null, 'withdrawn' => 0, 'jump_to_answer' => false, 'showwork' => 0,
        'regens_max' => DAD_ASSESS_REGENS, 'regen' => $st['regen'], 'feedback' => '', 'try' => $tries,
        'seed' => $state['seeds'][$qn], 'singlescore' => false,
    ];
    if (!$withHtml) {
        return $q + ['gbscore' => 0, 'gbrawscore' => 0, 'status' => 'unattempted', 'score' => 0, 'rawscore' => 0, 'html' => null];
    }
    $disp = $a2->displayQuestion($qn, ['maxtries' => DAD_ASSESS_TRIES, 'showansafter' => 1]);
    $answeights = $a2->getQuestion() ? $a2->getQuestion()->getAnswerPartWeights() : [1];
    $wsum = array_sum($answeights) ?: 1;
    $score = 0;
    $parts = [];
    foreach ($answeights as $pn => $w) {
        $r = isset($raw[$pn]) ? max(0, (float)$raw[$pn]) : 0;
        $ptry = $state['partattemptn'][$qn][$pn] ?? 0;
        $pts = DAD_ASSESS_POINTS * $w / $wsum;
        $parts[] = ['try' => $ptry, 'score' => round($r * $pts, 3), 'rawscore' => $r, 'penalties' => [],
            'req_manual' => isset($raw[$pn]) && (float)$raw[$pn] == -2, 'points_possible' => round($pts, 3)];
        $score += $r * $pts;
    }
    $score = round($score, 3);
    $status = $tries === 0 ? 'unattempted'
        : ($score >= DAD_ASSESS_POINTS * 0.99 ? 'correct' : ($score > 0 ? 'partial' : 'incorrect'));
    $html = $disp['html'];   // already IMathAS's <div class="question qscope1" ...>
    return $q + [
        'gbscore' => $score, 'gbrawscore' => $score / DAD_ASSESS_POINTS, 'parts' => $parts,
        'status' => $status, 'score' => $score, 'rawscore' => $score / DAD_ASSESS_POINTS, 'info' => true,
        'html' => $html, 'jsparams' => $disp['jsparams'], 'answeights' => $answeights,
        'useda11yalt' => !empty($disp['useda11yalt']), 'usedautosave' => [], 'work' => '', 'worktime' => '0',
        'errors' => $disp['errors'] ?? [],
    ];
}

/**
 * The page: just the question part of MyOpenMath's player (its site header and the assessment's
 * title bar are hidden), IMathAS's own scripts and styles, and the bridge that sends the player's
 * calls to the D.A.D engine. The player's Start screen is passed automatically.
 */
function dad_assess_page(array $body, string $name): string {
    $static = rtrim((string)($body['static'] ?? ''), '/');
    $flags = JSON_HEX_TAG | JSON_HEX_AMP | JSON_HEX_APOS | JSON_HEX_QUOT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES;
    $dad = json_encode([
        'src' => (string)($body['src'] ?? ''), 'question' => $body['question'] ?? null, 'name' => $name,
        'engine' => (string)($body['engine'] ?? ''), 'static' => $static, 'seed' => $body['seed'] ?? null,
    ], $flags);

    $staticroot = $imasroot = $static;
    $installname = 'MyOpenMath';
    $CFG = ['GEN' => ['uselocaljs' => true]];
    $myrights = 5;
    $lastvueupdate = 'dad';
    $nologo = $noskipnavlink = $flexwidth = true;
    $_SESSION['mathdisp'] = 1;
    $_SESSION['graphdisp'] = 1;
    $useeditor = 'noinit';
    $placeinhead =
        '<script type="text/javascript">var APIbase = "dad:assess/"; var inTreeReader = 0; var exiturl = "";</script>'
      . '<link rel="stylesheet" type="text/css" href="' . $static . '/assess2/vue/css/style.css" />'
      . '<script src="' . $static . '/mathquill/mathquill.min.js" type="text/javascript"></script>'
      . '<script src="' . $static . '/javascript/assess2_min.js" type="text/javascript"></script>'
      . '<link rel="stylesheet" type="text/css" href="' . $static . '/mathquill/mathquill-basic.css">'
      . '<link rel="stylesheet" type="text/css" href="' . $static . '/mathquill/mqeditor.css">'
      . '<script type="text/javascript">'
      . 'var DAD = ' . $dad . '; var dadState = null;'
      // The player's calls go to the engine (the app's in-page engine when "bundled"); the sandbox
      // state rides along and is taken off the answer before the player sees it.
      . '$.ajaxTransport("+*", function (o) { var m = /^dad:assess\/(\w+)\.php/.exec(o.url || ""); if (!m) return; return {'
      . '  send: function (headers, done) { var post = {}, d = o.data;'
      . '    if (typeof FormData !== "undefined" && d instanceof FormData) { d.forEach(function (v, k) { if (typeof v === "string") post[k] = v; }); }'
      . '    else if (typeof d === "string") { new URLSearchParams(d).forEach(function (v, k) { post[k] = v; }); }'
      . '    else if (d) { for (var k in d) post[k] = d[k]; }'
      . '    var body = JSON.stringify({ src: DAD.src, question: DAD.question, name: DAD.name, static: DAD.static, engine: DAD.engine, seed: DAD.seed, state: dadState, call: { name: m[1], post: post } });'
      . '    var p = DAD.engine === "bundled"'
      . '      ? window.parent.dadEngine.post("/api/assess", body)'
      . '      : fetch(DAD.engine + "/api/assess", { method: "POST", headers: { "Content-Type": "application/json" }, body: body }).then(function (r) { return r.text(); });'
      . '    p.then(function (t) { var j; try { j = JSON.parse(t); } catch (e) { done(500, "bad answer", { text: t }); return; }'
      . '      if (j._dadstate) { dadState = j._dadstate; delete j._dadstate; }'
      . '      var s = JSON.stringify(j); done(200, "OK", { text: s, json: j }); }, function (e) { done(500, String(e && e.message || e)); }); },'
      . '  abort: function () {} }; });'
      // The "Written Example" help opens MyOpenMath's showsoln.php in a popup; here the engine
      // answers it and the solution shows in a dialog over the page.
      . 'function dadCall(name, post) { return new Promise(function (ok, bad) { $.ajax({ url: "dad:assess/" + name + ".php", type: "POST", data: post || {}, dataType: "json" }).done(ok).fail(bad); }); }'
      . '$(function () { var open = window.popupwindow; window.popupwindow = function (id, url, w, h, s) {'
      . '  if (!/showsoln\\.php/.test(String(url))) { return open && open.apply(this, arguments); }'
      . '  dadCall("showsoln").then(function (r) { var d = document.getElementById("dadsoln") || document.createElement("div"); d.id = "dadsoln";'
      . '    d.setAttribute("role", "dialog"); d.setAttribute("aria-label", "Written Example");'
      . '    d.innerHTML = \'<div class="dadsoln-box"><div class="dadsoln-head"><b>Written Example</b><button type="button" class="secondary" onclick="this.closest(\\\'#dadsoln\\\').remove()">Close</button></div><div class="dadsoln-body"></div></div>\';'
      . '    d.querySelector(".dadsoln-body").innerHTML = r.html || "No worked solution."; document.body.appendChild(d);'
      . '    if (window.rendermathnode) { window.rendermathnode(d); } else if (window.MathJax && MathJax.typesetPromise) { MathJax.typesetPromise([d]); } }); }; });'
      . '</script>'
      // Past the Start screen, as a student who pressed Start.
      . '<script>(function go(n) { var b = Array.prototype.find.call(document.querySelectorAll("#app button"), function (b) { return /^\\s*(Start|Continue)/.test(b.textContent); });'
      . ' if (b) { b.click(); } else if (n < 200) { setTimeout(function () { go(n + 1); }, 50); } })(0);</script>'
      . '<style>' . dad_assess_css() . '</style>';

    $curdir = __DIR__;
    foreach (['staticroot', 'imasroot', 'installname', 'CFG', 'myrights', 'lastvueupdate', 'nologo', 'noskipnavlink', 'flexwidth', 'useeditor'] as $g) {
        $GLOBALS[$g] = $$g;
    }
    ob_start();
    require __DIR__ . '/header.php';
    echo '<noscript><strong>JavaScript is needed to show this question.</strong></noscript>';
    echo '<div id="app"></div>';
    echo '<script defer="defer" type="module" src="' . $static . '/assess2/vue/js/index.js"></script>';
    echo '<script defer="defer" nomodule src="' . $static . '/assess2/vue/js/index-legacy.js"></script>';
    $placeinfooter = '<div id="ehdd" class="ehdd" style="display:none;"><span id="ehddtext"></span>'
        . '<span onclick="showeh(curehdd);" style="cursor:pointer;">' . _('[more..]') . '</span></div><div id="eh" class="eh"></div>';
    require __DIR__ . '/footer.php';
    return ob_get_clean();
}

/** Only the question part shows: no site header, no assessment title bar or question menu. */
function dad_assess_css(): string {
    return 'body { background: #fff !important; margin: 0; font-family: "Noto Sans", "Open Sans", system-ui, sans-serif; } .mainbody { box-shadow: none !important; }'
        . ' #assess-header, #skip-question-header, .breadcrumb, .headerwrapper { display: none !important; }'
        . ' #app { padding: 8px 12px 30px; } .scrollpane { margin-top: 0 !important; }'
        . ' #dadsoln { position: fixed; inset: 0; background: rgba(0,0,0,.35); display: flex; align-items: flex-start; justify-content: center; padding-top: 60px; z-index: 50; }'
        . ' .dadsoln-box { background: #fff; width: min(700px, 92vw); max-height: 80vh; overflow: auto; border-radius: 6px; box-shadow: 0 6px 24px rgba(0,0,0,.25); }'
        . ' .dadsoln-head { display: flex; justify-content: space-between; align-items: center; padding: 10px 14px; border-bottom: 1px solid #ddd; }'
        . ' .dadsoln-body { padding: 14px; }';
}
