"""Run with python3 scripts/test_pre_commit.py; tests installation in a fresh clone."""

from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]

with tempfile.TemporaryDirectory(prefix="exploderous-hook-test-") as directory:
    repo = Path(directory) / "source"
    repo.mkdir()

    def git(*args, check=True):
        return subprocess.run(
            ["git", *args], cwd=repo, capture_output=True, text=True, check=check,
        )

    def stage(name, content):
        path = repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        git("add", "--", name)

    def hook(succeeds=True):
        result = git("hook", "run", "pre-commit", check=False)
        assert (result.returncode == 0) == succeeds, result.stderr
        return result.stderr

    git("init", "-q")
    git("config", "core.hooksPath", str(repo / "disabled-hooks"))
    git("config", "user.name", "Hook test")
    git("config", "user.email", "hook-test@example.invalid")
    (repo / ".githooks").mkdir()
    shutil.copy2(ROOT / ".githooks/pre-commit", repo / ".githooks/pre-commit")
    git("add", ".githooks/pre-commit")
    stage(".gitattributes", (ROOT / ".gitattributes").read_bytes())
    stage("legacy.rs", b"x" * 20_001)
    git("-c", "commit.gpgSign=false", "commit", "-qm", "Hook and existing oversized file")

    clone = Path(directory) / "clone"
    git("clone", "--quiet", str(repo), str(clone))
    repo = clone
    assert git("config", "--local", "--get", "core.hooksPath", check=False).returncode == 1
    git("config", "--local", "core.hooksPath", ".githooks")
    git("config", "user.name", "Hook test")
    git("config", "user.email", "hook-test@example.invalid")
    hook()  # No staged changes; the existing oversized file is not checked.

    stage("source.rs", "é".encode() * 10_000)  # Exactly 20,000 bytes, not characters.
    (repo / "source.rs").write_bytes(b"x" * 20_001)
    (repo / "untracked.rs").write_bytes(b"x" * 20_001)
    hook()  # Neither unstaged growth nor an untracked file is checked.

    stage("source.rs", b"x" * 20_001)
    (repo / "source.rs").write_bytes(b"")
    stage("second.rs", b"x" * 20_004)
    error = hook(succeeds=False)  # Staged growth still fails after disk shrinks.
    for expected in ("source.rs", "second.rs", "20,001 bytes", "5,000.25 tokens", "foo/tests.rs", "git add"):
        assert expected in error, error
    rejected = git("-c", "commit.gpgSign=false", "commit", "-qm", "Must be rejected", check=False)
    assert rejected.returncode != 0 and "source.rs" in rejected.stderr, rejected.stderr
    stage("source.rs", b"")
    stage("second.rs", b"")

    attributes = (ROOT / ".gitattributes").read_bytes()
    attributes += b"generated/** linguist-generated=true\nother-generated.rs linguist-generated\n"
    stage(".gitattributes", attributes)
    stage("Cargo.lock", b"x" * 40_000)
    stage("generated/code.rs", b"x" * 40_000)
    stage("other-generated.rs", b"x" * 40_000)
    (repo / ".gitattributes").write_bytes(b"")
    hook()  # Exemptions come from staged attributes, including boolean markers.

    stage(".gitattributes", attributes + b"generated/unmarked.rs -linguist-generated\n")
    stage("generated/unmarked.rs", b"x" * 20_001)
    (repo / ".gitattributes").write_bytes(attributes)
    assert "generated/unmarked.rs" in hook(succeeds=False)
    stage("generated/unmarked.rs", b"")

    strange_name = "space tab\tnewline\n雪.rs"
    stage(strange_name, "é".encode() * 10_001)
    error = hook(succeeds=False)
    assert repr(strange_name) in error and "20,002 bytes" in error, error
    stage(strange_name, b"")

    git("mv", "legacy.rs", "renamed.rs")
    assert "renamed.rs" in hook(succeeds=False)
    git("rm", "-f", "renamed.rs")
    hook()  # Deleting an oversized file must not block the commit.

    (repo / "type-change.rs").symlink_to("source.rs")
    git("add", "type-change.rs")
    git("-c", "commit.gpgSign=false", "commit", "-qm", "Symlink fixture")
    (repo / "type-change.rs").unlink()
    stage("type-change.rs", b"x" * 20_001)
    assert "type-change.rs" in hook(succeeds=False)
    stage("type-change.rs", b"")

    commit = git("rev-parse", "HEAD").stdout.strip()
    git("update-index", "--add", "--cacheinfo", f"160000,{commit},submodule")
    hook()  # Gitlinks are not file blobs.

    git("-c", "commit.gpgSign=false", "commit", "-qm", "Commit through the hook")
    hook()

print("pre-commit checks passed")
