<?php
/**
 * The sandbox: one question in MyOpenMath's own student view.
 *
 * A port of IMathAS's embedq2.php (the page MyOpenMath embeds single questions with) that takes the
 * question from the request instead of the database. The page is IMathAS's real header and footer,
 * its assess2 scripts and styles, MathQuill and MathJax 4, so answer boxes, previews, drawing tools,
 * score markers and "Try a similar question" behave as they do on MyOpenMath.
 *
 * POST /api/student {src | question, seed?, static, engine, post?}
 *   without post: {html} - the whole page. "static" is where IMathAS's scripts and styles are served
 *     from (the engine server, or the web app's engine/imathas/); "engine" is where the page sends
 *     its Submit and "Try a similar question" requests ("bundled": the app's in-page engine).
 *   with post (what the page's jQuery sends, as a map): the embedq2 JSON answers - {jwt, disp} after
 *     Submit, {state, disp} after "Try a similar question".
 *
 * State travels in a signed token, as in embedq2. The sandbox runs on the teacher's own machine
 * (or engine), so the signature only guards against a garbled page, not against a student.
 */

require_once __DIR__ . '/includes/JWT.php';

const DAD_STUDENT_QN = 0;
const DAD_STATE_SECRET = 'dad-sandbox';

function dad_student(PDO $dbh, array $qdata, array $body): void {
    $qn = DAD_STUDENT_QN;
    $post = is_array($body['post'] ?? null) ? $body['post'] : null;
    $a2 = new AssessStandalone($dbh);
    $a2->setQuestionData(DAD_QSID, $qdata);

    if ($post !== null && isset($post['state'])) {
        $state = json_decode(json_encode(JWT::decode((string)$post['state'], DAD_STATE_SECRET)), true);
    } else {
        $seed = isset($body['seed']) ? (int)$body['seed'] : rand(0, 9999);
        $state = [
            'seeds' => [$qn => $seed], 'qsid' => [$qn => DAD_QSID],
            'stuanswers' => [], 'stuanswersval' => [],
            'scorenonzero' => [($qn + 1) => -1], 'scoreiscorrect' => [($qn + 1) => -1],
            'partattemptn' => [$qn => []], 'rawscores' => [$qn => []],
            'useda11yalt' => [$qn => false], 'auth' => '',
            // embedq2's unsigned defaults: unlimited tries, answers shown after one, scored on submit.
            'jssubmit' => false, 'showhints' => 7, 'maxtries' => 0, 'showansafter' => 1,
            'showscoredonsubmit' => true, 'hidescoremarkers' => false, 'showans' => 0,
            'allowregen' => true, 'submitall' => false, 'autoseq' => 1, 'hidescoreval' => 0,
            'correctmsg' => false, 'incorrectmsg' => false, 'partcorrectmsg' => false, 'scroll' => 1,
        ];
    }

    if ($post !== null && isset($post['regen'])) {
        $state['seeds'][$qn] = rand(0, 9999);
        unset($state['stuanswers'][$qn + 1], $state['stuanswersval'][$qn + 1]);
        $state['scorenonzero'][$qn + 1] = -1;
        $state['scoreiscorrect'][$qn + 1] = -1;
        $state['partattemptn'][$qn] = [];
        $state['rawscores'][$qn] = [];
        $state['useda11yalt'][$qn] = false;
    }
    $a2->setState($state);

    if ($post !== null && isset($post['toscoreqn'])) {
        $_POST = $post;   // the answer fields, as the page posted them
        $toscore = json_decode((string)$post['toscoreqn'], true);
        $parts = [];
        foreach (($toscore[$qn] ?? []) as $pn) $parts[$pn] = true;
        $res = $a2->scoreQuestion($qn, $parts);
        $jwt = JWT::encode([
            'id' => DAD_QSID, 'score' => round(array_sum($res['scores']), 2), 'raw' => $res['raw'],
            'allans' => $res['allans'], 'errors' => $res['errors'],
            'state' => JWT::encode($a2->getState(), DAD_STATE_SECRET),
        ], '');
        $disp = $a2->displayQuestion($qn, []);
        $disp['hidescoreval'] = $state['hidescoreval'];
        dad_json(['jwt' => $jwt, 'disp' => $disp]);
    }

    $disp = $a2->displayQuestion($qn, []);
    $disp['hidescoreval'] = $state['hidescoreval'];
    if ($post !== null) {   // "Try a similar question" (and any other ajax load)
        dad_json(['state' => JWT::encode($a2->getState(), DAD_STATE_SECRET), 'disp' => $disp]);
    }

    dad_json(['seed' => $state['seeds'][$qn], 'html' => dad_student_page($a2, $disp, $body, $state)]);
}

/** The whole page, through IMathAS's own header.php and footer.php. */
function dad_student_page(AssessStandalone $a2, array $disp, array $body, array $state): string {
    $qn = DAD_STUDENT_QN;
    $static = rtrim((string)($body['static'] ?? ''), '/');
    $flags = JSON_HEX_TAG | JSON_HEX_AMP | JSON_HEX_APOS | JSON_HEX_QUOT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES;
    $dad = json_encode([
        'src' => (string)($body['src'] ?? ''), 'question' => $body['question'] ?? null,
        'engine' => (string)($body['engine'] ?? ''), 'static' => $static,
    ], $flags);

    // What header.php and footer.php read: they run inside this function, so these are locals, and
    // are mirrored into $GLOBALS for the functions they call. MathJax 4 from the bundled copy, as
    // MyOpenMath renders.
    $staticroot = $imasroot = $static;
    $installname = 'MyOpenMath';
    $CFG = ['GEN' => ['uselocaljs' => true]];
    $myrights = 5;
    $lastvueupdate = 'dad';
    $flexwidth = $nologo = $noskipnavlink = true;
    $_SESSION['mathdisp'] = 1;
    $_SESSION['graphdisp'] = 1;
    $placeinhead =
        '<link rel="stylesheet" type="text/css" href="' . $static . '/assess2/vue/css/style.css" />'
      . '<script src="' . $static . '/mathquill/mathquill.min.js" type="text/javascript"></script>'
      . '<script src="' . $static . '/javascript/assess2_min.js" type="text/javascript"></script>'
      . '<script src="' . $static . '/javascript/assess2supp.js" type="text/javascript"></script>'
      . '<link rel="stylesheet" type="text/css" href="' . $static . '/mathquill/mathquill-basic.css">'
      . '<link rel="stylesheet" type="text/css" href="' . $static . '/mathquill/mqeditor.css">'
      . '<script type="text/javascript">var frame_id = "dad-sandbox"; var qsid = ' . DAD_QSID . '; var thisqn = ' . $qn . ';'
      . 'function sendresizemsg() {}'
      // Route the page's own requests (jQuery posts to its own address) to the D.A.D engine: the
      // app's in-page engine when "bundled", else the engine server.
      . 'var DAD = ' . $dad . ';'
      . '$.ajaxPrefilter(function (o) { o.url = "dad:student"; });'
      . '$.ajaxTransport("+*", function (o) { if (o.url !== "dad:student") return; return {'
      . '  send: function (headers, done) { var post = {}, d = o.data;'
      . '    if (typeof FormData !== "undefined" && d instanceof FormData) { d.forEach(function (v, k) { if (typeof v === "string") post[k] = v; }); }'
      . '    else if (typeof d === "string") { new URLSearchParams(d).forEach(function (v, k) { post[k] = v; }); }'
      . '    else if (d) { for (var k in d) post[k] = d[k]; }'
      . '    var body = JSON.stringify({ src: DAD.src, question: DAD.question, static: DAD.static, engine: DAD.engine, post: post });'
      . '    var p = DAD.engine === "bundled"'
      . '      ? window.parent.dadEngine.post("/api/student", body)'
      . '      : fetch(DAD.engine + "/api/student", { method: "POST", headers: { "Content-Type": "application/json" }, body: body }).then(function (r) { return r.text(); });'
      . '    p.then(function (t) { done(200, "OK", { text: t }); }, function (e) { done(500, String(e && e.message || e)); }); },'
      . '  abort: function () {} }; });'
      . '</script>'
      . '<style>body { margin: 0; padding: 0 12px; background-color: #fff !important; } .question { margin-top: 0 !important; }'
      . ' .questionpane { margin: 0 !important; padding-top: .5em !important; } .questionpane > .question { background-image: none !important; }</style>';

    $curdir = __DIR__;
    foreach (['staticroot', 'imasroot', 'installname', 'CFG', 'myrights', 'lastvueupdate', 'flexwidth', 'nologo', 'noskipnavlink'] as $g) {
        $GLOBALS[$g] = $$g;
    }
    ob_start();
    require __DIR__ . '/header.php';
    echo '<div><ul id="errorslist" style="display:none" class="small"></ul></div>';
    echo '<div class="questionwrap"><div id="results' . $qn . '"></div>';
    echo '<div class="questionpane skipmathrender"><div class="question" id="questionwrap' . $qn . '"></div></div>';
    echo '<p><button type=button onclick="submitq(' . $qn . ')" class="primary">' . _('Submit') . '</button>';
    echo ' <button type=button onclick="regenq(' . $qn . ')" class="secondary">' . _('Try a similar question') . '</button></p>';
    echo '</div>';
    echo '<input type=hidden name=toscoreqn id=toscoreqn value=""/>';
    echo '<input type=hidden name=state id=state value="' . htmlspecialchars(JWT::encode($a2->getState(), DAD_STATE_SECRET)) . '" />';
    echo '<script>$(function() { showandinit(' . $qn . ',' . json_encode($disp) . '); });</script>';
    echo '<div id="embedspacer" style="display:none;height:150px">&nbsp;</div>';
    $placeinfooter = '<div id="ehdd" class="ehdd" style="display:none;"><span id="ehddtext"></span>'
        . '<span onclick="showeh(curehdd);" style="cursor:pointer;">' . _('[more..]') . '</span></div><div id="eh" class="eh"></div>';
    require __DIR__ . '/footer.php';
    return ob_get_clean();
}
