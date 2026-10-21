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


def mutation(marker, operator, replacement, function="random_choice", file="crates/core/src/game.rs"):
    source = Path("crates/core/src/game.rs").read_text()
    tag = f"/* {marker} */"
    offset = source.index(operator, source.index(tag) + len(tag))
    prefix = source[:offset]
    line, column = prefix.count("\n") + 1, len(prefix.rsplit("\n", 1)[-1]) + 1
    return {
        "name": f"{file}:{line}:{column}: replace {operator} with {replacement} in {function}",
        "file": file, "replacement": replacement, "genre": "BinaryOperator",
        "function": {"function_name": function}, "package": "exploderous-core",
        "span": {"start": {"line": line, "column": column},
                 "end": {"line": line, "column": column + len(operator)}},
        "diff": "fixture diff", "fixture_id": f"{marker}:{replacement}",
    }


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
    inventory = [{"name": f"crates/core/src/lib.rs:{i}:1: replace == with !=",
                  "diff": "fixture diff", "package": "exploderous-core"} for i in range(1, 5)]
    rng = [
        mutation("threshold", "%", "/"), mutation("shift30", ">>", "<<"),
        mutation("xor27", "^", "|"), mutation("xor27", "^", "&"),
        mutation("shift27", ">>", "<<"), mutation("shift31", ">>", "<<"),
        mutation("acceptance", ">=", "<"),
    ]
    inventory += rng
    if mode == "rng_exclusions":
        inventory += [
            mutation("return-mod", "%", "/"), mutation("threshold", "%", "*"),
            mutation("other-fn", "%", "/", "other_random_choice"),
            mutation("similar-fn", "%", "/", "random_choice_extra"),
            mutation("threshold", "%", "/", file="crates/core/src/other.rs"),
            mutation("xor30", "^", "|"),
        ]
        inventory[-2]["fixture_id"] = "other-file:/"
    if mode == "empty":
        inventory = rng
    patterns = [args[i + 1] for i, arg in enumerate(args) if arg == "--exclude-re"]
    if "--list" not in args and mode != "ignored_exclusions":
        inventory = [m for m in inventory if not any(re.search(p, m["name"]) for p in patterns)]
    names = [m["name"] for m in inventory]
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
        names.append(rng[0]["name"])
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
