#!/usr/bin/env python3
"""
Telegram command poller. Lets the founder control the build from a phone.

Commands: /status /pause /resume /budget /blocked
Runs as a separate systemd service from the head so a crash here never stops
the build, and a crash there still leaves the phone able to see state.
"""

import json
import os
import subprocess
import time
from pathlib import Path

import requests
import yaml

ROOT = Path(os.environ.get("CERULEA_ROOT", "/opt/cerulea")).resolve()
TASKS_FILE = ROOT / "repo" / "tasks.yaml"
SPEND_FILE = ROOT / "state" / "spend.json"
CONTROL_FILE = ROOT / "state" / "control"
OFFSET_FILE = ROOT / "state" / "tg_offset"

TOKEN = os.environ["TELEGRAM_TOKEN"]
CHAT_ID = str(os.environ["TELEGRAM_CHAT_ID"])
API = f"https://api.telegram.org/bot{TOKEN}"
CEILING = float(os.environ.get("CERULEA_BUDGET_CEILING", "1200"))


def send(text):
    requests.post(f"{API}/sendMessage",
                  data={"chat_id": CHAT_ID, "text": text,
                        "disable_web_page_preview": "true"}, timeout=20)


def spend():
    if not SPEND_FILE.exists():
        return 0.0
    return json.loads(SPEND_FILE.read_text()).get("total", 0.0)


def tasks():
    if not TASKS_FILE.exists():
        return []
    return yaml.safe_load(TASKS_FILE.read_text()).get("tasks", [])


def status():
    ts = tasks()
    if not ts:
        return "No task graph yet."
    counts = {}
    for t in ts:
        counts[t["status"]] = counts.get(t["status"], 0) + 1
    running = [t["id"] for t in ts if t["status"] == "running"]
    pct = int(100 * counts.get("done", 0) / len(ts))
    lines = [
        f"Cerulea build: {pct} percent",
        ", ".join(f"{k} {v}" for k, v in sorted(counts.items())),
        f"Spend {spend():.2f} of {CEILING:.0f} USD",
    ]
    if running:
        lines.append("Running: " + ", ".join(running))
    if CONTROL_FILE.exists() and CONTROL_FILE.read_text().strip() == "pause":
        lines.append("State: PAUSED")
    return "\n".join(lines)


def blocked():
    b = [t for t in tasks() if t["status"] == "blocked"]
    if not b:
        return "Nothing blocked."
    out = []
    for t in b[:8]:
        out.append(f"{t['id']}: {t['title']}")
        if t.get("last_failure"):
            out.append("  " + t["last_failure"][:200].replace("\n", " "))
    return "\n".join(out)


HANDLERS = {
    "/status": status,
    "/blocked": blocked,
    "/budget": lambda: f"Spend {spend():.2f} of {CEILING:.0f} USD",
    "/pause": lambda: (CONTROL_FILE.write_text("pause"), "Paused. Current task finishes, then work stops.")[1],
    "/resume": lambda: (CONTROL_FILE.write_text("run"), "Resumed.")[1],
}


def main():
    CONTROL_FILE.parent.mkdir(parents=True, exist_ok=True)
    offset = int(OFFSET_FILE.read_text()) if OFFSET_FILE.exists() else 0
    while True:
        try:
            r = requests.get(f"{API}/getUpdates",
                             params={"offset": offset + 1, "timeout": 50}, timeout=60)
            for upd in r.json().get("result", []):
                offset = upd["update_id"]
                OFFSET_FILE.write_text(str(offset))
                msg = upd.get("message", {})
                if str(msg.get("chat", {}).get("id")) != CHAT_ID:
                    continue  # ignore anyone who is not the founder
                cmd = msg.get("text", "").strip().split()[0].lower()
                if cmd in HANDLERS:
                    send(HANDLERS[cmd]())
                elif cmd.startswith("/"):
                    send("Commands: /status /blocked /budget /pause /resume")
        except Exception as e:
            time.sleep(10)


if __name__ == "__main__":
    main()
