"""Reviewed mutation exclusions resolved from Rust AST spans, never stored coordinates."""

import json
import re


RNG_REASON = (
    "Policy exclusion approved in #38: six known RNG-quality mutations and one "
    "mutant nontermination case are outside this assessment's scope. "
    "Excluded is not caught or proven equivalent; other RNG mutations remain eligible."
)
BOUNDARY_REASON = (
    "M249/M250: retains all required boundary events and only adds actual-path samples. "
    "Reconsider if path sampling or endpoint handling changes; see #36."
)
START_REASON = (
    "M251/M252/M253: changes redundant segment-start samples; the ordinary boundary "
    "loop covers negative edge starts, incoming bends cover their endpoints, and tick "
    "start is always sampled. Reconsider if boundary/path semantics change; see #36."
)
DEDUP_REASON = (
    "M270/M272: may retain duplicate rational times but cannot remove distinct required "
    "events under current sorted nonnegative tick-time constraints. Reconsider if time "
    "ranges, ordering or dedup callback argument semantics change; see #36."
)
BURORU_REASON = (
    "M269: equivalent only for reachable states with centred Buroru starts, canonical "
    "Footprint and 8-unit movement: enemy crossings occur at tick endpoints and interior "
    "player events share a speed denominator. Reconsider if Buroru alignment, speed, "
    "Footprint or event denominators change; not general rational equality. See #36."
)
RAY_REASON = (
    "Loosens an Arena-axis iteration allowance without changing coverage: the unchanged "
    "neighbor check stops at the true edge. Both variants stay spatially bounded; +1 "
    "fits validated dimensions. Reconsider if dimensions or ray/neighbor semantics change; see #37."
)
EXCLUSION_REASON = (
    "Seven RNG scope-policy exclusions approved in #38 and ten valid-state equivalence "
    "exclusions approved under #33 after review in #36/#37. Per-mutation reasons and "
    "reconsideration conditions follow. Excluded is not caught; RNG policy is not an equivalence claim."
)
# Owner, structural expression, original operator, exact replacements, rationale.
RULES = {
    "threshold": ("random_choice", "count.wrapping_neg() % count", "%", ("/",), RNG_REASON),
    "shift30": ("random_choice", "value >> 30", ">>", ("<<",), RNG_REASON),
    "xor27": ("random_choice", "value ^ (value >> 27)", "^", ("|", "&"), RNG_REASON),
    "shift27": ("random_choice", "value >> 27", ">>", ("<<",), RNG_REASON),
    "shift31": ("random_choice", "value >> 31", ">>", ("<<",), RNG_REASON),
    "acceptance": ("random_choice", "value >= threshold", ">=", ("<",), RNG_REASON),
    "boundary-first": ("Motion::boundary_times",
                       "start.min(end).div_euclid(WORLD_UNITS_PER_CELL) + 1", "+", ("-", "*"), BOUNDARY_REASON),
    "boundary-remainder": ("Motion::boundary_times", "start % WORLD_UNITS_PER_CELL", "%", ("/", "+"), START_REASON),
    "boundary-edge": ("Motion::boundary_times", "start % WORLD_UNITS_PER_CELL == 0", "==", ("!=",), START_REASON),
    "dedup-left-sum": ("Motion::contacts", "a * d", "*", ("+",), BURORU_REASON),
    "dedup-left-div": ("Motion::contacts", "a * d", "*", ("/",), DEDUP_REASON),
    "dedup-right-div": ("Motion::contacts", "c * b", "*", ("/",), DEDUP_REASON),
    "ray-limit": ("Game::blast_coverage", "axis_length - 1", "-", ("+", "/"), RAY_REASON),
}
FILES = {
    "random_choice": "crates/core/src/game.rs",
    "Motion::boundary_times": "crates/core/src/game.rs",
    "Motion::contacts": "crates/core/src/game.rs",
    "Game::blast_coverage": "crates/core/src/game/simulation.rs",
}


def ast_rules():
    rules = []
    for key, (function, pattern, _, _, _) in RULES.items():
        scope = {"inside": {
            "kind": "function_item", "has": {"field": "name", "regex": f"^{re.escape(function.split('::')[-1])}$"},
            "stopBy": {"kind": "function_item"},
        }}
        constraints = [{"pattern": pattern}, scope]
        if function == "Motion::contacts":
            # The same products occur in sorting and midpoint sampling: do not exclude those.
            constraints.append({"inside": {
                "pattern": "times.dedup_by($$$ARGS)", "stopBy": {"kind": "function_item"},
            }})
        rules.append(json.dumps({"id": key, "language": "Rust", "severity": "info",
                                 "rule": {"all": constraints}}))
    return "\n---\n".join(rules)


def select_exclusions(output):
    inventory = json.loads((output / "inventory.json").read_text())
    matches = json.loads((output / "ast-matches.json").read_text())
    excluded = []
    for rule, (function, _, operator, replacements, reason) in RULES.items():
        nodes = [m for m in matches if m["ruleId"] == rule and m["file"] == FILES[function]]
        if len(nodes) != 1:
            raise ValueError(f"exclusion source drift: expected one AST match for {rule}, got {len(nodes)}")
        node = nodes[0]
        # Both tools use character columns; ast-grep is zero-based, cargo-mutants one-based.
        start, end = [tuple(node["range"][key][axis] + 1 for axis in ("line", "column"))
                      for key in ("start", "end")]
        for replacement in replacements:
            candidates = [m for m in inventory if m.get("genre") == "BinaryOperator"
                          and m["file"] == FILES[function]
                          and m["name"].endswith(f": replace {operator} with {replacement} in {function}")
                          and start <= (m["span"]["start"]["line"], m["span"]["start"]["column"])
                          < (m["span"]["end"]["line"], m["span"]["end"]["column"]) <= end]
            if len(candidates) != 1:
                raise ValueError(f"exclusion source drift: expected one mutation for {rule} -> {replacement}, got {len(candidates)}")
            excluded.append({**candidates[0], "kind": "rng-policy" if function == "random_choice" else "equivalent",
                             "reason": reason})
    return excluded
