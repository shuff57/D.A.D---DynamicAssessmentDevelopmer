#!/usr/bin/env python3
"""Self-test every question in a mom-content bank against the D.A.D engine.

    python3 engine/regress.py ~/Documents/GitHub/steve-desktop/steve-desktop/mom-content
    python3 engine/regress.py <bank> --only annuity --seeds 4242,7,99 --out /tmp/regress.json

For each question, the engine renders it at each seed, submits the question's own correct
answers the way a student would (expects full marks) and a blank attempt (expects 0).
Standard library only.
"""
import argparse
import collections
import concurrent.futures as cf
import json
import pathlib
import re
import sys
import urllib.request

ap = argparse.ArgumentParser()
ap.add_argument("bank", type=pathlib.Path, help="mom-content folder (holds reference/question-index.json)")
ap.add_argument("--engine", default="http://127.0.0.1:8791")
ap.add_argument("--seeds", default="4242,7,99")
ap.add_argument("--only", default="", help="substring filter on the question path")
ap.add_argument("--jobs", type=int, default=4)
ap.add_argument("--out", type=pathlib.Path, default=pathlib.Path("/tmp/dad-regress.json"))
args = ap.parse_args()

index = json.loads((args.bank / "reference" / "question-index.json").read_text())
paths = [q["path"] for q in index if args.only in q["path"]]
seeds = [int(s) for s in args.seeds.split(",")]


def selftest(rel):
    src = (args.bank / rel).read_text(encoding="utf-8", errors="replace")
    body = json.dumps({"src": src, "seeds": seeds}).encode()
    req = urllib.request.Request(args.engine + "/api/selftest", body, {"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return rel, json.loads(r.read())
    except Exception as e:  # a crash or timeout is a result too
        return rel, {"pass": False, "transport": str(e), "runs": []}


def categories(res):
    """Short, groupable reasons, with question-specific text (numbers, option text) removed."""
    if "transport" in res:
        return {"transport: " + re.sub(r"\d+", "N", res["transport"])[:80]}
    out = set()
    for run in res["runs"]:
        for p in run["problems"]:
            p = re.sub(r"part \d+", "part N", p)
            p = re.sub(r"scored [\d.]+", "scored <1", p) if "correct answers" in p else p
            out.add(p)
        for e in run["correct"]["errors"] + run["render_errors"]:
            out.add("error: " + re.sub(r"\"[^\"]*\"|'[^']*'|\d+", "…", e)[:110])
    return out


results = {}
with cf.ThreadPoolExecutor(args.jobs) as pool:
    for i, (rel, res) in enumerate(pool.map(selftest, paths), 1):
        results[rel] = res
        if i % 100 == 0:
            print(f"  {i}/{len(paths)}", file=sys.stderr)

args.out.write_text(json.dumps(results, indent=1))
passed = [p for p, r in results.items() if r.get("pass")]
print(f"\n{len(passed)} of {len(paths)} questions pass the self-test at seeds {seeds}")
counts = collections.Counter(c for r in results.values() if not r.get("pass") for c in categories(r))
print("\nWhy the rest fail (a question can count under several reasons):")
for reason, n in counts.most_common(25):
    print(f"  {n:5d}  {reason}")
print(f"\nfull results: {args.out}")
