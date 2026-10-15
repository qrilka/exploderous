# Design Interviews

- When requesting approval, state exactly what is being approved and the available continuation options. Treat confirmation as authorizing only the stated scope.
- Before implementing a noticeable change where the request/spec leaves materially different approaches open, show the alternatives and tradeoffs and ask the user which to use. This applies to domain/interface ownership, crate/test layout, representations, dependencies, and workflows—not just style. Skip trivial implementation details and decisions already settled by the user or spec.
- Before asking a behavioral design question, show a reachable scenario under the agreed rules where the choices produce different outcomes. If existing invariants determine the answer, explain the conclusion rather than asking the user to choose.
- Before asking for a technical decision, explain unfamiliar terms in plain language with a concrete example. For representation or geometry choices, show a small diagram or code sketch.
- Before final confirmation of a system design, show the complete model and interactions covering every required domain object, including objects outside the first delivery slice. Distinguish settled relationships from open rules.
- When playtest behavior matches the current contract but conflicts with the intended experience, distinguish implementation correctness from rule suitability. Present relevant rule-change and presentation alternatives before choosing a remedy; align the owning contracts after approval.
