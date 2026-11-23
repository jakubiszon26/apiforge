#!/usr/bin/env bash
# AB-01 round 3: the pass check of T5 and T8, two questions answered from
# the window.
#
#   check.sh T5|T8 <cli> <project> <start> <answer> [<log>]
#
# <cli> is the `apiforge` of the run's own version, <project> the folder the
# driver left, <start> the prepared copy the run began from (never touched),
# <answer> a text file holding the driver's answer as it was returned, <log>
# the driver's log. Every run is a simulation (`run` without `--live`).
#
# What is checked:
#   - every file of the start outside `.apiforge/local/` and
#     `.apiforge/diagram.json` (the window's own state) is byte for byte the
#     same in <project>, and no other file was added beside them;
#   - `check` exits 0 on <project>;
#   - the answer holds the key's facts, and for T8 the log names no run.
# The key is the simulated run's snapshot of the scenario registered as
# `booking-day`, made on a scratch copy of the start:
#   T5  the failed check, the request whose response it reads, the value
#       that came back and the value expected;
#   T8  the requests sent, in the order of the run (main, the branch taken,
#       then cleanup; a component counts as one; cleanup's requests in any
#       order), and the check right after the team is read.
# The answer is searched for the words of each fact (a request by its handle
# or the last word of its address). The script prints the key beside each
# condition; the coordinator reads every answer against it, grades right,
# right in part or wrong, and notes where its grade departs from the script.
set -u
task=$1 cli=$2 proj=$3 start=$4 answer=$5 log=${6:-/dev/null}
scenario_path() {
  python3 -I -c 'import json,sys
m=json.load(open(sys.argv[1]))
print([d["path"] for d in m["documents"] if d.get("entityId")=="booking-day"][0])' "$1/apiforge.project.json"
}
F=$(scenario_path "$start") || { echo "FAIL: booking-day is not registered in the start"; exit 1; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
"$cli" check --folder "$proj" >"$tmp/check.out" 2>&1
check_rc=$?
cp -a "$start" "$tmp/copy"
"$cli" run --folder "$tmp/copy" --scenario "$F" --json >"$tmp/run.json" 2>"$tmp/run.err"
python3 -I - "$task" "$check_rc" "$tmp/run.json" "$start" "$proj" "$answer" "$log" <<'PY'
import json, os, re, sys, hashlib
task, check_rc, run_p, start, proj, answer_p, log_p = sys.argv[1:]
ok = True
def cond(name, good, detail=""):
    global ok
    ok = ok and good
    print(("ok   " if good else "FAIL ") + name + (f" — {detail}" if detail else ""))
def note(text):
    print("note " + text)

# The files: the window's state is left out, everything else is compared.
def own_state(rel):
    return rel == ".apiforge/diagram.json" or rel.startswith(".apiforge/local/")
def files(root):
    out = {}
    for d, _, names in os.walk(root):
        for n in names:
            p = os.path.join(d, n)
            rel = os.path.relpath(p, root)
            if not own_state(rel):
                out[rel] = hashlib.sha256(open(p, "rb").read()).hexdigest()
    return out
F0, F1 = files(start), files(proj)
changed = sorted(r for r in F0 if r in F1 and F0[r] != F1[r])
gone = sorted(r for r in F0 if r not in F1)
added = sorted(r for r in F1 if r not in F0)
cond("the .afs files and the manifest are byte for byte the start",
     not (changed or gone or added), f"changed {changed}, gone {gone}, added {added}")
cond("check exits 0", check_rc == "0", f"exit {check_rc}")

run = json.load(open(run_p))
cond("the key's run is a simulation", run.get("mode") == "simulation", run.get("mode"))
steps = run["steps"]
by = {s["handle"]: s for s in steps}
text = open(answer_p).read()
low = text.lower()

if task == "T5":
    failed = [s for s in steps if s["kind"] == "assert" and s["status"] == "failed"]
    cond("the key: exactly one check fails", len(failed) == 1, [s["handle"] for s in failed])
    if failed:
        f = failed[0]
        snap = {n["id"]: n for part in ("graph", "cleanup") for n in run["snapshot"][part]["nodes"]}
        reads = sorted({snap[b["nodeId"]]["handle"] for b in snap[f["nodeId"]]["inputs"].values()
                        if isinstance(b, dict) and b.get("kind") == "reference"})
        reqs = [h for h in reads if by.get(h, {}).get("kind") == "request"]
        req_after = None
        for s in steps[: steps.index(f)]:
            if s["kind"] == "request":
                req_after = s
        url = req_after["request"]["url"] if req_after else "?"
        note(f"key: check `{f['handle']}` after {req_after and req_after['request']['method']} {url}"
             f" (it reads {reqs}); came back {f.get('actual')!r}, expected {f.get('expected')!r}")
        path = url.split("/")[-1]
        cond(f"the answer names the request ({path}) or its check", bool(re.search(r"invit", low)))
        cond(f"the answer names the value that came back ({f.get('actual')})",
             bool(re.search(rf"(?<![\w.]){re.escape(str(f.get('actual')))}(?![\w.])", text)))
        cond(f"the answer names the value expected ({f.get('expected')}, the team's size)",
             bool(re.search(rf"(?<![\w.]){re.escape(str(f.get('expected')))}(?![\w.])", text)))
elif task == "T8":
    def sent(s):
        if s["kind"] == "request":
            return [s]
        if s["kind"] == "component":
            return [s]  # the sign-in counts as one
        return []
    key = [x for s in steps if s["status"] != "skipped" for x in sent(s)]
    def label(s):
        if s["kind"] == "component":
            inner = [c for c in s.get("children", []) if c["kind"] == "request"]
            r = inner[0]["request"] if inner else {"method": "?", "url": "?"}
            return f"{s['handle']} (component: {r['method']} {r['url'].split('/rooms', 1)[-1]}) [{s['phase']}]"
        r = s["request"]
        return f"{s['handle']}: {r['method']} {r['url'].split('/demo/rooms', 1)[-1]} [{s['phase']}]"
    note(f"key: {len(key)} requests in the order of the run")
    for i, s in enumerate(key, 1):
        print(f"       {i:2}. {label(s)}")
    # An aid for the grade, not the grade: each request's words searched in
    # the answer in order (handle, or a word of its address).
    def words(s):
        if s["kind"] == "component":
            return [r"sign", r"session", r"token", r"log ?in"]
        u = s["request"]["url"].split("/demo/rooms", 1)[-1].split("?")[0]
        segs = [g for g in u.split("/") if g and not re.search(r"\d|-|^qa$|^hq$", g)]
        w = [re.escape(s["handle"])]
        if segs:
            w.append(re.escape(segs[-1].rstrip("s")))
        return w
    main = [s for s in key if s["phase"] != "cleanup"]
    pos, missing = 0, []
    for s in main:
        m = None
        for w in words(s):
            mm = re.compile(w, re.I).search(text, pos)
            if mm and (m is None or mm.start() < m.start()):
                m = mm
        if m:
            pos = m.end()
        else:
            missing.append(s["handle"])
    cond("the answer lists the requests of main and the branch in order (by the words of each)",
         not missing, f"missing or out of order: {missing or 'none'}")
    tail = text[pos:]
    clean = [s for s in key if s["phase"] == "cleanup"]
    lost = [s["handle"] for s in clean if not any(re.search(w, tail, re.I) for w in words(s))]
    cond("the answer lists cleanup's requests after them, in any order", not lost, f"missing: {lost or 'none'}")
    team = by.get("team")
    after = [s for s in steps if s["kind"] == "assert" and steps.index(s) > steps.index(team)][:1]
    if after:
        note(f"key: right after `team` the check `{after[0]['handle']}` compares the team's size with 0"
             f" (actual {after[0].get('actual')!r})")
    cond("the answer says the team's size is checked to be above 0",
         bool(re.search(r"(size|team)[^.\n]*(>\s*0|above 0|above zero|greater than\W*(0|zero)|more than\W*(0|zero)|positive|at least one|non-?zero)", low)))
    runs = [l.rstrip() for l in open(log_p) if re.search(r"\bRun\b(?![- ]order)|\brun (the|a) (test|simulation)|\bran\b", l)]
    cond("the log names no run (a run makes the run void)", not runs, "" if not runs else f"{len(runs)} lines")
    for l in runs:
        print("       " + l)
else:
    cond("task is T5 or T8", False, task)

print("PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
PY
