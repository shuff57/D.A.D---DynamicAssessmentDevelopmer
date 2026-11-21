#!/usr/bin/env python3
"""Build src/mom/reference.json: what a MyOpenMath (IMathAS) question may use, as structured data.

    python3 tools/build_mom_reference.py [--imathas ~/.cache/dad/IMathAS] [--engine http://127.0.0.1:8791]

Facts come from IMathAS itself, docs from docs/dad/mom-reference (MyOpenMath's help page and the
IMathAS libs, as collected in steve-desktop):

- allowed_functions: the engine's own $allowedmacros after a render (`POST /api/dialect`), i.e.
  exactly what the code checker accepts without a library.
- libraries: functions each assessment/libs/*.php adds with array_push($allowedmacros, ...), which a
  question gets with loadlibrary("name").
- disallowed_vars, answer_types: also from the engine.
- answer_type_docs: "## `name` — Title" sections of question-types/*.md.
- macro_docs: "### `signature`" sections of macros/*.md, plus the function tables of libs/*.md.
- topics: syntax, display, common options, question structure, dialect rules.
- php_traps: common PHP functions question code may not call, each with what to use instead,
  checked against allowed_functions so a suggestion is never itself forbidden.

Standard library only. The app embeds the JSON (src/mom/reference.rs).
"""
import argparse
import json
import pathlib
import re
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs/dad/mom-reference"

ap = argparse.ArgumentParser()
ap.add_argument("--imathas", type=pathlib.Path, default=pathlib.Path.home() / ".cache/dad/IMathAS")
ap.add_argument("--engine", default="http://127.0.0.1:8791")
ap.add_argument("--out", type=pathlib.Path, default=ROOT / "src/mom/reference.json")
args = ap.parse_args()

# --- facts from the engine ---
probe = "// === SET QUESTION TYPE TO: number ===\n// === COMMON CONTROL ===\n$answer = 1\n// === QUESTION TEXT ===\nx $answerbox\n"
req = urllib.request.Request(args.engine + "/api/dialect", json.dumps({"src": probe}).encode(), {"Content-Type": "application/json"})
dialect = json.loads(urllib.request.urlopen(req, timeout=60).read())
allowed = sorted(set(dialect["allowed_functions"]) | set(dialect["internal_functions"]) | {"rand", "log", "ln", "is_numeric"})

# --- library functions, from each library's array_push($allowedmacros, ...) ---
libraries = {}
for f in sorted((args.imathas / "assessment/libs").glob("*.php")):
    text = f.read_text(errors="replace")
    names = []
    for m in re.finditer(r"array_push\(\s*\$allowedmacros\s*,(.*?)\)\s*;", text, re.S):
        names += re.findall(r"['\"]([A-Za-z_]\w*)['\"]", m.group(1))
    for m in re.finditer(r"\$allowedmacros\[\]\s*=\s*['\"]([A-Za-z_]\w*)['\"]", text):
        names.append(m.group(1))
    names = sorted(set(names) - set(allowed))
    if names:
        libraries[f.stem] = names

# --- docs ---
def sections(text, level):
    """(heading, body) pairs for headings of exactly `level` #'s."""
    out, head, buf = [], None, []
    pat = re.compile(r"^(#{1,6})\s+(.*)$")
    for line in text.splitlines():
        m = pat.match(line)
        if m and len(m.group(1)) <= level:
            if head is not None:
                out.append((head, "\n".join(buf).strip()))
            head, buf = (m.group(2).strip(), []) if len(m.group(1)) == level else (None, [])
            continue
        if head is not None:
            buf.append(line)
    if head is not None:
        out.append((head, "\n".join(buf).strip()))
    return out

# Names MyOpenMath's help page (and so these docs, before they were corrected) use for answer types,
# mapped to the codes IMathAS accepts in $anstypes. A writer that copied the help page's names
# produced "multiple_choice" questions that IMathAS rejects.
ANSWER_TYPE_ALIASES = {"multiple_choice": "choices", "multiple_answer": "multans", "function": "numfunc",
    "fileupload": "file", "numericalmatrix": "matrix", "calculatedmatrix": "calcmatrix", "numerical": "number",
    "multiplechoice": "choices", "select_all": "multans", "free_response": "essay", "text": "string"}

answer_type_docs = {}
for f in sorted((DOCS / "question-types").glob("*.md")):
    for head, body in sections(f.read_text(), 2):
        m = re.match(r"`([a-z]+)`\s*[—-]\s*(.*)", head)
        if m:
            summary = next((l for l in body.splitlines() if l.strip() and not l.startswith(("#", "`", "-", "|"))), "")
            answer_type_docs[m.group(1)] = {"title": m.group(2), "summary": summary.strip(), "doc": body, "page": f.name}

macro_docs = {}
for f in sorted((DOCS / "macros").glob("*.md")):
    for head, body in sections(f.read_text(), 3):
        m = re.match(r"`([A-Za-z_]\w*)\s*(\(.*?\))?`", head)
        if m:
            summary = next((l for l in body.splitlines() if l.strip() and not l.startswith(("-", "`", "|"))), "")
            macro_docs[m.group(1)] = {"signature": head.strip("`"), "summary": summary.strip(), "doc": body, "page": f"macros/{f.name}", "library": None}
for f in sorted((DOCS / "libs").glob("*.md")):
    for m in re.finditer(r"^\|\s*`([A-Za-z_]\w*)`\s*\|\s*(.*?)\s*\|\s*$", f.read_text(), re.M):
        name, purpose = m.group(1), m.group(2)
        lib = next((l for l, fns in libraries.items() if name in fns), None)
        macro_docs.setdefault(name, {"signature": name, "summary": purpose, "doc": purpose, "page": f"libs/{f.name}", "library": lib})
for name, d in macro_docs.items():
    if d["library"] is None:
        d["library"] = next((l for l, fns in libraries.items() if name in fns), None)

topics = {}
for name in ["dialect-rules", "syntax", "display", "common-options", "question-structure"]:
    p = DOCS / f"{name}.md"
    if p.exists():
        topics[name] = p.read_text()

# --- PHP functions models reach for, and what IMathAS offers instead ---
TRAPS = {
    "shuffle": ["jointshuffle", "randfrom", "diffrandsfrom"], "sort": ["sortarray"], "rsort": ["sortarray"],
    "usort": ["sortarray"], "asort": ["sortarray"], "implode": ["join", "arraytolist"], "explode": ["listtoarray"],
    "array_slice": ["array_slice"], "array_rand": ["randfrom", "diffrandsfrom", "jointrandfrom"],
    "range": ["listtoarray", "fillarray"], "number_format": ["makeprettydisp", "prettyreal", "round"],
    "pow": ["**", "safepow"], "array_sum": ["calconarray", "sum"], "array_map": ["calconarray"],
    "str_repeat": [], "sprintf": ["prettyreal", "makeprettydisp"], "printf": [], "intdiv": ["floor"],
    "array_fill": ["fillarray"], "in_array": ["arrayfindindex"], "array_search": ["arrayfindindex"],
    "str_replace": ["str_replace"], "strlen": ["strlen"], "substr": ["substr"], "ucfirst": ["ucfirst"],
    "invNorm": ["invnormalcdf"], "invnorm": ["invnormalcdf"], "normalcdf": ["normalcdf"], "count": ["count"], "array_push": [], "array_merge": ["mergearrays"],
}
php_traps = {}
lib_index = {fn: l for l, fns in libraries.items() for fn in fns}
for fn, alts in TRAPS.items():
    if fn in allowed:
        continue
    if fn in lib_index:
        php_traps[fn] = {"use": [], "needs_library": lib_index[fn]}
        continue
    ok = [a if a == "**" or a in allowed else f'{a} (needs loadlibrary("{lib_index[a]}"))'
          for a in alts if a == "**" or a in allowed or a in lib_index]
    php_traps[fn] = {"use": ok, "needs_library": None}

out = {
    "source": "IMathAS pinned in engine/Dockerfile; docs/dad/mom-reference (MyOpenMath help page, IMathAS libs)",
    "allowed_functions": allowed,
    "libraries": libraries,
    "disallowed_vars": sorted({v.lstrip("$") for v in dialect["disallowed_vars"]}),
    "answer_types": dialect["answer_types"],
    "answer_type_aliases": ANSWER_TYPE_ALIASES,
    "answer_type_docs": answer_type_docs,
    "macro_docs": dict(sorted(macro_docs.items())),
    "topics": topics,
    "php_traps": php_traps,
}
args.out.write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n")
undocumented = [t for t in dialect["answer_types"] if t not in answer_type_docs]
print(f"{args.out}: {len(allowed)} allowed functions, {len(libraries)} libraries ({sum(map(len, libraries.values()))} functions), "
      f"{len(answer_type_docs)}/{len(dialect['answer_types'])} answer types documented, {len(macro_docs)} macros documented, "
      f"{len(php_traps)} PHP traps, {args.out.stat().st_size // 1024} KB")
if undocumented:
    print("answer types without docs:", ", ".join(undocumented))
for fn, t in php_traps.items():
    print(f"  {fn}: " + (f"loadlibrary(\"{t['needs_library']}\")" if t["needs_library"] else ("use " + ", ".join(t["use"]) if t["use"] else "no replacement")))
