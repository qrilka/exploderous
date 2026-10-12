## Project scope

Keep project workflow improvements in this repository. Changes to shared skills, installed extensions, or user-wide settings require explicit approval of their global scope.

## Commit attribution

Before creating a commit, read and follow `docs/agents/commit-attribution.md`.

## Agent skills

### Issue tracker and ticket branches

Before starting ticket work, read `docs/agents/issue-tracker.md` for GitHub issue tracking and branch workflow.

### Triage labels

Use the default canonical triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Use the single-context layout. See `docs/agents/domain.md`.

## Design interviews

Before asking for a technical decision, explain unfamiliar terms in plain language with a concrete example. For representation or geometry choices, show a small diagram or code sketch.

Before final confirmation of a system design, show the complete model and interactions covering every required domain object, including objects outside the first delivery slice. Distinguish settled relationships from open rules.

## Questionnaires

Use `Q1`–`Q4` as question headers; put full titles and explanations in the question text. Supply 2–4 meaningful choices. The tool owns the appended custom-answer and navigation rows: never author options labelled `Other`, `Type something.`, or `Next`.

For a reserved-label rejection, replace the offending option with a meaningful choice or remove it while retaining 2–4 authored choices, then retry. Custom answers use the tool's automatically appended row.

## Documentation standards

Before editing or reviewing documentation, read `CODING_STANDARDS.md`.

## Parallel reviews

For an authorized two-reviewer fan-out, read the installed `pi-subagents` skill and its `references/review-and-validation.md`. Use their compact `runs.all(...)`, fresh read-only reviewer, saved-output, and native async-completion guidance. Consult the full workflow guide only when a required capability is not covered there.
