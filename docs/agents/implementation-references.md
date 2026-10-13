# Implementation References

Before implementing behavior governed by a project design/ADR or an external protocol, platform, framework, or dependency:

1. Check for and read relevant project design/ADR/specs, plus authoritative documentation for the external behavior and version in use. Use the normative/reference sections to establish the contract; examples and API names are supplemental.
2. Identify the rules that constrain the change, especially lifecycle, state scope, ordering, and edge cases.
3. Record the source section and applicable constraint in the ticket or implementation notes, and turn it into an acceptance check or test.
4. If documentation is missing, ambiguous, or conflicts with the installed version, inspect that version's source and resolve or surface the remaining assumption before choosing behavior.

Completion means each externally defined behavior in scope has a documented source and an acceptance check, with unresolved assumptions made explicit.
