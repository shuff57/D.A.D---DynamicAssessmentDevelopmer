<?php
/**
 * Parse a question file in the "mom" format used by steve-desktop's bank into IMathAS question data.
 *
 * Two layouts exist, both carried over unchanged from the VPS sandbox's serve.php:
 *  1. Separate sections: "// === COMMON CONTROL ... ===", "// === QUESTION TEXT ===", "// === ANSWER ===".
 *  2. The native IMathAS blob inside the COMMON CONTROL section, split internally by a
 *     "//question text" line, then a lone "///" line before the answer.
 * The question type comes from a section header "SET QUESTION TYPE TO: <type>", default multipart.
 */
function dad_parse_mom(string $src): array {
    $parts = preg_split("/^\/\/ === (.+?) ===\s*$/m", $src, -1, PREG_SPLIT_DELIM_CAPTURE);
    $sec = [];
    for ($i = 1; $i < count($parts); $i += 2) {
        $sec[trim($parts[$i])] = trim($parts[$i + 1] ?? '');
    }

    $qtype = 'multipart';
    foreach ($sec as $k => $v) {
        if (preg_match('/SET QUESTION TYPE TO:\s*(\w+)/i', $k, $m)) $qtype = $m[1];
    }

    $control = '';
    foreach ($sec as $k => $v) {
        if (stripos($k, 'COMMON CONTROL') === 0) { $control = $v; break; }
    }
    $qtext = $sec['QUESTION TEXT'] ?? '';
    $answer = $sec['ANSWER'] ?? '';

    if ($qtext === '') {
        if (preg_match('#^//\s*question text\s*$#mi', $control)) {
            $bits = preg_split('#^//\s*question text\s*$#mi', $control, 2);
            $control = rtrim($bits[0]);
            $qa = preg_split('#^///\s*$#m', $bits[1], 2);
            $qtext = trim($qa[0]);
            if (isset($qa[1]) && $answer === '') $answer = trim($qa[1]);
        } elseif (preg_match('#^///\s*$#m', $control)) {
            $qa = preg_split('#^///\s*$#m', $control, 2);
            $control = rtrim($qa[0]);
            $qtext = trim($qa[1]);
        }
    }

    // As in the VPS sandbox, the answer section runs as statements after control. IMathAS's own
    // "answer" field is evaluated separately with seed+1, which this format never relied on.
    if ($answer !== '') $control .= "\n" . $answer;

    return [
        'qtype' => $qtype,
        'control' => $control,
        'qcontrol' => '',
        'qtext' => $qtext,
        'answer' => '',
        'solution' => '',
        'solutionopts' => 0,
        'hasimg' => 0,
        'extref' => '',
        'a11yalttype' => 0,
        'a11yalt' => 0,
        'lastmoddate' => 0,
    ];
}
