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
    env = dict(os.environ, PATH=f"{binary}:{os.environ['PATH']}")
    print("Checking mixed outcomes and readable reports...", flush=True)
    output = base / "mixed"
    result = subprocess.run(
        [sys.executable, str(RUNNER), "--output", str(output)],
        cwd=base, env=env, text=True, capture_output=True, timeout=15,
    )
    assert result.returncode == 3, (result.returncode, result.stdout, result.stderr)
    summary = json.loads((output / "summary.json").read_text())
    assert summary["counts"] == {
        "generated": 4, "attempted": 4, "completed": 4, "untested": 0, "interrupted": 0,
        "caught": 1, "surviving": 1, "unviable": 1, "timed_out": 1,
    }, summary
    assert summary["baseline"] == "Success" and summary["complete"] is True
    assert "Unviable" in (output / "summary.md").read_text()
    assert "https://mutants.rs/using-results.html" in (output / "summary.md").read_text()
    assert (output / "inventory.json").is_file() and (output / "console.log").is_file()
    assert (output / "metadata.json").is_file()

    for mode, expected in [("clean", 0), ("survivor", 2), ("timeout_outcome", 3),
                           ("baseline_failure", 4), ("baseline_timeout", 4), ("tool_error", 70),
                           ("wrong_version", 1), ("empty", 1), ("truncated", 1),
                           ("missing_diff", 1), ("missing_diff_reference", 1)]:
        print(f"Checking {mode.replace('_', ' ')} (expected exit {expected})...", flush=True)
        output = base / mode
        result = subprocess.run(
            [sys.executable, str(RUNNER), "--output", str(output)], cwd=base,
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

    print("Checking worker override and automatic output directory...", flush=True)
    result = subprocess.run([sys.executable, str(RUNNER), "--jobs", "1"], cwd=base,
                            env=dict(env, TMPDIR=str(base), FAKE_MODE="clean", EXPECT_JOBS="1"),
                            text=True, capture_output=True, timeout=15)
    assert result.returncode == 0, (result.stdout, result.stderr)
    auto_output = Path(result.stdout.splitlines()[0].removeprefix("Reports: "))
    assert auto_output.parent == base and (auto_output / "summary.md").is_file()

    print("Checking invalid arguments and unsafe output locations...", flush=True)
    # Unsafe inputs must fail before creating reports or launching Cargo.
    for options in [["--budget", "nan"], ["--budget", "inf"], ["--jobs", "0"],
                    ["--output", str(base / "clean")], ["--output", str(ROOT / "must-not-exist")]]:
        result = subprocess.run([sys.executable, str(RUNNER), *options], cwd=base,
                                env=env, text=True, capture_output=True, timeout=15)
        assert result.returncode == 2, (options, result.stdout, result.stderr)
    assert not (ROOT / "must-not-exist").exists()

    print("Checking mutation timeout cleanup (intentional 3-second deadline)...", flush=True)
    # A separate child process group must not escape the runner's elapsed deadline.
    pid_file = base / "child.pid"
    output = base / "timeout"
    try:
        result = subprocess.run(
            [sys.executable, str(RUNNER), "--budget", "3", "--output", str(output)],
            cwd=base, env=dict(env, FAKE_MODE="hang", CHILD_PID_FILE=str(pid_file)),
            text=True, capture_output=True, timeout=15,
        )
        assert result.returncode == 124, (result.stdout, result.stderr)
        summary = json.loads((output / "summary.json").read_text())
        assert summary["counts"] == {
            "generated": 4, "attempted": 2, "completed": 1, "untested": 2, "interrupted": 1,
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
            [sys.executable, str(RUNNER), "--budget", "10" if cancel else "3", "--output", str(output)],
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
