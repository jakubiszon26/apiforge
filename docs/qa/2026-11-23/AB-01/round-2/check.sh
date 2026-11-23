#!/usr/bin/env bash
# AB-01 round 2: the pass check of T3 and T4.
#
#   check.sh T3|T4 <cli> <project> <start>
#
# <cli> is the `apiforge` of the run's own version, <project> the folder the
# driver left, <start> the prepared copy the run began from (never touched).
# Every run is a simulation (`run` without `--live`). The script prints one
# line per condition and `PASS` or `FAIL` at the end; it exits 0 on PASS.
set -u
task=$1 cli=$2 proj=$3 start=$4
scenario_path() {
  python3 -I -c 'import json,sys
m=json.load(open(sys.argv[1]))
print([d["path"] for d in m["documents"] if d.get("entityId")=="double-booking"][0])' "$1/apiforge.project.json"
}
F=$(scenario_path "$proj") || { echo "FAIL: double-booking is not registered"; exit 1; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
"$cli" check --folder "$proj" >"$tmp/check.out" 2>&1
check_rc=$?
"$cli" run --folder "$proj" --scenario "$F" --json >"$tmp/run.json" 2>"$tmp/run.err"
"$cli" run --folder "$start" --scenario "$(scenario_path "$start")" --json >"$tmp/start.json" 2>/dev/null
diff -u "$start/$(scenario_path "$start")" "$proj/$F" >"$tmp/file.diff"
python3 -I - "$task" "$check_rc" "$tmp/run.json" "$tmp/start.json" "$tmp/file.diff" <<'PY'
import json, sys
task, check_rc, run_p, start_p, diff_p = sys.argv[1:]
ok = True
def cond(name, good, detail=""):
    global ok
    ok = ok and good
    print(("ok   " if good else "FAIL ") + name + (f" — {detail}" if detail else ""))

cond("check exits 0", check_rc == "0", f"exit {check_rc}")
try:
    run = json.load(open(run_p))
except Exception as e:
    cond("the simulated run printed its snapshot", False, str(e)); print("FAIL"); sys.exit(1)
start = json.load(open(start_p))
cond("the run is a simulation", run.get("mode") == "simulation", run.get("mode"))
cond("the simulated run passed", run.get("status") == "passed", run.get("status"))

def nodes_of(r):
    s = r["snapshot"]
    return {n["id"]: n for part in ("graph", "cleanup") for n in s[part]["nodes"]}
N, N0 = nodes_of(run), nodes_of(start)
by_handle = {n["handle"]: n for n in N.values()}
by_handle0 = {n["handle"]: n for n in N0.values()}

def resolve(b, nodes, depth=0):
    """A binding with value steps followed, node ids replaced by handles."""
    if not isinstance(b, dict) or depth > 8:
        return b
    k = b.get("kind")
    if k == "reference":
        n = nodes.get(b["nodeId"])
        if n is None:
            return ("ref", b["nodeId"], b.get("port"), b.get("path", ""))
        if n["kind"] == "value" and b.get("port") == "value" and "value" in n["inputs"]:
            inner = resolve(n["inputs"]["value"], nodes, depth + 1)
            p = b.get("path", "")
            if not p:
                return inner
            if isinstance(inner, tuple) and inner[0] == "ref":
                base = inner[3]
                return ("ref", inner[1], inner[2], f"{base}.{p}" if base else p)
            return ("path", inner, p)
        return ("ref", n["handle"], b.get("port"), b.get("path", ""))
    if k == "literal":
        return ("lit", json.dumps(b.get("value"), sort_keys=True))
    out = {}
    for key, v in b.items():
        if isinstance(v, dict):
            out[key] = resolve(v, nodes, depth + 1) if "kind" in v else {kk: resolve(vv, nodes, depth + 1) for kk, vv in v.items()}
        elif isinstance(v, list):
            out[key] = [resolve(x, nodes, depth + 1) for x in v]
        else:
            out[key] = v
    return ("x", json.dumps(out, sort_keys=True, default=str))

def norm(n, nodes):
    # A setting left out reads as its default; the v0.1.0 window writes
    # `@config` without `bodyFormat` on save, which is the same JSON body.
    config = {k: v for k, v in n.get("config", {}).items()
              if not (k == "bodyFormat" and v == "json")}
    return {"kind": n["kind"],
            "inputs": {k: resolve(v, nodes) for k, v in n["inputs"].items()},
            "config": config}

def step(handle):
    n = by_handle.get(handle)
    if n is None:
        return None
    return next((s for s in run["steps"] if s["nodeId"] == n["id"]), None)

def changed_nodes():
    """Handles whose step differs from the start, and handles added or gone."""
    changed = [h for h in by_handle0 if h in by_handle and norm(by_handle0[h], N0) != norm(by_handle[h], N)]
    added = [h for h in by_handle if h not in by_handle0]
    gone = [h for h in by_handle0 if h not in by_handle]
    return changed, added, gone

changed, added, gone = changed_nodes()

def edges_of(r, nodes):
    s = r["snapshot"]
    return {(nodes[e["source"]]["handle"], nodes[e["target"]]["handle"], e.get("branch"))
            for part in ("graph", "cleanup") for e in s[part]["edges"]}
E, E0 = edges_of(run, N), edges_of(start, N0)
print(f"note order edges added: {sorted(E - E0, key=str) or 'none'}; gone: {sorted(E0 - E, key=str) or 'none'}")

if task == "T3":
    second = by_handle.get("second")
    room = None
    if second is not None:
        body = second["inputs"].get("body", {})
        if body.get("kind") == "object" and "roomId" in body.get("fields", {}):
            room = resolve(body["fields"]["roomId"], N)
    meant = {("ref", "rooms", "body", "items[0].id"), ("ref", "first", "body", "roomId")}
    cond("second's roomId reads the room the first booking asks for, as no literal",
         room in meant or room == resolve(by_handle["first"]["inputs"]["body"]["fields"]["roomId"], N),
         repr(room))
    sent = step("second")
    first_sent = step("first")
    cond("the simulated second booking sends the first booking's room",
         bool(sent and first_sent) and sent["inputs"].get("body", {}).get("roomId") == first_sent["inputs"].get("body", {}).get("roomId"),
         repr(sent and sent["inputs"].get("body", {}).get("roomId")))
    others = [h for h in changed if h != "second"]
    if "second" in changed:
        a, b = norm(by_handle0["second"], N0), norm(second, N)
        a["inputs"]["body"] = b["inputs"]["body"] = None
        if a != b:
            others.append("second (beyond roomId)")
    cond("the same steps and checks as at the start", not (others or added or gone),
         f"changed {others}, added {added}, gone {gone}")
elif task == "T4":
    def is_code_check(n):
        if n["kind"] != "assert" or n["config"].get("predicate") != "equals":
            return False
        sides = {resolve(n["inputs"].get("actual"), N), resolve(n["inputs"].get("expected"), N)}
        return sides == {("ref", "second", "body", "error.code"), ("lit", '"slot_conflict"')}
    def is_type_check(n):
        if n["kind"] != "assert":
            return False
        a, e = n["inputs"].get("actual"), n["inputs"].get("expected")
        pred = n["config"].get("predicate")
        idref = ("ref", "first", "body", "id")
        if pred == "type":
            return resolve(a, N) == idref and resolve(e, N) == ("lit", '"string"')
        if pred == "equals":
            for x, y in ((a, e), (e, a)):
                if resolve(y, N) == ("lit", '"string"') and isinstance(x, dict) and x.get("kind") == "expression" \
                        and x.get("operator") == "typeOf" and resolve(x["inputs"].get("left"), N) == idref:
                    return True
        return False
    for name, test in (("a check compares second.body.error.code with \"slot_conflict\"", is_code_check),
                       ("a check that first.body.id is a string", is_type_check)):
        hits = [n["handle"] for n in N.values() if test(n)]
        ran = [h for h in hits if (step(h) or {}).get("status") == "passed"]
        cond(name + ", run and passed", bool(ran), f"found {hits}, passed {ran}")
    starts = ["refused", "one_booking", "not_booked", "gone"]
    moved = [h for h in starts if h not in by_handle or norm(by_handle0[h], N0) != norm(by_handle[h], N)]
    cond("the four checks of the start are unchanged", not moved, f"changed or gone: {moved}")
    other = [h for h in changed if h not in starts]
    print(f"note steps changed beyond the task: {other or 'none'}; gone: {gone or 'none'}; added: {added}")
else:
    cond("task is T3 or T4", False, task)

lines = [l for l in open(diff_p).read().splitlines()
         if l[:1] in "+-" and not l.startswith(("+++", "---"))]
print(f"note the file's diff from the start: {sum(l[0]=='-' for l in lines)} lines out, {sum(l[0]=='+' for l in lines)} in")
for l in lines:
    print("     " + l)
print("PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
PY
