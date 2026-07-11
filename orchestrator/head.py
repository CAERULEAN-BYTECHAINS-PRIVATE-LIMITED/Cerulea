#!/usr/bin/env python3
"""
Cerulea build orchestrator.

Deterministic. No language model in the control loop. State lives on disk outside
the git tree so it can never cause a merge conflict. Every failure mode has an
explicit branch. The process may be killed at any instant and resume correctly.
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
STATE = ROOT / "state"
TASKS_FILE = STATE / "tasks.yaml"
SPEND_FILE = STATE / "spend.json"
CONTROL_FILE = STATE / "control"
LOGS_DIR = STATE / "logs"
WORKTREES = ROOT / "worktrees"
FAILED_DIR = ROOT / "failed"
PLANS_DIR = REPO / "plans"
ORCH = Path(__file__).resolve().parent

BUDGET_CEILING = float(os.environ.get("CERULEA_BUDGET_CEILING", "1200"))
BUDGET_WARN = float(os.environ.get("CERULEA_BUDGET_WARN", "900"))
MAX_ATTEMPTS = int(os.environ.get("CERULEA_MAX_ATTEMPTS", "3"))
POLL_SECONDS = int(os.environ.get("CERULEA_POLL_SECONDS", "20"))
DIGEST_SECONDS = int(os.environ.get("CERULEA_DIGEST_SECONDS", "10800"))
TIMEOUT_COST_ESTIMATE = 5.0
CARGO_TARGET = os.environ.get("CARGO_TARGET_DIR", str(ROOT / "cargo-target"))
CLAUDE_BIN = os.environ.get("CLAUDE_BIN", "claude")
INTEGRATION_BRANCH = "develop"

MODEL_MAP = {
    "haiku": "claude-haiku-4-5-20251001",
    "sonnet": "claude-sonnet-5",
    "opus": "claude-opus-4-8",
}
TURN_CAP = {"haiku": 25, "sonnet": 40, "opus": 60}


class InfrastructureFault(Exception):
    """The agent never ran. Not a code failure. Halt, do not retry."""


class AgentTimeout(Exception):
    """The agent exhausted its wall clock. Real work may exist in the worktree."""


# --------------------------------------------------------------------- helpers

def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def notify(level: str, msg: str) -> None:
    script = ORCH / "notify.sh"
    if not script.exists():
        print(f"[{level}] {msg}", flush=True)
        return
    subprocess.run([str(script), level, msg], check=False)


def log(msg: str) -> None:
    print(f"{now()} {msg}", flush=True)


def git(*args, cwd=None):
    return subprocess.run(
        ["git", *args], cwd=str(cwd or REPO),
        capture_output=True, text=True, check=False,
    )


def build_env() -> dict:
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = CARGO_TARGET
    return env


# ----------------------------------------------------------------------- state

def load_tasks() -> dict:
    with open(TASKS_FILE) as f:
        return yaml.safe_load(f)


def save_tasks(data: dict) -> None:
    tmp = TASKS_FILE.with_suffix(".tmp")
    with open(tmp, "w") as f:
        yaml.safe_dump(data, f, sort_keys=False, width=100)
    tmp.replace(TASKS_FILE)


def read_spend() -> dict:
    if not SPEND_FILE.exists():
        return {"total": 0.0, "by_task": {}}
    try:
        return json.loads(SPEND_FILE.read_text())
    except json.JSONDecodeError:
        return {"total": 0.0, "by_task": {}}


def add_spend(task_id: str, usd: float) -> float:
    s = read_spend()
    s["total"] = round(s["total"] + usd, 4)
    s["by_task"][task_id] = round(s["by_task"].get(task_id, 0.0) + usd, 4)
    SPEND_FILE.parent.mkdir(parents=True, exist_ok=True)
    tmp = SPEND_FILE.with_suffix(".tmp")
    tmp.write_text(json.dumps(s, indent=2))
    tmp.replace(SPEND_FILE)
    return s["total"]


def paused() -> bool:
    return CONTROL_FILE.exists() and CONTROL_FILE.read_text().strip() == "pause"


# ------------------------------------------------------------------- scheduling

def ready_tasks(tasks: list) -> list:
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


def make_worktree(task_id: str):
    wt = WORKTREES / task_id
    branch = f"task/{task_id}"
    if wt.exists():
        git("worktree", "remove", "--force", str(wt))
        shutil.rmtree(wt, ignore_errors=True)
    git("worktree", "prune")
    git("branch", "-D", branch)
    r = git("worktree", "add", "-b", branch, str(wt), INTEGRATION_BRANCH)
    if r.returncode != 0:
        raise InfrastructureFault(f"cannot create worktree: {r.stderr.strip()}")
    return wt, branch


def build_prompt(task: dict) -> str:
    parts = [
        f"# Task {task['id']}: {task['title']}",
        "",
        task["spec"].strip(),
        "",
        "## Definition of done",
    ]
    parts += [f"- {c}" for c in task["done_when"]]
    parts += [
        "",
        "## Files you own",
        ", ".join(task.get("owns", ["scoped by the spec"])),
        "",
        "Read CLAUDE.md at the repository root first. Its rules are absolute.",
        "If you cannot complete this as specified, write the reason to BLOCKED.md",
        "and exit. Never produce partial work and call it done.",
    ]
    if task.get("last_failure"):
        parts += [
            "",
            "## The previous attempt failed",
            "```",
            str(task["last_failure"])[-3000:],
            "```",
            "Fix the underlying cause. Do not suppress or delete the check.",
        ]
    return "\n".join(parts)


def run_agent(task: dict, worktree: Path):
    """Returns (returncode, cost_usd, output). Raises on fault or timeout."""
    tier = task.get("model", "sonnet")
    cmd = [
        CLAUDE_BIN, "-p", build_prompt(task),
        "--model", MODEL_MAP[tier],
        "--max-turns", str(task.get("max_turns", TURN_CAP[tier])),
        "--allowedTools", task.get("allowed_tools", "Read,Write,Edit,Bash,Glob,Grep"),
        "--output-format", "json",
        "--dangerously-skip-permissions",
    ]
    try:
        proc = subprocess.run(
            cmd, cwd=str(worktree), capture_output=True, text=True,
            env=build_env(), timeout=task.get("timeout_seconds", 5400), check=False,
        )
    except FileNotFoundError:
        raise InfrastructureFault(f"{CLAUDE_BIN} not found on PATH")
    except subprocess.TimeoutExpired as exc:
        tail = ""
        for stream in (exc.stdout, exc.stderr):
            if stream:
                tail += stream if isinstance(stream, str) else stream.decode("utf8", "replace")
        raise AgentTimeout(f"exceeded {task.get('timeout_seconds', 5400)}s\n{tail[-1500:]}")

    combined = (proc.stdout + proc.stderr)[-6000:]

    # A real run always emits a JSON object carrying total_cost_usd. Claude Code
    # can exit zero while refusing to start, so the return code proves nothing.
    payload = None
    for line in reversed(proc.stdout.strip().splitlines()):
        try:
            candidate = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(candidate, dict) and "total_cost_usd" in candidate:
            payload = candidate
            break

    if payload is None:
        raise InfrastructureFault(
            f"agent emitted no result JSON (exit {proc.returncode}).\n{combined[:800]}"
        )
    return proc.returncode, float(payload["total_cost_usd"]), combined


def commit_agent_work(worktree: Path, task: dict) -> bool:
    """Agents are told to commit and frequently do not. Without a commit the diff
    against the integration branch is empty and every check is meaningless."""
    status = subprocess.run(
        ["git", "status", "--porcelain"], cwd=str(worktree),
        capture_output=True, text=True, check=False,
    ).stdout.strip()
    if not status:
        return False
    subprocess.run(["git", "add", "-A"], cwd=str(worktree), check=False)
    subprocess.run(
        ["git", "-c", "user.email=build@cbytechains.com",
         "-c", "user.name=Orchestrator", "commit", "-m",
         f"{task['id']}: {task['title']}"],
        cwd=str(worktree), capture_output=True, check=False,
    )
    return True


def verify(worktree: Path, domain: str, scope: str):
    script = ORCH / "verify.sh"
    p = subprocess.run(
        [str(script), str(worktree), domain, scope],
        capture_output=True, text=True, check=False,
        timeout=int(os.environ.get("CERULEA_VERIFY_TIMEOUT", "7200")),
        env=build_env(),
    )
    out = (p.stdout + p.stderr).strip()
    # The harness may log to file rather than stdout. A retry prompt with no
    # failure text tells the next agent nothing.
    logfile = worktree / ".verify.log"
    if len(out) < 200 and logfile.exists():
        try:
            out = (out + "\n" + logfile.read_text()).strip()
        except OSError:
            pass
    return p.returncode, out[-6000:]


def merge_task(branch: str, task_id: str):
    git("checkout", INTEGRATION_BRANCH)
    r = git("merge", "--no-ff", branch, "-m", f"merge {task_id}")
    if r.returncode != 0:
        git("merge", "--abort")
        return False, (r.stdout + r.stderr)[-2000:]
    return True, ""


def cleanup(worktree: Path, branch: str, task_id: str, failed: bool) -> None:
    src_log = worktree / ".verify.log"
    if src_log.exists():
        LOGS_DIR.mkdir(parents=True, exist_ok=True)
        try:
            shutil.copy(src_log, LOGS_DIR / f"{task_id}.log")
        except OSError:
            pass
    if failed and worktree.exists():
        # Keep the work. A failed task contains real code that a later attempt or
        # a human can salvage. Deleting it throws away everything it cost.
        FAILED_DIR.mkdir(parents=True, exist_ok=True)
        keep = FAILED_DIR / f"{task_id}-{int(time.time())}"
        try:
            shutil.copytree(worktree, keep, symlinks=True)
        except OSError:
            pass
    git("worktree", "remove", "--force", str(worktree))
    shutil.rmtree(worktree, ignore_errors=True)
    git("worktree", "prune")
    git("branch", "-D", branch)


# -------------------------------------------------------------------- judgement

def escalate(task: dict, data: dict) -> None:
    task["status"] = "blocked"

    # A diagnosis that fails must not spawn its own diagnosis. That recursion
    # generates an unbounded chain of Opus calls until the budget stops it.
    if task["id"].endswith("-diagnose"):
        notify("alert",
               f"Diagnosis task {task['id']} failed {MAX_ATTEMPTS} times.\n"
               f"Original task cannot proceed without you.\n"
               f"Worktree preserved under {FAILED_DIR}")
        return

    diag_id = f"{task['id']}-diagnose"
    if any(t["id"] == diag_id for t in data["tasks"]):
        return
    data["tasks"].append({
        "id": diag_id,
        "title": f"Diagnose repeated failure of {task['id']}",
        "domain": task["domain"],
        "model": "opus",
        "priority": 1,
        "status": "pending",
        "attempts": 0,
        "verify": "docs",
        "depends_on": [],
        "timeout_seconds": 3600,
        "max_turns": 40,
        "owns": ["plans/repair.yaml", "BLOCKED.md"],
        "spec": (
            f"Task {task['id']} failed {MAX_ATTEMPTS} times.\n\n"
            f"Original spec:\n{task['spec']}\n\n"
            f"Last failure:\n{str(task.get('last_failure'))[:4000]}\n\n"
            "Decide whether the specification is wrong, a prerequisite is missing, "
            "or the work is genuinely hard. Write a corrected task or the missing "
            "prerequisite to plans/repair.yaml. If it cannot be fixed, write the "
            "reason to BLOCKED.md. Do not attempt the original task yourself."
        ),
        "done_when": [
            "plans/repair.yaml contains a corrected plan, or BLOCKED.md explains why not",
        ],
    })
    notify("alert",
           f"Task {task['id']} failed {MAX_ATTEMPTS} times and is blocked.\n"
           f"Worktree preserved under {FAILED_DIR}\n"
           f"Diagnosis task queued. Reply /blocked for detail.")


def ingest_plans(data: dict) -> int:
    """Planning agents write plans/<id>.yaml, which merges into the integration
    branch like any other file. The head folds them into the graph."""
    if not PLANS_DIR.exists():
        return 0
    known = {t["id"] for t in data["tasks"]}
    added = 0
    for f in sorted(PLANS_DIR.glob("*.yaml")):
        try:
            doc = yaml.safe_load(f.read_text())
        except yaml.YAMLError as e:
            notify("alert", f"Plan file {f.name} is invalid YAML: {e}")
            continue
        if not doc:
            continue
        items = doc.get("tasks", []) if isinstance(doc, dict) else doc
        if not isinstance(items, list):
            continue
        for t in items:
            if not isinstance(t, dict) or not t.get("id") or t["id"] in known:
                continue
            if not t.get("spec") or not t.get("done_when"):
                continue
            t.setdefault("status", "pending")
            t.setdefault("attempts", 0)
            t.setdefault("depends_on", [])
            t.setdefault("priority", 50)
            t.setdefault("verify", "rust-build")
            t.setdefault("model", "sonnet")
            t.setdefault("domain", "backend")
            data["tasks"].append(t)
            known.add(t["id"])
            added += 1
    if added:
        save_tasks(data)
        notify("milestone",
               f"Task graph expanded by {added}. Total {len(data['tasks'])}.")
    return added


def digest(tasks: list, total: float) -> None:
    counts = {}
    for t in tasks:
        counts[t["status"]] = counts.get(t["status"], 0) + 1
    blocked = [t for t in tasks if t["status"] == "blocked"]
    lines = [
        "Progress: " + ", ".join(f"{v} {k}" for k, v in sorted(counts.items())),
        f"Total tasks: {len(tasks)}",
        f"Spend: {total:.2f} of {BUDGET_CEILING:.0f} USD",
    ]
    if paused():
        lines.append("State: PAUSED. Send /resume to continue.")
    if blocked:
        lines.append("Blocked: " + ", ".join(t["id"] for t in blocked[:6]))
        lines.append("ACTION NEEDED: review blocked tasks")
    else:
        lines.append("Action needed: none")
    notify("info", "\n".join(lines))


def recover_orphans() -> None:
    """A task marked running with no process behind it was orphaned by a crash.
    Left alone it deadlocks the graph, because ready_tasks skips it forever."""
    data = load_tasks()
    orphans = [t for t in data["tasks"] if t["status"] == "running"]
    if not orphans:
        return
    for t in orphans:
        t["status"] = "pending"
    save_tasks(data)
    notify("info", "Requeued after restart: " + ", ".join(t["id"] for t in orphans))


# ---------------------------------------------------------------------- the loop

def run_one(task: dict, data: dict) -> None:
    """Execute a single task to completion. Every exit path updates state."""
    task["status"] = "running"
    task["attempts"] = task.get("attempts", 0) + 1
    save_tasks(data)

    wt, branch = make_worktree(task["id"])
    notify("info", f"Started {task['id']} ({task.get('model','sonnet')}): {task['title']}")
    log(f"start {task['id']} attempt {task['attempts']}")

    failed = True
    try:
        try:
            rc, cost, out = run_agent(task, wt)
        except AgentTimeout as exc:
            add_spend(task["id"], TIMEOUT_COST_ESTIMATE)
            task["status"] = "failed"
            task["last_failure"] = f"TIMEOUT: {exc}"
            notify("alert", f"Task {task['id']} timed out on attempt {task['attempts']}.")
            if task["attempts"] >= MAX_ATTEMPTS:
                escalate(task, data)
            return

        total = add_spend(task["id"], cost)
        commit_agent_work(wt, task)
        vrc, vout = verify(wt, task["domain"], task.get("verify", "rust-build"))

        if rc != 0:
            task["status"] = "failed"
            task["last_failure"] = f"agent exited {rc}\n{out}"
        elif vrc != 0:
            task["status"] = "failed"
            task["last_failure"] = vout
        else:
            merged, err = merge_task(branch, task["id"])
            if merged:
                task["status"] = "done"
                task["last_failure"] = None
                failed = False
                notify("info", f"Done {task['id']} (cost {cost:.2f}, total {total:.2f})")
                log(f"done {task['id']} cost {cost:.2f}")
            else:
                task["status"] = "failed"
                task["last_failure"] = f"merge conflict:\n{err}"

        if task["status"] == "failed" and task["attempts"] >= MAX_ATTEMPTS:
            escalate(task, data)

    finally:
        cleanup(wt, branch, task["id"], failed=failed)
        save_tasks(data)


def main() -> int:
    for d in (WORKTREES, STATE, FAILED_DIR, LOGS_DIR):
        d.mkdir(parents=True, exist_ok=True)

    if not TASKS_FILE.exists():
        notify("alert", f"No task graph at {TASKS_FILE}. Nothing to do.")
        return 1

    recover_orphans()
    notify("milestone", f"Orchestrator started. Ceiling {BUDGET_CEILING:.0f} USD.")
    last_digest = time.time()
    warned = False

    while True:
        if paused():
            time.sleep(POLL_SECONDS)
            continue

        total = read_spend()["total"]
        if total >= BUDGET_CEILING:
            notify("alert",
                   f"HARD STOP. Spend {total:.2f} reached the {BUDGET_CEILING:.0f} USD "
                   "ceiling. All work halted.")
            return 2
        if total >= BUDGET_WARN and not warned:
            notify("alert", f"Spend at {total:.2f} of {BUDGET_CEILING:.0f} USD.")
            warned = True

        data = load_tasks()
        ingest_plans(data)
        tasks = data["tasks"]

        if all(t["status"] in ("done", "blocked") for t in tasks):
            notify("milestone", f"Build complete. Final spend {total:.2f} USD.")
            digest(tasks, total)
            return 0

        ready = ready_tasks(tasks)
        if ready:
            try:
                run_one(ready[0], data)
            except InfrastructureFault as fault:
                # Never burn attempts on a broken environment, and never let this
                # be reported as a code failure.
                t = ready[0]
                t["status"] = "pending"
                t["attempts"] = max(0, t.get("attempts", 1) - 1)
                save_tasks(data)
                notify("alert",
                       "INFRASTRUCTURE FAULT. The agent did not run.\n"
                       f"Task: {t['id']}\n{fault}\n\n"
                       "Build halted. This is a setup problem, not a code problem.")
                return 3
        else:
            time.sleep(POLL_SECONDS)

        if time.time() - last_digest > DIGEST_SECONDS:
            digest(load_tasks()["tasks"], read_spend()["total"])
            last_digest = time.time()


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as exc:  # never die silently on an unattended box
        notify("alert", f"Orchestrator crashed: {type(exc).__name__}: {exc}")
        raise
