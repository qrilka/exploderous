## Project scope

Keep project workflow improvements in this repository. Changes to shared skills, installed extensions, or user-wide settings require explicit approval of their global scope.

Create, modify, move, or delete files and directories only under the project root or the system temporary directory. Obtain explicit approval for the specific external location before making changes elsewhere. This applies to direct file operations, commands, subagents, and artifact/output paths; existing directories and past use do not grant approval.

## Development setup

For a new or unconfigured checkout, follow [Development setup](README.md#development-setup). Stop if required setup fails.

## Context economy

- Resolve the target, blockers, checkout state, and baseline before broad source exploration.
- Reuse required reads while their unchanged contents remain available in context; reread after source changes or context loss.
- Locate definitions and callers, then read complete relevant routines and tests. Bound searches and tool output to the current question; expand as dependencies emerge.

## Commit attribution

Before creating a commit, read and follow `docs/agents/commit-attribution.md`.

## Agent skills

### Issue tracker and exploration

Before ticket work or project experiments, read `docs/agents/issue-tracker.md` for GitHub tracking, branches, and experiment artifacts.

### Triage labels

Use the default canonical triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Use the single-context layout. See `docs/agents/domain.md`.

### Implementation references

Before implementing behavior governed by a project design/ADR or by a protocol, platform, framework, or dependency, read and follow `docs/agents/implementation-references.md`.

## Design interviews

When a noticeable change leaves materially different design choices open, or a technical decision or system design needs discussion, read `docs/agents/design-interviews.md`.

## Questionnaires

Use `Q1`–`Q4` as question headers; put full titles and explanations in the question text. Supply 2–4 meaningful choices. The tool owns the appended custom-answer and navigation rows: never author options labelled `Other`, `Type something.`, or `Next`.

For a reserved-label rejection, replace the offending option with a meaningful choice or remove it while retaining 2–4 authored choices, then retry. Custom answers use the tool's automatically appended row.

## Coding and documentation standards

Before planning or editing code or documentation, preparing review handoffs, or reviewing changes, read `CODING_STANDARDS.md`.

## Parallel reviews

For an authorized two-reviewer fan-out, read the installed `pi-subagents` skill and its `references/review-and-validation.md`. Use their compact `runs.all(...)`, fresh read-only reviewer, saved-output, and native async-completion guidance. Consult the full workflow guide only when a required capability is not covered there.
