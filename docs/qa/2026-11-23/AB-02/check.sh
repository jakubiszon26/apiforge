#!/usr/bin/env bash
# AB-02: the pass check of T6, round 4 of AB-01 unchanged (T7 kept, not used).
#
#   check.sh T6|T7 <cli> <project> <start>
#
# <cli> is the `apiforge` of the run's own version, <project> the folder the
# driver left, <start> the prepared copy the run began from (never touched).
# Every run is a simulation (`run` without `--live`). The script prints one
# line per condition and `PASS` or `FAIL` at the end; it exits 0 on PASS.
#
# T6  in *Room booking day* (`booking-day`): a new `GET /rooms/room-a/photos`
#     runs after `profile` and before `team`; `policy` and its check
#     `short_enough` run before `equipment`; `check` exits 0 and the
#     simulated run passes; no step or check of the start is changed or gone,
#     order lines apart. In language 1 the two orders must follow from the
#     file: a path of order edges (`@after`) and value reads leads from
#     `profile` to the new step, from it to `team`, and from `policy` and
#     `short_enough` to `equipment`; a tie between siblings does not count.
# T7  in the scenario `cart`: the `POST`'s body reads `quantity` 3 and
#     `productId` 124, both literals; `check` exits 0; no other step or check
#     is changed, added or gone. The start has no samples, so its simulated
#     run stops at the `POST`; the run is made only for its snapshot. Whether
#     the driver saw each value in the other view is read from its two
#     captures, not here.
set -u
task=$1 cli=$2 proj=$3 start=$4
case $task in
  T6) entity=booking-day ;;
  T7) entity=cart ;;
  *) echo "FAIL: task is T6 or T7"; exit 1 ;;
esac
scenario_path() {
  python3 -I -c 'import json,sys
m=json.load(open(sys.argv[1]))
print([d["path"] for d in m["documents"] if d.get("entityId")==sys.argv[2]][0])' "$1/apiforge.project.json" "$entity"
}
F=$(scenario_path "$proj") || { echo "FAIL: $entity is not registered"; exit 1; }
F0=$(scenario_path "$start")
lang=$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("languageVersion", 1))' "$proj/apiforge.project.json")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
"$cli" check --folder "$proj" >"$tmp/check.out" 2>&1
check_rc=$?
"$cli" run --folder "$proj" --scenario "$F" --json >"$tmp/run.json" 2>"$tmp/run.err"
cp -a "$start" "$tmp/copy"
"$cli" run --folder "$tmp/copy" --scenario "$F0" --json >"$tmp/start.json" 2>/dev/null
diff -u "$start/$F0" "$proj/$F" >"$tmp/file.diff"
python3 -I - "$task" "$check_rc" "$tmp/run.json" "$tmp/start.json" "$tmp/file.diff" "$lang" <<'PY'
import json, sys
task, check_rc, run_p, start_p, diff_p, lang = sys.argv[1:]
ok = True
def cond(name, good, detail=""):
    global ok
    ok = ok and good
    print(("ok   " if good else "FAIL ") + name + (f" — {detail}" if detail else ""))
def note(text):
    print("note " + text)

cond("check exits 0", check_rc == "0", f"exit {check_rc}")
try:
    run = json.load(open(run_p))
except Exception as e:
    cond("the simulated run printed its snapshot", False, str(e)); print("FAIL"); sys.exit(1)
start = json.load(open(start_p))
cond("the run is a simulation", run.get("mode") == "simulation", run.get("mode"))
if task == "T6":
    cond("the simulated run passed", run.get("status") == "passed", run.get("status"))
else:
    note(f"simulated run {run.get('status')} (the start has no samples)")

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

changed = [h for h in by_handle0 if h in by_handle and norm(by_handle0[h], N0) != norm(by_handle[h], N)]
added = [h for h in by_handle if h not in by_handle0]
gone = [h for h in by_handle0 if h not in by_handle]

def edges_of(r, nodes):
    s = r["snapshot"]
    return {(nodes[e["source"]]["handle"], nodes[e["target"]]["handle"], e.get("branch"))
            for part in ("graph", "cleanup") for e in s[part]["edges"]}
E, E0 = edges_of(run, N), edges_of(start, N0)
note(f"order edges added: {sorted(E - E0, key=str) or 'none'}; gone: {sorted(E0 - E, key=str) or 'none'}")

if task == "T6":
    order = [s["handle"] for s in run["steps"] if s["status"] != "skipped"]
    pos = {h: i for i, h in enumerate(order)}
    def url_of(h):
        s = next((s for s in run["steps"] if s["handle"] == h), None)
        return ((s or {}).get("request") or {}).get("url", "")
    photos = [h for h in added if by_handle[h]["kind"] == "request"
              and by_handle[h]["config"].get("method") == "GET"
              and url_of(h).split("?")[0].endswith("/rooms/room-a/photos")]
    cond("one new GET /rooms/room-a/photos", len(photos) == 1, f"{photos}; added {added}")
    ph = photos[0] if photos else None
    cond("in the run, the new GET is after profile and before team",
         bool(ph) and ph in pos and pos["profile"] < pos[ph] < pos["team"],
         f"profile {pos.get('profile')}, new {pos.get(ph)}, team {pos.get('team')}")
    cond("in the run, policy and short_enough are before equipment",
         all(h in pos for h in ("policy", "short_enough", "equipment"))
         and pos["policy"] < pos["equipment"] and pos["short_enough"] < pos["equipment"],
         f"policy {pos.get('policy')}, short_enough {pos.get('short_enough')}, equipment {pos.get('equipment')}")
    note("run order: " + " ".join(order))
    cond("no step or check of the start changed or gone (order lines apart)",
         not (changed or gone), f"changed {changed}, gone {gone}")
    others = [h for h in added if h != ph]
    note(f"steps added beyond the new GET: {others or 'none'}")
    # Does the order follow from the file? Order edges plus value reads.
    succ = {}
    for a, b, _ in E:
        succ.setdefault(a, set()).add(b)
    def refs(x, acc):
        if isinstance(x, dict):
            if x.get("kind") == "reference" and x.get("nodeId") in N:
                acc.add(N[x["nodeId"]]["handle"])
            for v in x.values():
                refs(v, acc)
        elif isinstance(x, list):
            for v in x:
                refs(v, acc)
        return acc
    for n in N.values():
        for src in refs(n["inputs"], set()):
            succ.setdefault(src, set()).add(n["handle"])
    def reaches(a, b):
        seen, todo = set(), [a]
        while todo:
            x = todo.pop()
            for y in succ.get(x, ()):
                if y == b:
                    return True
                if y not in seen:
                    seen.add(y); todo.append(y)
        return False
    pairs = [("profile", ph), (ph, "team"), ("policy", "equipment"), ("short_enough", "equipment")]
    found = {f"{a}→{b}": bool(a and b) and reaches(a, b) for a, b in pairs}
    if lang == "1":
        cond("language 1: the orders follow from the file (@after or a value read)",
             all(found.values()), repr(found))
    else:
        note(f"language 2 orders by the written order; edge paths {found}")
elif task == "T7":
    post = [h for h, n in by_handle.items() if n["kind"] == "request" and n["config"].get("method") == "POST"]
    cond("one POST", len(post) == 1, repr(post))
    fields = {}
    if post:
        body = by_handle[post[0]]["inputs"].get("body", {})
        if body.get("kind") == "object":
            fields = {k: resolve(v, N) for k, v in body.get("fields", {}).items()}
    cond("the POST's quantity reads 3", fields.get("quantity") == ("lit", "3"), repr(fields.get("quantity")))
    cond("the POST's productId reads 124", fields.get("productId") == ("lit", "124"), repr(fields.get("productId")))
    others = [h for h in changed if not post or h != post[0]]
    if post and post[0] in changed:
        a, b = norm(by_handle0[post[0]], N0), norm(by_handle[post[0]], N)
        for x in (a, b):
            x["inputs"]["body"] = None
        f0 = {k: resolve(v, N0) for k, v in by_handle0[post[0]]["inputs"]["body"]["fields"].items()}
        extra = {k for k in set(f0) | set(fields) if k not in ("quantity", "productId") and f0.get(k) != fields.get(k)}
        if a != b or extra:
            others.append(f"{post[0]} (beyond the two values)")
    cond("no other step or check changed, added or gone", not (others or added or gone),
         f"changed {others}, added {added}, gone {gone}")

lines = [l for l in open(diff_p).read().splitlines()
         if l[:1] in "+-" and not l.startswith(("+++", "---"))]
note(f"the file's diff from the start: {sum(l[0]=='-' for l in lines)} lines out, {sum(l[0]=='+' for l in lines)} in")
for l in lines:
    print("     " + l)
print("PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
PY
