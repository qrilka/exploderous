# Questionnaire input constraints and upstream options

Checked on 2026-10-12: upstream `juicesharp/rpiv-mono` documentation, source, issues, and pull requests for retrospective improvement #5. These are research findings, not authorization to modify installed tools or project configuration.

## Existing behavior

The [tool schema documentation](https://github.com/juicesharp/rpiv-mono/blob/main/packages/rpiv-ask-user-question/docs/tool-schema.md#reserved-option-labels) reserves the authored option labels `Other`, `Type something.`, and `Next`. The UI supplies its own custom-answer row and, for multi-select, its advance/submit control. `Other` is rejected so the built-in custom-answer row remains the sole fallback. This does not prevent users from giving custom answers.

The same documentation and [parameter schema](https://github.com/juicesharp/rpiv-mono/blob/main/packages/rpiv-ask-user-question/tool/types.ts) require a header of at most 16 characters and 2–4 authored choices. The [runtime validator](https://github.com/juicesharp/rpiv-mono/blob/main/packages/rpiv-ask-user-question/tool/validate-questionnaire.ts) reports `Error: Option label is reserved (Other, Type something., Next)` without a repair instruction.

## Proposed improvement versus project-only guidance

The proposed implementation had two parts: optional headers with generated Q1–Q4 defaults, and a reserved-label error explaining how to repair the call. Neither adds a new custom-answer capability; that already exists.

Project instructions can instead require Q1–Q4 headers, forbid authored reserved labels, and explain how to correct rejected calls. They do not change the schema or error messages. Those constraints are already documented upstream; the observed failures were agent failures to follow them, not missing documentation.

## Related upstream work

- [Issue #95, closed](https://github.com/juicesharp/rpiv-mono/issues/95): made `Type something.` the universal fallback on every question, including previews and multi-select. The maintainer explicitly rejected making that behavior configurable. The implementation is confirmed in the issue comments.
- [Issue #16, closed](https://github.com/juicesharp/rpiv-mono/issues/16): reports repeated overlong option-label failures despite a schema limit. The maintainer strengthened the field description to lead with the hard limit. This is analogous to our header-length failures, not an automatic-header feature.
- [Issue #169, closed](https://github.com/juicesharp/rpiv-mono/issues/169): added configurable tool-description text. Current [configuration docs](https://github.com/juicesharp/rpiv-mono/blob/main/packages/rpiv-ask-user-question/docs/configuration.md) support `guidance.description`, `guidance.promptSnippet`, and `guidance.promptGuidelines`. These change model-facing instructions, not runtime constraints. The ordinary configuration location is user-wide, with an XDG environment override; the docs do not describe a project-local config lookup.
- [PR #289, open and unmerged when checked](https://github.com/juicesharp/rpiv-mono/pull/289): proposes configurable `maxQuestions`, `maxOptions`, `maxHeaderLength`, and `maxLabelLength`. Its proposed schema still requires a header; this is configurable limits, not generated headers. PR head at inspection: `4d052d65cb94f97358ad0820e80100a99c3f90de`.

Targeted searches across issues and PRs for `header optional`, `header default`, `header generate`, `Q1`, `reserved_label`, and `reserved label` found no dedicated proposal for generated question headers or a reserved-label repair hint. This is a bounded search result, not proof that no discussion exists.
