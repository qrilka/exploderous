# Coding standards

## Rust types and invariants

- Follow **parse, don't validate**: convert raw input into types that carry validity guarantees through constructors or `TryFrom`. Downstream code should rely on those guarantees rather than repeat the checks.
- Keep fields private when unrestricted construction or mutation could violate invariants. Provide explicit constructors and read-only accessors; any mutation must preserve validity.
- Use newtypes when they enforce useful constraints or prevent mixing distinct domain concepts. Do not wrap primitives without a concrete benefit.
- Distinguish raw input from validated domain values. Check context-dependent relationships when constructing the owning aggregate, and test rejection at the constructor that owns each invariant.

## Interface review

For a new public abstraction, identify the caller requirement that cannot be served by an existing owner or a function.

## Workflow review

For repository automation, review how users discover and activate it, the scope and trigger of each operation, and whether validation exercises the documented usage path.

## Testing review

- Review tests against the design's observable behavioral properties. Require independently justified expectations and identify a plausible incorrect implementation the tests reject; reference agreement and case counts alone do not establish conformance. Verify that a test exercises the production artifact it claims to protect. Checking a separately maintained copy of its values does not establish coverage of their production use.
- For new or expanded parameterized tests, review the purpose of each case dimension and the measured debug runtime. Reconsider unsuitable test scope before adding production complexity solely to accelerate it.
- When an API widens accepted inputs, check the new boundary cases through existing consumers, including consumers whose assumptions were previously guaranteed by a fixed fixture.

### Test placement

For new or substantially rewritten tests, choose placement by responsibility and boundary. Calling public methods alone does not make a test an integration test. Existing suites need not move solely to conform to this placement policy.

| Test responsibility | Default location |
| --- | --- |
| A crate's own domain behavior, including construction, generation, movement, Bomb/Item rules, and NPC decisions | A `#[cfg(test)]` module beside the owning implementation |
| Behavior integrating crates or adapters, such as TUI input/time through core simulation to rendered output | Integration tests under `tests/` |
| External public-API availability or compatibility | Integration tests under `tests/` |

## Documentation review

Describe the current contract for a fresh reader. Name concrete requirements and operations; remove wording that only makes sense as a response to the preceding discussion.

Before editing, choose the owning document using this table. During review, check every added or changed section against that document's responsibility. Also check unchanged capability summaries affected by the delivered behavior, including README and roadmap sections; keep ticket progress in the issue tracker. Link to the owner instead of duplicating its content.

| Document | Responsibility |
| --- | --- |
| README | Repository overview, minimal quickstart, and links to detailed documentation |
| Glossary | Domain terms and their meanings, without implementation details |
| Project plan | Project direction, broad priorities, and delivery sequencing; not slice requirements or per-issue status tracking |
| Spec issue | Desired outcome, scope, cross-cutting requirements, open decisions, and feature-level acceptance gates |
| Implementation ticket | One implementable slice, its local acceptance checks, and dependencies; it may also serve as the spec for small work |
| Design | Current representation, relationships, invariants, interfaces, interaction semantics, and open design questions |
| ADR | Why a consequential decision was made, including alternatives and trade-offs |
| Coding standards | Policies for code and documentation quality and review |

Specs and tickets live in GitHub issues; see `docs/agents/issue-tracker.md` for their lifecycle and linking conventions. Keep the project plan a concise roadmap that points to issues and durable design documents instead of copying their detailed requirements. Do not maintain per-issue status in the plan.

Flag content that belongs to another document's responsibility. A design should remain useful after a delivery slice is complete; next-slice instructions belong in the project plan or issue. Testing implications and correctness properties belong in the design, while feature acceptance gates and tool rollout belong in the spec issue or implementation ticket.

When sources conflict, do not silently let a narrower delivery issue redefine a lasting design rule. Update the design to change the current contract, and record the rationale in an ADR when the choice is consequential; then align the spec/ticket. The plan guides prioritization but does not override any of these contracts. Flag conflicting or duplicated requirements and identify which source should own them. Design sketches should distinguish agreed relationships from illustrative types or unsettled numeric choices.
