#!/usr/bin/env python3
"""Group a regress.py run's failures by cause into a Markdown report, and write the baseline.

    python3 engine/triage.py <mom-content> [--results /tmp/dad-regress.json]
        [--report docs/dad/bank-triage.md] [--baseline engine/baseline.json]

A failure is filed under the first rule its messages match: real question defects first, then
"needs a look" for anything no rule explains (the question, or a gap in the self-test).
"""
import argparse
import collections
import datetime
import json
import pathlib
import re

ap = argparse.ArgumentParser()
ap.add_argument("bank", type=pathlib.Path)
ap.add_argument("--results", type=pathlib.Path, default=pathlib.Path("/tmp/dad-regress.json"))
ap.add_argument("--report", type=pathlib.Path, default=pathlib.Path("docs/dad/bank-triage.md"))
ap.add_argument("--baseline", type=pathlib.Path, default=pathlib.Path("engine/baseline.json"))
args = ap.parse_args()

RULES = [
    ("Invented answer type (`multiple_choice`, `numerical`; IMathAS has `choices`, `number`)", r"Unknown answer type"),
    ("Multipart question with no `$anstypes`", r"missing \$anstypes|\$anstypes not defined"),
    ("Code syntax error", r"syntax error"),
    ("Uses a function or construct MyOpenMath forbids", r"unallowed macro|unsafe code"),
    ("Question text prints a variable the code never sets (a number, picture or table silently missing)", r"never sets"),
    ("Duplicate choice options (picking the other copy is marked wrong)", r"duplicate options"),
    ("HTML or backtick math inside a dropdown option (shows as raw text)", r"dropdown option holds"),
    ("A blank answer earns credit", r"blank answer scored"),
    ("Bad `requiretimes`", r"Invalid requiretimes"),
    ("A choices part with no `$questions`", r"\$questions is not defined"),
    ("Wrong arguments to a macro", r"expects at least"),
    ("Warning in the question code (undefined variable or array offset)", r"Caught warning in the question code"),
]
LOOK = "Needs a look: the question or the self-test (no error, an answer did not score)"

results = json.loads(args.results.read_text())
index = {q["path"]: q for q in json.loads((args.bank / "reference" / "question-index.json").read_text())}
groups = collections.defaultdict(list)
for path, r in sorted(results.items()):
    if r.get("pass"):
        continue
    runs = r.get("runs", [])
    text = " | ".join(x for run in runs for x in run["problems"] + run["render_errors"] + run["correct"]["errors"])
    seeds = [run["seed"] for run in runs if not run["ok"]]
    for title, rx in RULES:
        if re.search(rx, text):
            groups[title].append((path, seeds, ""))
            break
    else:
        first = next((x for run in runs for x in run["problems"]), r.get("transport", ""))
        groups[LOOK].append((path, seeds, first))

passed = sum(1 for r in results.values() if r.get("pass"))
untested = sum(1 for r in results.values() if r.get("pass") and r.get("untested_parts"))
defects = sum(len(v) for k, v in groups.items() if k != LOOK)
lines = [
    f"# Question bank triage ({datetime.date.today()})\n",
    f"Engine self-test of `mom-content` ({len(results):,} questions) at seeds 4242, 7 and 99.\n",
    f"- **{passed:,} pass**: every machine-gradable part scores full marks with its correct answer and 0 when blank, at every seed.",
    f"- {untested} of those also have essay, upload, drawing or picture-option parts, which a teacher has to check by eye.",
    f"- **{len(results) - passed} fail**: {defects} question defects and {len(groups.get(LOOK, []))} to look at. "
    '"In use" means the question is in at least one assignment.\n',
    "These are the repo copies of the questions; a live MyOpenMath copy edited by hand may differ.\n",
    "Re-run: `engine/run.sh && python3 engine/regress.py <mom-content> --check engine/baseline.json && python3 engine/triage.py <mom-content>`.\n",
]
for title in [t for t, _ in RULES] + [LOOK]:
    rows = groups.get(title)
    if not rows:
        continue
    lines.append(f"## {title} ({len(rows)})\n")
    for path, seeds, note in rows:
        q = index.get(path, {})
        inuse = f" — in use (qsetid {q['qsetid']})" if q.get("used_by") and q.get("qsetid") else (" — in use" if q.get("used_by") else "")
        lines.append(f"- `{path.removeprefix('questions/')}` (seeds {', '.join(map(str, seeds))}){inuse}{' — ' + note[:100] if note else ''}")
    lines.append("")
args.report.write_text("\n".join(lines))
args.baseline.write_text(json.dumps({p: bool(r.get("pass")) for p, r in sorted(results.items())}, indent=0) + "\n")
for title, rows in groups.items():
    print(f"{len(rows):4d}  {title}")
print(f"report: {args.report}  baseline: {args.baseline}")
