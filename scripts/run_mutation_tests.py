"""Bounded, core-only cargo-mutants execution (Linux, Python 3.11+)."""

import argparse
from collections import Counter
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
VERSION = "27.1.0"
CARGO_ARGS = [
    "cargo", "mutants", "--package=exploderous-core", "--test-package=exploderous-core",
    "--cargo-arg=--locked", "--cargo-arg=--all-targets", "--no-config",
]
OUTCOMES = {"CaughtMutant": "caught", "MissedMutant": "surviving",
            "Unviable": "unviable", "Timeout": "timed_out"}
# Exact, reviewed source locations: drift requires policy review, not a broader match.
RNG_EXCLUSIONS = (
    "crates/core/src/game.rs:567:42: replace % with / in random_choice",
    "crates/core/src/game.rs:571:33: replace >> with << in random_choice",
    "crates/core/src/game.rs:572:24: replace ^ with | in random_choice",
    "crates/core/src/game.rs:572:24: replace ^ with & in random_choice",
    "crates/core/src/game.rs:572:33: replace >> with << in random_choice",
    "crates/core/src/game.rs:573:24: replace >> with << in random_choice",
    "crates/core/src/game.rs:574:18: replace >= with < in random_choice",
)
RNG_EXCLUSION_REASON = (
    "Policy exclusion approved in #38: six known RNG-quality mutations and one "
    "mutant nontermination case are outside this assessment's scope. "
    "Excluded is not caught or proven equivalent; other RNG mutations remain eligible."
)


def save(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def session_processes(session):
    """Include Cargo's separate process groups, not just the supervisor's group."""
    processes = {}
    for path in Path("/proc").glob("[0-9]*/stat"):
        try:
            fields = path.read_text().rsplit(")", 1)[1].split()
        except (FileNotFoundError, ProcessLookupError):
            continue
        if int(fields[3]) == session and fields[0] not in ("Z", "X"):
            processes[int(path.parent.name)] = fields[0]
    return processes


def send(pid, signum):
    try:
        os.kill(pid, signum)
    except ProcessLookupError:
        pass


def stop_session(child):
    # Freeze the session before killing children so the supervisor cannot dispatch
    # another mutation. A second scan includes children forked during the first.
    handlers = {s: signal.signal(s, signal.SIG_IGN) for s in (signal.SIGINT, signal.SIGTERM)}
    killed = set()
    try:
        deadline = time.monotonic() + 2
        while True:
            members = session_processes(child.pid)
            for pid in members:
                send(pid, signal.SIGSTOP)
            if all(state in ("T", "t") for state in members.values()) or time.monotonic() > deadline:
                break
            time.sleep(0.01)
        for pid in members:
            if pid != child.pid:
                send(pid, signal.SIGKILL)
                killed.add(pid)
        # Only supervisor cleanup is allowed after the deadline. This lets
        # cargo-mutants delete its copied scratch tree and retain earlier results.
        if child.poll() is None:
            send(child.pid, signal.SIGINT)
            send(child.pid, signal.SIGCONT)
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        deadline = time.monotonic() + 1
        while remaining := session_processes(child.pid):
            for pid in remaining:
                send(pid, signal.SIGKILL)
                killed.add(pid)
            if time.monotonic() > deadline:
                raise RuntimeError(f"process cleanup incomplete: {sorted(remaining)}")
            time.sleep(0.01)
        return sorted(killed)
    finally:
        for signum, handler in handlers.items():
            signal.signal(signum, handler)


def record_exclusions(output):
    inventory = json.loads((output / "inventory.json").read_text())
    excluded = [m for m in inventory if m["name"] in RNG_EXCLUSIONS]
    save(output / "excluded.json", {"reason": RNG_EXCLUSION_REASON, "mutations": excluded})
    missing = set(RNG_EXCLUSIONS) - {m["name"] for m in excluded}
    if missing:
        raise ValueError(f"RNG exclusion source drift: review #38 before scanning; missing {sorted(missing)}")


def summarize(output):
    inventory = json.loads((output / "inventory.json").read_text())
    generated = {m["name"] for m in inventory}
    if len(generated) != len(inventory):
        raise ValueError("inventory must contain unique mutations")
    exclusions = json.loads((output / "excluded.json").read_text())
    excluded = {m["name"] for m in exclusions["mutations"]}
    if (len(excluded) != len(exclusions["mutations"]) or excluded != set(RNG_EXCLUSIONS)
            or not excluded <= generated):
        raise ValueError("excluded inventory does not match the approved policy")
    names = generated - excluded
    if not names:
        raise ValueError("eligible inventory must not be empty")
    raw = output / "mutants.out"
    scan_inventory = raw / "mutants.json"
    if scan_inventory.exists():
        scanned = json.loads(scan_inventory.read_text())
        if len(scanned) != len(names) or {m["name"] for m in scanned} != names:
            raise ValueError("scan inventory differs from the eligible inventory")
    results_path = raw / "outcomes.json"
    results = json.loads(results_path.read_text()) if results_path.exists() else {}
    errors = []
    baseline = "Not completed"
    mutations = {}
    completed_logs = set()
    for row in results.get("outcomes", []):
        for key in (["log_path"] if row["scenario"] == "Baseline" else ["log_path", "diff_path"]):
            if not row.get(key):
                errors.append(f"Missing evidence reference: {key}")
                continue
            path = (raw / row[key]).resolve()
            if not path.is_relative_to(raw) or not path.is_file():
                errors.append(f"Missing or invalid evidence: {row[key]}")
        if row["scenario"] == "Baseline":
            baseline = row["summary"]
            continue
        name = row["scenario"]["Mutant"]["name"]
        if name not in names or name in mutations or row["summary"] not in OUTCOMES:
            raise ValueError("unexpected or duplicate mutation outcome")
        mutations[name] = OUTCOMES[row["summary"]]
        completed_logs.add(row["log_path"])
    started_logs = {f"log/{p.name}" for p in (raw / "log").glob("*.log") if p.name != "baseline.log"}
    attempted = len(started_logs | completed_logs)
    if not len(mutations) <= attempted <= len(names):
        raise ValueError("mutation log and inventory counts do not reconcile")
    counts = dict.fromkeys(OUTCOMES.values(), 0)
    counts.update(Counter(mutations.values()))
    counts.update(generated=len(generated), excluded=len(excluded), eligible=len(names),
                  attempted=attempted, completed=len(mutations), untested=len(names) - attempted,
                  interrupted=attempted - len(mutations))
    complete = (bool(results.get("end_time")) and baseline == "Success"
                and len(mutations) == len(names) and scan_inventory.is_file() and not errors)
    return {"counts": counts, "exclusions": exclusions, "baseline": baseline,
            "complete": complete, "evidence_errors": errors}


def report(output, summary):
    counts = summary.get("counts")
    lines = ["# Core mutation testing", "", f"**{summary['reason']}**", "",
             f"Elapsed: {summary['elapsed_seconds']:.2f}s. Exit status: {summary['exit_code']}.",
             f"Baseline: {summary['baseline']}. Complete scan: {summary['complete']}.", ""]
    if counts:
        lines += ["| Execution | Count |", "| --- | ---: |"]
        lines += [f"| {key} | {counts[key]} |" for key in
                  ["generated", "excluded", "eligible", "attempted", "completed", "untested", "interrupted"]]
        lines += ["", "| Outcome | Count | Meaning |", "| --- | ---: | --- |",
                  f"| Caught | {counts['caught']} | Compiled; tests rejected the change. |",
                  f"| Surviving | {counts['surviving']} | Tests passed; possible gap or equivalent change. |",
                  f"| Unviable | {counts['unviable']} | Did not compile; inconclusive about tests. |",
                  f"| Timed out | {counts['timed_out']} | Evaluation exceeded its limit; not counted as caught. |"]
    if exclusions := summary.get("exclusions"):
        lines += ["", "## Excluded by policy", "", exclusions["reason"], ""]
        lines += [f"- `{m['name']}`" for m in exclusions["mutations"]]
    lines += ["", *summary.get("evidence_errors", []),
              "Untested means never started; interrupted means started without a terminal outcome.",
              "Generated = excluded + eligible. Eligible = completed + untested + interrupted.",
              "Completed includes terminal timeouts, not successful test completion.",
              "Survivors are not automatically bugs; review required behavior before proposing fixes or exclusions.",
              "[Outcome interpretation](https://mutants.rs/using-results.html#mutant-outcomes) · "
              "[Timeouts](https://mutants.rs/timeouts.html) · [Exit codes](https://mutants.rs/exit-codes.html)",
              "", "See `metadata.json`, `inventory.json`, `console.log`, and `mutants.out/` for raw evidence."]
    (output / "summary.md").write_text("\n".join(lines) + "\n")
    save(output / "summary.json", summary)
    print("\n".join(lines), flush=True)


def positive_number(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("must be a finite number greater than zero")
    return number


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--budget", type=positive_number, default=600, help="elapsed seconds, including setup and baseline (default: 600)")
    parser.add_argument("--jobs", type=int, default=2, help="parallel mutation workers (default: 2)")
    parser.add_argument("--output", type=Path, help="new report directory outside the checkout (default: temporary directory)")
    args = parser.parse_args()
    if sys.platform != "linux" or args.jobs < 1:
        parser.error("Linux and --jobs >= 1 are required")
    output = (args.output or Path(tempfile.gettempdir()) / f"exploderous-mutants-{time.time_ns()}").resolve()
    if output.is_relative_to(ROOT):
        parser.error("--output must be outside the checkout")
    try:
        output.mkdir(parents=True, exist_ok=False)
    except OSError as error:
        parser.error(str(error))
    print(f"Reports: {output}", flush=True)
    start = time.monotonic()
    metadata = {"started_utc": datetime.now(timezone.utc).isoformat(), "budget_seconds": args.budget,
                "jobs": args.jobs, "host": platform.platform(), "python": sys.version,
                "cpu_count": len(os.sched_getaffinity(0)), "disk_free_bytes": shutil.disk_usage(output).free,
                "memory": Path("/proc/meminfo").read_text(),
                "build_environment": {key: os.environ[key] for key in
                    ("RUSTUP_TOOLCHAIN", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_DIR",
                     "CARGO_BUILD_TARGET", "CARGO_BUILD_JOBS", "RUST_TEST_THREADS") if key in os.environ},
                "commands": [], "process_cleanup": []}
    exit_code, reason = 1, "Setup failed"

    def run(command, name, separate_errors=False):
        metadata["commands"].append(command)
        save(output / "metadata.json", metadata)
        remaining = args.budget - (time.monotonic() - start)
        if remaining <= 0:
            raise subprocess.TimeoutExpired(command, args.budget)
        with (output / name).open("w") as stdout, (output / f"{name}.stderr").open("w") as stderr:
            child = subprocess.Popen(command, cwd=ROOT, stdin=subprocess.DEVNULL, stdout=stdout,
                                     stderr=stderr if separate_errors else subprocess.STDOUT, start_new_session=True)
            try:
                return child.wait(timeout=remaining)
            finally:
                if session_processes(child.pid):
                    was_running = child.poll() is None
                    killed = stop_session(child)
                    metadata["process_cleanup"].append({"session": child.pid, "killed": killed})
                    if not was_running:
                        raise RuntimeError("subprocess exited with live children; cleaned up")

    def required(command, name):
        code = run(command, name)
        if code:
            raise RuntimeError(f"{' '.join(command)} exited {code}; see {name}")
        return (output / name).read_text().strip()

    def interrupted(signum, _frame):
        raise InterruptedError(signum, "execution interrupted")

    for signum in (signal.SIGINT, signal.SIGTERM):
        signal.signal(signum, interrupted)
    try:
        metadata["revision"] = required(["git", "rev-parse", "HEAD"], "revision.txt")
        required(["git", "status", "--porcelain=v1", "--untracked-files=all"], "checkout.txt")
        required(["git", "diff", "--binary", "HEAD"], "source.patch")
        metadata["cargo_mutants"] = required(["cargo", "mutants", "--version"], "mutants-version.txt")
        if metadata["cargo_mutants"] != f"cargo-mutants {VERSION}":
            raise RuntimeError(f"cargo-mutants {VERSION} is required; no tools were installed or updated")
        metadata["rustc"] = required(["rustc", "--version", "--verbose"], "rustc-version.txt")
        metadata["cargo"] = required(["cargo", "--version"], "cargo-version.txt")
        code = run(CARGO_ARGS + ["--list", "--json", "--diff"], "inventory.json", separate_errors=True)
        if code:
            raise RuntimeError(f"mutation inventory exited {code}; see inventory.json.stderr")
        record_exclusions(output)
        # Reject an empty/malformed eligible inventory before starting a scan.
        summarize(output)
        filters = [arg for name in RNG_EXCLUSIONS for arg in ("--exclude-re", f"^{re.escape(name)}$")]
        command = CARGO_ARGS + filters + ["--baseline=run", "--shuffle", f"--jobs={args.jobs}",
                                "--copy-target=false", "--colors=never", "--output", str(output)]
        exit_code = run(command, "console.log")
        metadata["cargo_mutants_exit_code"] = exit_code
        reason = "cargo-mutants finished"
    except subprocess.TimeoutExpired:
        exit_code, reason = 124, "Elapsed-time budget exhausted; no restart"
    except InterruptedError as error:
        exit_code, reason = 128 + error.errno, str(error)
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        exit_code, reason = 1, str(error)
    try:
        summary = summarize(output)
        if exit_code == 0:
            counts = summary["counts"]
            exit_code = 3 if counts["timed_out"] else 2 if counts["surviving"] else 0
            if not summary["complete"]:
                exit_code = 1
        if reason == "cargo-mutants finished":
            if not summary["complete"]:
                reason = "Scan incomplete or baseline unsuccessful; inspect raw evidence"
            elif exit_code:
                reason = f"Scan complete with findings or tool failure (exit {exit_code})"
            else:
                reason = "Scan complete; all viable eligible mutations were caught"
    except (OSError, ValueError, KeyError, TypeError) as error:
        summary = {"counts": None, "baseline": "Unknown", "complete": False}
        exit_code = exit_code or 1
        reason += f"; incomplete report evidence: {error}"
    if exit_code < 0:
        exit_code = 128 - exit_code
    summary.update(exit_code=exit_code, reason=reason, elapsed_seconds=time.monotonic() - start)
    metadata["finished_utc"] = datetime.now(timezone.utc).isoformat()
    save(output / "metadata.json", metadata)
    report(output, summary)
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
