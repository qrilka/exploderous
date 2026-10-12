# Coding standards

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
