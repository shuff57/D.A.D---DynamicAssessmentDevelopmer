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
    // Only the five real markers start a section. Authors also write comment banners in the same
    // shape inside the code ("// === Scenario picker ==="); those stay part of the section they are
    // in, or the code after them is silently dropped.
    $parts = preg_split("/^\/\/ === (.+?) ===\s*$/m", $src, -1, PREG_SPLIT_DELIM_CAPTURE);
    $sec = [];
    $current = null;
    for ($i = 1; $i < count($parts); $i += 2) {
        $head = trim($parts[$i]);
        $body = $parts[$i + 1] ?? '';
        if (dad_is_section_marker($head) || $current === null) {
            $current = $head;
            $sec[$current] = $body;
        } else {
            $sec[$current] .= "// === $head ===" . $body;
        }
    }
    foreach ($sec as $k => $v) $sec[$k] = trim($v);

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

    // A third layout: no ANSWER section, and a lone "///" line inside QUESTION TEXT separating the
    // question from its worked solution. Only outside multipart, where "///" means sequential steps.
    if ($answer === '' && !in_array(strtolower($qtype), ['multipart', 'conditional'], true)
        && preg_match('#^\s*///\s*$#m', $qtext)) {
        [$qtext, $answer] = array_map('trim', preg_split('#^\s*///\s*$#m', $qtext, 2));
    }

    // The ANSWER section is one of two things. Newer files put code there (`$solutionguide = '...'`,
    // per steve-desktop's learned-rules.md), which runs after control as in the VPS sandbox. Older
    // files put the worked solution there as prose, which is MyOpenMath's Detailed Solution field;
    // running prose as code is a syntax error on a question that is fine on MyOpenMath.
    $solution = '';
    if ($answer !== '') {
        if (dad_looks_like_code($answer)) $control .= "\n" . $answer;
        else $solution = $answer;
    }

    return [
        'qtype' => $qtype,
        'control' => $control,
        'qcontrol' => '',
        'qtext' => $qtext,
        'answer' => '',
        'solution' => $solution,
        'solutionopts' => 0,
        'hasimg' => 0,
        'extref' => '',
        'a11yalttype' => 0,
        'a11yalt' => 0,
        'lastmoddate' => 0,
    ];
}

/**
 * Whether an ANSWER section is question code rather than a prose solution: its first line that is
 * not blank and not a comment starts like a statement (an assignment, a control keyword, a macro).
 */
function dad_looks_like_code(string $section): bool {
    foreach (preg_split('/\R/', $section) as $line) {
        $t = trim($line);
        if ($t === '' || str_starts_with($t, '//')) continue;
        return (bool)preg_match('/^(\$|if\s*\(|for\s*\(|foreach\s*\(|while\s*\(|loadlibrary\s*\(|includecodefrom\s*\(|[A-Za-z_]\w*\s*\(.*\)\s*;?$)/', $t);
    }
    return false;
}

/** The section markers of the mom format; any other "// === X ===" line is a comment. */
function dad_is_section_marker(string $head): bool {
    $u = strtoupper(trim($head));
    return str_starts_with($u, 'NAME - DESCRIPTION:') || str_contains($u, 'SET QUESTION TYPE TO:')
        || str_starts_with($u, 'COMMON CONTROL') || $u === 'QUESTION TEXT' || $u === 'ANSWER';
}
