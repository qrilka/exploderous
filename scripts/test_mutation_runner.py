"""Exercise the public runner CLI without running mutation tests: python3 scripts/test_mutation_runner.py."""

import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "scripts/run_mutation_tests.py"
CARGO_FIXTURE = ROOT / "scripts/fixtures/mutation_cargo.py"

with tempfile.TemporaryDirectory(prefix="exploderous-mutation-check-") as directory:
    base = Path(directory)
    binary = base / "bin"
    binary.mkdir()
    shutil.copyfile(CARGO_FIXTURE, binary / "cargo")
    (binary / "cargo").chmod(0o755)
    (binary / "rustc").write_text("#!/bin/sh\nprintf 'rustc fixture\\n'\n")
    (binary / "rustc").chmod(0o755)
    ast_grep = shutil.which("ast-grep")
    assert ast_grep, "ast-grep 0.51.0 is required for real-parser fixture checks"
    (binary / "ast-grep").write_text('''#!/usr/bin/env python3
import os, sys
if os.environ.get("FAKE_MODE") == "wrong_ast_version" and sys.argv[1:] == ["--version"]:
    print("ast-grep 0.0.0")
elif os.environ.get("FAKE_MODE") == "ast_error" and sys.argv[1] == "scan":
    sys.exit(42)
else:
    os.execv(os.environ["REAL_AST_GREP"], [os.environ["REAL_AST_GREP"], *sys.argv[1:]])
''')
    (binary / "ast-grep").chmod(0o755)
    checkout = base / "checkout"
    subprocess.run(["git", "clone", "--quiet", "--no-hardlinks", str(ROOT), str(checkout)], check=True)
    shutil.copyfile(RUNNER, checkout / "scripts/run_mutation_tests.py")
    shutil.copyfile(ROOT / "scripts/mutation_exclusions.py", checkout / "scripts/mutation_exclusions.py")
    source_path = checkout / "crates/core/src/game.rs"
    source = (ROOT / "scripts/fixtures/mutation_rng.rs").read_text()
    source_path.write_text(source)
    blast_path = checkout / "crates/core/src/game/simulation.rs"
    blast_source = (ROOT / "scripts/fixtures/mutation_blast.rs").read_text()
    blast_path.write_text(blast_source)
    runner = checkout / "scripts/run_mutation_tests.py"
    env = dict(os.environ, PATH=f"{binary}:{os.environ['PATH']}", REAL_AST_GREP=ast_grep)
    print("Checking 17 exclusions and nearby eligible mutations...", flush=True)
    output = base / "exclusion-policy"
    result = subprocess.run(
        [sys.executable, str(runner), "--output", str(output)], cwd=base,
        env=dict(env, FAKE_MODE="policy_exclusions"), text=True, capture_output=True, timeout=15,
    )
    assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
    summary = json.loads((output / "summary.json").read_text())
    assert summary["counts"] == {
        "generated": 38, "excluded": 17, "eligible": 21,
        "attempted": 21, "completed": 21, "untested": 0, "interrupted": 0,
        "caught": 21, "surviving": 0, "unviable": 0, "timed_out": 0,
    }, summary
    excluded = json.loads((output / "excluded.json").read_text())
    assert "#38" in excluded["reason"] and len(excluded["mutations"]) == 17
    excluded_names = {m["name"] for m in excluded["mutations"]}
    rng_ids = {"threshold:/", "shift30:<<", "xor27:|", "xor27:&",
               "shift27:<<", "shift31:<<", "acceptance:<"}
    equivalent_ids = {"boundary-first:-", "boundary-first:*", "boundary-remainder:/",
                      "boundary-remainder:+", "boundary-edge:!=", "dedup-left:+",
                      "dedup-left:/", "dedup-right:/", "ray-limit:+", "ray-limit:/"}
    expected_ids = rng_ids | equivalent_ids
    assert {m["fixture_id"] for m in excluded["mutations"]} == expected_ids, excluded
    for kind, identities in [("rng-policy", rng_ids), ("equivalent", equivalent_ids)]:
        selected = [m for m in excluded["mutations"] if m["kind"] == kind]
        assert {m["fixture_id"] for m in selected} == identities
        assert all(m["reason"] for m in selected)
    conditional = next(m for m in excluded["mutations"] if m["fixture_id"] == "dedup-left:+")
    assert "Buroru" in conditional["reason"] and "8" in conditional["reason"]
    generated_names = {m["name"] for m in json.loads((output / "inventory.json").read_text())}
    eligible_names = {m["name"] for m in json.loads((output / "mutants.out/mutants.json").read_text())}
    assert generated_names == excluded_names | eligible_names
    assert not excluded_names & eligible_names
    eligible = json.loads((output / "mutants.out/mutants.json").read_text())
    negative_ids = {"return-mod:/", "threshold:*", "other-fn:/", "similar-fn:/", "other-file:/", "xor30:|",
                    "sort-left:+", "sort-right:/", "midpoint-left:+", "midpoint-right:/",
                    "dedup-right:+", "boundary-remainder:*", "other-boundary:*", "other-dedup:/",
                    "other-limit:+", "other-ray:+", "other-contact-file:/"}
    assert negative_ids <= {m.get("fixture_id") for m in eligible}, eligible
    readable = (output / "summary.md").read_text()
    assert "Excluded by policy" in readable and excluded["reason"] in readable
    assert "RNG scope policy (7)" in readable and "Valid-state equivalence (10)" in readable
    assert all(f"`{name}`" in readable for name in excluded_names)
    assert all(m["reason"] in readable for m in excluded["mutations"])
    metadata = json.loads((output / "metadata.json").read_text())
    assert any("--exclude-re" in command for command in metadata["commands"])
    workflow = (ROOT / ".github/workflows/mutation-tests.yml").read_text()
    assert 'run: python3 scripts/run_mutation_tests.py --output "$MUTATION_REPORTS"' in workflow

    policy_counts = summary["counts"]
    for case, changed, changed_blast in [
        ("moved", "\n" * 40 + source, "\n" * 20 + blast_source),
        ("compact", source.replace("\n", " "), blast_source.replace("\n", " ")),
        ("commented", source.replace("times.dedup_by", "times /* é */ .dedup_by"),
         blast_source.replace("axis_length", "axis_length /* é */")),
    ]:
        print(f"Checking {case} Rust with real ast-grep...", flush=True)
        source_path.write_text(changed)
        blast_path.write_text(changed_blast)
        output = base / case
        result = subprocess.run(
            [sys.executable, str(runner), "--output", str(output)], cwd=base,
            env=dict(env, FAKE_MODE="policy_exclusions"), text=True, capture_output=True, timeout=15,
        )
        assert result.returncode == 0, (case, result.stdout, result.stderr)
        assert json.loads((output / "summary.json").read_text())["counts"] == policy_counts
        selected = json.loads((output / "excluded.json").read_text())["mutations"]
        assert {m["fixture_id"] for m in selected} == expected_ids
        assert {m["name"] for m in selected} != excluded_names, "fixture coordinates did not move"
        eligible = json.loads((output / "mutants.out/mutants.json").read_text())
        assert negative_ids <= {m.get("fixture_id") for m in eligible}
    source_path.write_text(source)
    blast_path.write_text(blast_source)

    print("Checking mixed outcomes and readable reports...", flush=True)
    output = base / "mixed"
    result = subprocess.run(
        [sys.executable, str(runner), "--output", str(output)],
        cwd=base, env=env, text=True, capture_output=True, timeout=15,
    )
    assert result.returncode == 3, (result.returncode, result.stdout, result.stderr)
    summary = json.loads((output / "summary.json").read_text())
    assert summary["counts"] == {
        "generated": 21, "excluded": 17, "eligible": 4,
        "attempted": 4, "completed": 4, "untested": 0, "interrupted": 0,
        "caught": 1, "surviving": 1, "unviable": 1, "timed_out": 1,
    }, summary
    assert summary["baseline"] == "Success" and summary["complete"] is True
    assert "Unviable" in (output / "summary.md").read_text()
    assert "https://mutants.rs/using-results.html" in (output / "summary.md").read_text()
    assert (output / "inventory.json").is_file() and (output / "console.log").is_file()
    assert (output / "metadata.json").is_file()

    for mode, expected in [("clean", 0), ("survivor", 2), ("timeout_outcome", 3),
                           ("baseline_failure", 4), ("baseline_timeout", 4), ("tool_error", 70),
                           ("wrong_version", 1), ("wrong_ast_version", 1), ("ast_error", 1),
                           ("empty", 1), ("exclusion_drift", 1), ("ambiguous_target", 1),
                           ("boundary_drift", 1), ("dedup_drift", 1), ("blast_drift", 1),
                           ("ambiguous_boundary", 1), ("ambiguous_dedup", 1), ("ambiguous_blast", 1),
                           ("missing_equivalent", 1),
                           ("ignored_exclusions", 1), ("excluded_outcome", 1), ("truncated", 1),
                           ("missing_diff", 1), ("missing_diff_reference", 1)]:
        print(f"Checking {mode.replace('_', ' ')} (expected exit {expected})...", flush=True)
        output = base / mode
        changed = source.replace(">> 27", ">> 28") if mode == "exclusion_drift" else source
        if mode == "ambiguous_target":
            changed = source.replace("let threshold =", "let duplicate = count.wrapping_neg() % count; let threshold =")
        if mode == "boundary_drift":
            changed = source.replace("/* boundary-first */ + 1", "/* boundary-first */ + 2")
        if mode == "dedup_drift":
            changed = source.replace("times.dedup_by", "times.other_dedup")
        if mode == "ambiguous_boundary":
            changed = source.replace("let first =", "let duplicate = start.min(end).div_euclid(WORLD_UNITS_PER_CELL) + 1; let first =", 1)
        if mode == "ambiguous_dedup":
            changed = source.replace("a /* dedup-left */ * d", "(a * d) + a /* dedup-left */ * d")
        changed_blast = blast_source
        if mode == "blast_drift":
            changed_blast = blast_source.replace("/* ray-limit */ - 1", "/* ray-limit */ - 2")
        if mode == "ambiguous_blast":
            changed_blast = blast_source.replace("/* other-limit */ - 2", "/* other-limit */ - 1")
        source_path.write_text(changed)
        blast_path.write_text(changed_blast)
        result = subprocess.run(
            [sys.executable, str(runner), "--output", str(output)], cwd=base,
            env=dict(env, FAKE_MODE=mode), text=True, capture_output=True, timeout=15,
        )
        assert result.returncode == expected, (mode, result.returncode, result.stdout, result.stderr)
        summary = json.loads((output / "summary.json").read_text())
        if mode == "clean":
            assert summary["complete"] is True and summary["counts"]["caught"] == 4
        elif mode.startswith("baseline_"):
            assert not summary["complete"] and summary["counts"]["attempted"] == 0
            assert summary["counts"]["untested"] == 4
        elif mode in ("survivor", "timeout_outcome"):
            assert summary["complete"]
        else:
            assert not summary["complete"], summary
        if mode in ("empty", "exclusion_drift", "ambiguous_target", "wrong_ast_version", "ast_error",
                    "boundary_drift", "dedup_drift", "blast_drift", "ambiguous_boundary",
                    "ambiguous_dedup", "ambiguous_blast", "missing_equivalent"):
            commands = json.loads((output / "metadata.json").read_text())["commands"]
            assert not any("--baseline=run" in command for command in commands), commands
            expected_reason = {
                "empty": "eligible inventory must not be empty",
                "exclusion_drift": "exclusion source drift",
                "ambiguous_target": "expected one AST match for threshold, got 2",
                "wrong_ast_version": "ast-grep 0.51.0 is required",
                "ast_error": "ast-grep exited 42",
                "boundary_drift": "expected one AST match for boundary-first, got 0",
                "dedup_drift": "expected one AST match for dedup-left-sum, got 0",
                "blast_drift": "expected one AST match for ray-limit, got 0",
                "ambiguous_boundary": "expected one AST match for boundary-first, got 2",
                "ambiguous_dedup": "expected one AST match for dedup-left-sum, got 2",
                "ambiguous_blast": "expected one AST match for ray-limit, got 2",
                "missing_equivalent": "expected one mutation for ray-limit -> /, got 0",
            }
            assert expected_reason[mode] in summary["reason"], summary
        if mode == "ignored_exclusions":
            assert "scan inventory differs" in summary["reason"]
        if mode == "excluded_outcome":
            assert "unexpected or duplicate mutation outcome" in summary["reason"]

    source_path.write_text(source)
    blast_path.write_text(blast_source)
    print("Checking worker override and automatic output directory...", flush=True)
    result = subprocess.run([sys.executable, str(runner), "--jobs", "1"], cwd=base,
                            env=dict(env, TMPDIR=str(base), FAKE_MODE="clean", EXPECT_JOBS="1"),
                            text=True, capture_output=True, timeout=15)
    assert result.returncode == 0, (result.stdout, result.stderr)
    auto_output = Path(result.stdout.splitlines()[0].removeprefix("Reports: "))
    assert auto_output.parent == base and (auto_output / "summary.md").is_file()

    print("Checking invalid arguments and unsafe output locations...", flush=True)
    # Unsafe inputs must fail before creating reports or launching Cargo.
    for options in [["--budget", "nan"], ["--budget", "inf"], ["--jobs", "0"],
                    ["--output", str(base / "clean")], ["--output", str(checkout / "must-not-exist")]]:
        result = subprocess.run([sys.executable, str(runner), *options], cwd=base,
                                env=env, text=True, capture_output=True, timeout=15)
        assert result.returncode == 2, (options, result.stdout, result.stderr)
    assert not (checkout / "must-not-exist").exists()

    print("Checking mutation timeout cleanup (intentional 3-second deadline)...", flush=True)
    # A separate child process group must not escape the runner's elapsed deadline.
    pid_file = base / "child.pid"
    output = base / "timeout"
    try:
        result = subprocess.run(
            [sys.executable, str(runner), "--budget", "3", "--output", str(output)],
            cwd=base, env=dict(env, FAKE_MODE="hang", CHILD_PID_FILE=str(pid_file)),
            text=True, capture_output=True, timeout=15,
        )
        assert result.returncode == 124, (result.stdout, result.stderr)
        summary = json.loads((output / "summary.json").read_text())
        assert summary["counts"] == {
            "generated": 21, "excluded": 17, "eligible": 4,
            "attempted": 2, "completed": 1, "untested": 2, "interrupted": 1,
            "caught": 1, "surviving": 0, "unviable": 0, "timed_out": 0,
        }, summary
        pid = int(pid_file.read_text())
        stat = Path(f"/proc/{pid}/stat")
        assert not stat.exists() or stat.read_text().rsplit(")", 1)[1].split()[0] == "Z", "launched child survived deadline"
    finally:
        if pid_file.exists():
            try:
                os.kill(int(pid_file.read_text()), signal.SIGKILL)
            except ProcessLookupError:
                pass

    # Cancellation uses the same cleanup path, including when baseline has not finished.
    for mode, cancel in [("hang_baseline", False), ("hang", True)]:
        check = "SIGTERM cancellation cleanup" if cancel else "baseline timeout cleanup (intentional 3-second deadline)"
        print(f"Checking {check}...", flush=True)
        pid_file = base / f"{mode}-cancel-{cancel}.pid"
        output = base / f"{mode}-cancel-{cancel}"
        process = subprocess.Popen(
            [sys.executable, str(runner), "--budget", "10" if cancel else "3", "--output", str(output)],
            cwd=base, env=dict(env, FAKE_MODE=mode, CHILD_PID_FILE=str(pid_file)),
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        try:
            if cancel:
                import time
                deadline = time.monotonic() + 5
                while not pid_file.exists() and time.monotonic() < deadline:
                    time.sleep(0.01)
                assert pid_file.exists(), "fixture failed to launch child"
                process.send_signal(signal.SIGTERM)
            stdout, stderr = process.communicate(timeout=15)
            assert process.returncode == (143 if cancel else 124), (stdout, stderr)
            summary = json.loads((output / "summary.json").read_text())
            assert not summary["complete"]
            if not cancel:
                assert summary["counts"]["untested"] == 4 and summary["counts"]["attempted"] == 0
            stat = Path(f"/proc/{int(pid_file.read_text())}/stat")
            assert not stat.exists() or stat.read_text().rsplit(")", 1)[1].split()[0] == "Z"
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            if pid_file.exists():
                try:
                    os.kill(int(pid_file.read_text()), signal.SIGKILL)
                except ProcessLookupError:
                    pass

print("mutation runner checks passed")
