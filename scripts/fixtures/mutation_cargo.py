#!/usr/bin/env python3
"""Controlled Cargo/report fixture for test_mutation_runner.py; never invokes Cargo."""

import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time


def main():
    args = sys.argv[1:]
    if args == ["--sleep-child"]:
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        time.sleep(60)
        return 0

    mode = os.environ.get("FAKE_MODE", "mixed")
    if args == ["--version"]:
        print("cargo fixture")
        return 0
    if args == ["mutants", "--version"]:
        print("cargo-mutants 99.0.0" if mode == "wrong_version" else "cargo-mutants 27.1.0")
        return 0
    assert args[0] == "mutants", args
    for flag in ["--package=exploderous-core", "--test-package=exploderous-core",
                 "--cargo-arg=--locked", "--cargo-arg=--all-targets", "--no-config"]:
        assert flag in args, args
    assert not any(a.startswith(("--exclude=", "--file", "--in-place", "--workspace")) for a in args)
    names = [f"crates/core/src/lib.rs:{i}:1: replace == with !=" for i in range(1, 5)]
    rng = [
        "crates/core/src/game.rs:567:42: replace % with / in random_choice",
        "crates/core/src/game.rs:571:33: replace >> with << in random_choice",
        "crates/core/src/game.rs:572:24: replace ^ with | in random_choice",
        "crates/core/src/game.rs:572:24: replace ^ with & in random_choice",
        "crates/core/src/game.rs:572:33: replace >> with << in random_choice",
        "crates/core/src/game.rs:573:24: replace >> with << in random_choice",
        "crates/core/src/game.rs:574:18: replace >= with < in random_choice",
    ]
    names += rng
    if mode == "rng_exclusions":
        names += [
            "crates/core/src/game.rs:575:27: replace % with / in random_choice",
            "crates/core/src/game.rs:567:42: replace % with * in random_choice",
            "crates/core/src/game.rs:567:42: replace % with / in other_random_choice",
            "crates/core/src/game.rs:567:42: replace % with / in random_choice_extra",
            "crates/core/src/other.rs:567:42: replace % with / in random_choice",
        ]
    if mode == "empty":
        names = rng
    if mode == "exclusion_drift":
        names[-1] = names[-1].replace(":574:18:", ":575:18:")
    patterns = [args[i + 1] for i, arg in enumerate(args) if arg == "--exclude-re"]
    if "--list" not in args and mode != "ignored_exclusions":
        names = [name for name in names if not any(re.search(pattern, name) for pattern in patterns)]
    inventory = [{"name": name, "diff": "fixture diff", "package": "exploderous-core"} for name in names]
    if "--list" in args:
        assert "--json" in args and "--diff" in args and not patterns
        print(json.dumps(inventory))
        return 0
    assert "--shuffle" in args and "--baseline=run" in args
    assert f"--jobs={os.environ.get('EXPECT_JOBS', '2')}" in args
    out = Path(args[args.index("--output") + 1]) / "mutants.out"
    (out / "log").mkdir(parents=True)
    (out / "diff").mkdir()
    (out / "mutants.json").write_text(json.dumps(list(reversed(inventory))))
    if mode == "excluded_outcome":
        names.append(rng[0])
    (out / "log/baseline.log").write_text("\n*** baseline\n\n*** result: Success\n")
    rows = [{"scenario": "Baseline", "summary": "Success", "log_path": "log/baseline.log",
             "diff_path": None, "phase_results": []}]
    if mode in ("baseline_failure", "baseline_timeout"):
        rows[0]["summary"] = "Failure" if mode == "baseline_failure" else "Timeout"
        (out / "outcomes.json").write_text(json.dumps({"outcomes": rows, "end_time": "2026-01-01T00:00:00Z"}))
        return 4
    if mode in ("hang", "hang_baseline"):
        child = subprocess.Popen([sys.executable, __file__, "--sleep-child"], process_group=0)
        Path(os.environ["CHILD_PID_FILE"]).write_text(str(child.pid))
        (out / "log/m0.log").write_text(f"\n*** {names[0]}\n")
        (out / "diff/m0.diff").write_text("fixture diff")
        rows.append({"scenario": {"Mutant": {"name": names[0]}}, "summary": "CaughtMutant",
                     "log_path": "log/m0.log", "diff_path": "diff/m0.diff", "phase_results": []})
        (out / "log/m1.log").write_text(f"\n*** {names[1]}\n")
        (out / "diff/m1.diff").write_text("fixture diff")
        if mode == "hang_baseline":
            rows = []
            for path in list((out / "log").glob("m*.log")) + list((out / "diff").glob("*")):
                path.unlink()
        (out / "outcomes.json").write_text(json.dumps({"outcomes": rows, "end_time": None}))
        time.sleep(60)
    if mode == "tool_error":
        return 70
    statuses = ["CaughtMutant"] * len(names)
    if mode == "mixed":
        statuses[:4] = ["CaughtMutant", "MissedMutant", "Unviable", "Timeout"]
    if mode == "survivor":
        statuses[1] = "MissedMutant"
    if mode == "timeout_outcome":
        statuses[1] = "Timeout"
    for i, (name, status) in enumerate(zip(names, statuses)):
        log, diff = f"log/m{i}.log", f"diff/m{i}.diff"
        (out / log).write_text(f"\n*** {name}\n")
        (out / diff).write_text("fixture diff")
        rows.append({"scenario": {"Mutant": {"name": name}}, "summary": status,
                     "log_path": log, "diff_path": diff, "phase_results": []})
    (out / "outcomes.json").write_text(json.dumps({"outcomes": rows, "end_time": "2026-01-01T00:00:00Z",
        "total_mutants": len(names), "caught": statuses.count("CaughtMutant"), "missed": statuses.count("MissedMutant"),
        "unviable": statuses.count("Unviable"), "timeout": statuses.count("Timeout"),
        "cargo_mutants_version": "27.1.0"}))
    if mode == "missing_diff":
        (out / "diff/m0.diff").unlink()
    if mode == "missing_diff_reference":
        rows[1]["diff_path"] = None
        (out / "outcomes.json").write_text(json.dumps({"outcomes": rows, "end_time": "2026-01-01T00:00:00Z"}))
    if mode == "truncated":
        (out / "outcomes.json").write_text('{"outcomes": [')
    print("fixture scan completed")
    return 3 if mode in ("mixed", "timeout_outcome") else 2 if mode == "survivor" else 0


if __name__ == "__main__":
    sys.exit(main())
