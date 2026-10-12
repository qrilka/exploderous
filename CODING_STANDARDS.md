# Coding standards

## Rust types and invariants

- Follow **parse, don't validate**: convert raw input into types that carry validity guarantees through constructors or `TryFrom`. Downstream code should rely on those guarantees rather than repeat the checks.
- Keep fields private when unrestricted construction or mutation could violate invariants. Provide explicit constructors and read-only accessors; any mutation must preserve validity.
- Use newtypes when they enforce useful constraints or prevent mixing distinct domain concepts. Do not wrap primitives without a concrete benefit.
- Distinguish raw input from validated domain values. Check context-dependent relationships when constructing the owning aggregate, and test rejection at the constructor that owns each invariant.

## Documentation review

Review documentation by responsibility:

| Document | Responsibility |
| --- | --- |
| Glossary | Domain terms and their meanings, without implementation details |
| ADR | Why a consequential decision was made, including alternatives and trade-offs |
| Design | Representation, relationships, invariants, interfaces, interaction semantics, and open design questions |
| Plan, slice spec, or ticket | Delivery sequence, implementation scope, tooling setup, and acceptance gates |

Flag content that belongs to another document's responsibility. In particular, a design should remain useful after a delivery slice is complete; next-slice instructions belong in the plan or spec. Testing implications and correctness properties belong in the design, while tool rollout and execution gates belong in the plan or spec.

Keep each rule or decision authoritative in one place and link to it elsewhere. Flag conflicting or duplicated requirements across documents, and identify which source should own them. Design sketches should distinguish agreed relationships from illustrative types or unsettled numeric choices.
