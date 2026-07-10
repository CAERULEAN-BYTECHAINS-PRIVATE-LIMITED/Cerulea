#!/usr/bin/env python3
"""
Cerulea build orchestrator.

The head is deterministic code, not a language model. It resolves dependencies,
dispatches work, enforces the budget, and escalates. Judgment is sampled by
invoking planning, integration, escalation, and coherence tasks as needed.

State lives in git. The process can die at any moment and resume cleanly.
"""

import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import yaml

ROOT = Path(os.environ.get("CERULEA_ROOT", "/opt/cerulea")).resolve()
REPO = ROOT / "repo"
TASKS_FILE = ROOT / "state" / "tasks.yaml"
PLANS_DIR = REPO / "plans"
FAILED_DIR = ROOT / "failed"
SPEND_FILE = ROOT / "state" / "spend.json"
CONTROL_FILE = ROOT / "state" / "control"
WORKTREES = ROOT / "worktrees"
ORCH = Path(__file__).resolve().parent

BUDGET_CEILING = float(os.environ.get("CERULEA_BUDGET_CEILING", "1200"))
BUDGET_WARN = float(os.environ.get("CERULEA_BUDGET_WARN", "900"))
MAX_PARALLEL = int(os.environ.get("CERULEA_MAX_PARALLEL", "4"))
MAX_ATTEMPTS = 3
TIMEOUT_COST_ESTIMATE = 5.0
# One shared, persistent build cache. Without this every task recompiles the
# entire dependency tree from scratch inside its throwaway worktree.
CARGO_TARGET = os.environ.get("CARGO_TARGET_DIR", str(ROOT / "cargo-target"))  # conservative, so the ceiling is never undercounted
POLL_SECONDS = 20

MODEL_MAP = {
    "haiku": "claude-haiku-4-5-20251001",
    "sonnet": "claude-sonnet-5",
    "opus": "claude-opus-4-8",
}

TURN_CAP = {"haiku": 25, "sonnet": 40, "opus": 60}


# ---------------------------------------------------------------- utilities

def now():
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def notify(level, msg):
    subprocess.run([str(ORCH / "notify.sh"), level, msg], check=False)


def git(*args, cwd=REPO):
    return subprocess.run(
        ["git", *args], cwd=cwd, capture_output=True, text=True, check=False
    )


def load_tasks():
    with open(TASKS_FILE) as f:
        return yaml.safe_load(f)


def save_tasks(data):
    tmp = TASKS_FILE.with_suffix(".tmp")
    with open(tmp, "w") as f:
        yaml.safe_dump(data, f, sort_keys=False, width=100)
    tmp.replace(TASKS_FILE)   # atomic, survives a crash mid-write


def ingest_plans(data):
    """Planning agents write plans/<id>.yaml inside their worktree, which merges
    into develop like any other file. The head folds them into the task graph.
    This keeps state out of the branches agents work on."""
    if not PLANS_DIR.exists():
        return False
    known = {t["id"] for t in data["tasks"]}
    added = 0
    for f in sorted(PLANS_DIR.glob("*.yaml")):
        try:
            new = yaml.safe_load(f.read_text()) or []
        except yaml.YAMLError as e:
            notify("alert", f"Plan file {f.name} is invalid YAML: {e}")
            continue
        for t in (new.get("tasks", new) if isinstance(new, dict) else new):
            if t.get("id") and t["id"] not in known:
                t.setdefault("status", "pending")
                t.setdefault("attempts", 0)
                t.setdefault("depends_on", [])
                t.setdefault("priority", 50)
                t.setdefault("verify", "rust-build")
                t.setdefault("model", "sonnet")
                data["tasks"].append(t)
                known.add(t["id"])
                added += 1
    if added:
        save_tasks(data)
        notify("milestone", f"Task graph expanded by {added} tasks. Total {len(data['tasks'])}.")
    return added > 0


# ---------------------------------------------------------------- budget

def read_spend():
    if not SPEND_FILE.exists():
        return {"total": 0.0, "by_task": {}}
    with open(SPEND_FILE) as f:
        return json.load(f)


def add_spend(task_id, usd):
    s = read_spend()
    s["total"] = round(s["total"] + usd, 4)
    s["by_task"][task_id] = round(s["by_task"].get(task_id, 0.0) + usd, 4)
    SPEND_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(SPEND_FILE, "w") as f:
        json.dump(s, f, indent=2)
    return s["total"]


def budget_check():
    """Returns (allowed, total). Hard stop at ceiling."""
    total = read_spend()["total"]
    if total >= BUDGET_CEILING:
        return False, total
    return True, total


# ---------------------------------------------------------------- control

def paused():
    if not CONTROL_FILE.exists():
        return False
    return CONTROL_FILE.read_text().strip() == "pause"


# ---------------------------------------------------------------- dispatch

def ready_tasks(tasks):
    done = {t["id"] for t in tasks if t["status"] == "done"}
    out = []
    for t in tasks:
        if t["status"] not in ("pending", "failed"):
            continue
        if t.get("attempts", 0) >= MAX_ATTEMPTS:
            continue
        if all(d in done for d in t.get("depends_on", [])):
            out.append(t)
    out.sort(key=lambda t: (t.get("priority", 50), t["id"]))
    return out


def make_worktree(task_id):
    wt = WORKTREES / task_id
    if wt.exists():
        shutil.rmtree(wt, ignore_errors=True)
        git("worktree", "prune")
    branch = f"task/{task_id}"
    git("branch", "-D", branch)
    git("worktree", "add", "-b", branch, str(wt), "develop")
    return wt, branch


def build_prompt(task):
    parts = [
        f"# Task {task['id']}: {task['title']}",
        "",
        task["spec"].strip(),
        "",
        "## Definition of done",
    ]
    for c in task["done_when"]:
        parts.append(f"- {c}")
    parts += [
        "",
        "## Files you own",
        ", ".join(task.get("owns", ["(scoped by spec)"])),
        "",
        "Read CLAUDE.md at the repository root first. Its rules are absolute.",
        "If you cannot complete this as specified, write your blocker to",
        "BLOCKED.md and exit rather than producing partial or fake work.",
        "Commit your work. Do not merge. Do not push to main.",
    ]
    if task.get("last_failure"):
        parts += [
            "",
            "## Previous attempt failed with",
            "```",
            task["last_failure"][-3000:],
            "```",
            "Fix the underlying cause. Do not suppress the check.",
        ]
    return "\n".join(parts)


class AgentTimeout(Exception):
    """The agent ran out of wall clock. Real work may have happened."""


class InfrastructureFault(Exception):
    """The agent could not run at all. Not a code failure. Do not retry blindly."""


def run_agent(task, worktree):
    """Invoke Claude Code headless. Returns (exit_code, cost_usd, output).

    Raises InfrastructureFault when the agent never actually ran. Claude Code can
    exit 0 while refusing to start, for example when invoked as root with
    --dangerously-skip-permissions. Treating that as a code failure sends the
    orchestrator chasing phantom test errors.
    """
    tier = task.get("model", "sonnet")
    model = MODEL_MAP[tier]
    tools = task.get("allowed_tools", "Read,Write,Edit,Bash,Glob,Grep")

    cmd = [
        "claude", "-p", build_prompt(task),
        "--model", model,
        "--max-turns", str(task.get("max_turns", TURN_CAP[tier])),
        "--allowedTools", tools,
        "--output-format", "json",
        "--dangerously-skip-permissions",
    ]
    try:
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = CARGO_TARGET
        proc = subprocess.run(
            cmd, cwd=worktree, capture_output=True, text=True, env=env,
            timeout=task.get("timeout_seconds", 5400), check=False
        )
    except subprocess.TimeoutExpired as exc:
        partial = ""
        for stream in (exc.stdout, exc.stderr):
            if stream:
                partial += stream if isinstance(stream, str) else stream.decode("utf8", "replace")
        # The agent ran and spent money. We cannot read the cost, so estimate high
        # rather than silently undercount against the ceiling.
        raise AgentTimeout(
            f"exceeded {task.get('timeout_seconds', 5400)}s. Tail:\n{partial[-1500:]}"
        )

    combined = (proc.stdout + proc.stderr)[-6000:]

    # A real run always emits JSON containing total_cost_usd on the last line.
    payload = None
    for line in reversed(proc.stdout.strip().splitlines()):
        try:
            candidate = json.loads(line)
            if "total_cost_usd" in candidate:
                payload = candidate
                break
        except json.JSONDecodeError:
            continue

    if payload is None:
        raise InfrastructureFault(
            f"agent produced no result JSON (exit {proc.returncode}). "
            f"Output: {combined[:800]}"
        )

    return proc.returncode, float(payload["total_cost_usd"]), combined


def verify(worktree, domain, scope="full"):
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = CARGO_TARGET
    p = subprocess.run(
        [str(ORCH / "verify.sh"), str(worktree), domain, scope],
        capture_output=True, text=True, check=False, timeout=7200, env=env
    )
    return p.returncode, (p.stdout + p.stderr)[-6000:]


def merge_task(branch, task_id):
    git("checkout", "develop")
    r = git("merge", "--no-ff", branch, "-m", f"merge {task_id}")
    if r.returncode != 0:
        git("merge", "--abort")
        return False, r.stdout + r.stderr
    return True, ""


def cleanup(worktree, branch, task_id=None, failed=False):
    log = Path(worktree) / ".verify.log"
    if task_id and log.exists():
        dest = ROOT / "state" / "logs"
        dest.mkdir(parents=True, exist_ok=True)
        try:
            shutil.copy(log, dest / f"{task_id}.log")
        except OSError:
            pass

    if failed:
        # Keep the work. A failed task has real code in it that a later attempt,
        # or a human, can salvage. Destroying it throws the money away.
        FAILED_DIR.mkdir(parents=True, exist_ok=True)
        keep = FAILED_DIR / f"{task_id}-{int(time.time())}"
        try:
            shutil.copytree(worktree, keep, symlinks=True)
        except OSError:
            pass

    shutil.rmtree(worktree, ignore_errors=True)
    git("worktree", "prune")
    git("branch", "-D", branch)   # branch content is preserved in FAILED_DIR


# ---------------------------------------------------------------- judgment hooks

def escalate(task, tasks_data):
    """Task failed MAX_ATTEMPTS times. Spawn an Opus diagnosis task."""
    notify(
        "alert",
        f"Task {task['id']} ({task['title']}) failed {MAX_ATTEMPTS} times.\n"
        f"Domain: {task['domain']}\n"
        f"Last failure head:\n{(task.get('last_failure') or '')[:600]}\n\n"
        "Escalating to diagnosis. Reply /status for full state."
    )
    diag = {
        "id": f"{task['id']}-diagnose",
        "title": f"Diagnose repeated failure of {task['id']}",
        "domain": task["domain"],
        "model": "opus",
        "priority": 1,
        "status": "pending",
        "attempts": 0,
        "depends_on": [],
        "spec": (
            f"Task {task['id']} failed {MAX_ATTEMPTS} times.\n\n"
            f"Original spec:\n{task['spec']}\n\n"
            f"Last failure output:\n{(task.get('last_failure') or '')[:4000]}\n\n"
            "Determine whether the specification is wrong, a dependency is "
            "missing, or the work is genuinely hard. Then either rewrite the "
            "task spec in tasks.yaml, insert the missing prerequisite task, or "
            "Write the corrected task or the missing prerequisite to "
            "plans/repair.yaml. If it cannot be fixed, write the reason to "
            "BLOCKED.md. Do not attempt the original task yourself."
        ),
        "verify": "docs",
        "done_when": ["plans/repair.yaml written with a corrected plan, or BLOCKED.md explains why not"],
        "owns": ["plans/repair.yaml", "BLOCKED.md"],
    }
    tasks_data["tasks"].append(diag)
    task["status"] = "blocked"


def digest(tasks, total):
    counts = {}
    for t in tasks:
        counts[t["status"]] = counts.get(t["status"], 0) + 1
    blocked = [t for t in tasks if t["status"] == "blocked"]
    assert sum(counts.values()) == len(tasks)

    lines = [
        "Progress: " + ", ".join(f"{v} {k}" for k, v in sorted(counts.items())),
        f"Total tasks: {len(tasks)}",
        f"Spend: {total:.2f} of {BUDGET_CEILING:.0f} USD",
    ]
    if blocked:
        lines.append("Blocked: " + ", ".join(t["id"] for t in blocked[:6]))
        lines.append("ACTION NEEDED: review blocked tasks")
    else:
        lines.append("Action needed: none")
    notify("info", "\n".join(lines))


# ---------------------------------------------------------------- main loop

def recover_orphans():
    """A task marked running with no process behind it is orphaned by a crash.
    Left alone it deadlocks the graph forever, because ready_tasks skips it."""
    data = load_tasks()
    orphans = [t for t in data["tasks"] if t["status"] == "running"]
    if not orphans:
        return
    for t in orphans:
        t["status"] = "pending"
    save_tasks(data)
    notify("info", "Requeued after restart: " + ", ".join(t["id"] for t in orphans))


def main():
    WORKTREES.mkdir(parents=True, exist_ok=True)
    (ROOT / "state").mkdir(parents=True, exist_ok=True)
    recover_orphans()

    notify("milestone", f"Orchestrator started. Ceiling {BUDGET_CEILING:.0f} USD.")
    last_digest = 0.0
    running = {}

    while True:
        if paused():
            time.sleep(POLL_SECONDS)
            continue

        allowed, total = budget_check()
        if not allowed:
            notify(
                "alert",
                f"HARD STOP. Spend reached {total:.2f} USD of {BUDGET_CEILING:.0f}.\n"
                "All work halted. Reply to release reserve or raise the ceiling."
            )
            return 2

        if total >= BUDGET_WARN and time.time() - last_digest > 3600:
            notify("alert", f"Spend at {total:.2f} of {BUDGET_CEILING:.0f} USD.")

        data = load_tasks()
        ingest_plans(data)
        tasks = data["tasks"]

        if all(t["status"] in ("done", "blocked") for t in tasks):
            notify("milestone", f"Build complete. Final spend {total:.2f} USD.")
            digest(tasks, total)
            return 0

        for t in ready_tasks(tasks):
            if len(running) >= MAX_PARALLEL:
                break
            # One agent per domain at a time. File ownership is per domain.
            if any(r["domain"] == t["domain"] for r in running.values()):
                continue

            t["status"] = "running"
            t["attempts"] = t.get("attempts", 0) + 1
            save_tasks(data)

            wt, branch = make_worktree(t["id"])
            running[t["id"]] = {"task": t, "wt": wt, "branch": branch, "domain": t["domain"]}
            notify("info", f"Started {t['id']} ({t['model']}): {t['title']}")

            try:
                rc, cost, out = run_agent(t, wt)
            except AgentTimeout as to:
                add_spend(t["id"], TIMEOUT_COST_ESTIMATE)
                t["status"] = "failed"
                t["last_failure"] = f"TIMEOUT: {to}"
                if t["attempts"] >= MAX_ATTEMPTS:
                    escalate(t, data)
                notify("alert", f"Task {t['id']} timed out. Attempt {t['attempts']} of {MAX_ATTEMPTS}.")
                cleanup(wt, branch, t["id"])
                del running[t["id"]]
                save_tasks(data)
                continue
            except InfrastructureFault as fault:
                # Never burn an attempt on a broken environment, and never let
                # this masquerade as a failing test.
                t["status"] = "pending"
                t["attempts"] = max(0, t["attempts"] - 1)
                save_tasks(data)
                cleanup(wt, branch, t["id"])
                del running[t["id"]]
                notify(
                    "alert",
                    "INFRASTRUCTURE FAULT. The agent did not run.\n"
                    f"Task: {t['id']}\n{fault}\n\n"
                    "Build halted. No money spent. This is a setup problem, "
                    "not a code problem."
                )
                return 3

            new_total = add_spend(t["id"], cost)

                    # Agents are told to commit and frequently do not. Without a commit
            # the diff against develop is empty and every check is meaningless.
            dirty = subprocess.run(["git", "status", "--porcelain"], cwd=wt,
                                   capture_output=True, text=True).stdout.strip()
            if dirty:
                subprocess.run(["git", "add", "-A"], cwd=wt, check=False)
                subprocess.run(
                    ["git", "-c", "user.email=build@cbytechains.com",
                     "-c", "user.name=Orchestrator", "commit", "-m",
                     f"{t['id']}: {t['title']}"], cwd=wt, check=False)

            if rc == 0 and vrc == 0:
                ok, merr = merge_task(branch, t["id"])
                if ok:
                    t["status"] = "done"
                    t["last_failure"] = None
                    notify("info", f"Done {t['id']} (cost {cost:.2f}, total {new_total:.2f})")
                else:
                    t["status"] = "failed"
                    t["last_failure"] = f"merge conflict:\n{merr}"
            else:
                t["status"] = "failed"
                t["last_failure"] = vout if vrc != 0 else out

            if t["status"] == "failed" and t["attempts"] >= MAX_ATTEMPTS:
                escalate(t, data)

            cleanup(wt, branch, t["id"], failed=(t["status"] != "done"))
            del running[t["id"]]
            save_tasks(data)

        if time.time() - last_digest > 10800:  # every three hours
            digest(tasks, read_spend()["total"])
            last_digest = time.time()

        time.sleep(POLL_SECONDS)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as e:  # never die silently on an unattended box
        notify("alert", f"Orchestrator crashed: {type(e).__name__}: {e}")
        raise
