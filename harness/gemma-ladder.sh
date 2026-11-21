#!/usr/bin/env bash
# Gemma 4 31B, four loop setups on the same 40 cases, judged the same way at the end:
#   A  no reference (line quoting only)      B  + reference (cheat sheet, lookups, pre-flight)
#   C  + 2 fresh restarts                    D  + judge in the loop (scores below 4 go back)
# usage: harness/gemma-ladder.sh <harness binary> [run-name]
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="$1"; NAME="${2:-gemma-ladder-$(date +%F)}"
export VERSHION_AI_KEY="$(cat ~/.cli-proxy-api/client-key)" XDG_DATA_HOME=/tmp/dad-store
export DAD_EVAL=harness/evals.json DAD_EVAL_PICK=gemma4-31b DAD_EVAL_JOBS=3
run() { local tag=$1; shift; echo "== $tag ($*)"; env "$@" DAD_EVAL_OUT="harness/runs/$NAME/$tag" "$BIN" 2>&1 | grep -E "^\[|report:" | tail -3; }
run A DAD_DOCS=0 DAD_RESTARTS=0
run B DAD_DOCS=1 DAD_RESTARTS=0
run C DAD_DOCS=1 DAD_RESTARTS=2
run D DAD_DOCS=1 DAD_RESTARTS=2 DAD_CRITIC=4
for t in A B C D; do echo "== $t"; sed -n 6,7p "harness/runs/$NAME/$t/report.md" | tail -1; done
