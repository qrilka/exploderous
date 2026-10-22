# Manual core mutation testing

Mutation testing changes one expression or function in a temporary source copy, then checks whether the existing tests reject it. This workflow mutates only `exploderous-core` and runs only its ordinary core test targets with `--all-targets --locked`. It does not use TUI tests to judge core mutations.

## Local command

Follow the [development setup](../README.md#development-setup). The runner additionally requires Linux with `/proc`, Python 3.11+, Git, **cargo-mutants 27.1.0** and **ast-grep 0.51.0** on `PATH`. It checks both versions and never installs or updates local tools. If installation is needed, explicitly install the pinned versions into a directory you control; there is no automatic user-wide setup.

From any directory, invoke the script by its path:

```sh
python3 scripts/run_mutation_tests.py
```

The example above assumes the repository root. The script always evaluates the checkout containing it, not the caller's working directory. Configure its time limit, concurrency, and report directory with:

```sh
python3 scripts/run_mutation_tests.py \
  --budget 600 --jobs 2 --output /tmp/exploderous-mutations-review
```

### Command-line options

| Argument | Default | Meaning |
| --- | --- | --- |
| `--budget SECONDS` | `600` (10 minutes) | Total **wall-clock time limit in seconds** for the run, including setup, inventory, copying, baseline build/tests, and mutation evaluation. It is not CPU time or a separate allowance for each mutation. |
| `--jobs N` | `2` | Maximum number of **mutation evaluations running concurrently**. Each worker builds and tests a different mutation in its own source copy. |
| `--output PATH` | A new system temporary directory | Where to retain summaries and raw evidence. The directory must be outside the checkout and must not already exist. |

`--budget` must be a finite positive number; fractional seconds are accepted. `--jobs` must be a positive integer. With `--jobs 1`, mutations are evaluated one at a time; with `--jobs 2`, up to two are evaluated at once. The unmodified baseline runs once before mutation evaluation starts. Worker count does **not** limit compiler parallelism or threads within each Rust test run; more workers can increase CPU and memory usage and do not guarantee a faster scan.

The run can finish before its time limit. If the budget expires first, active evaluations are stopped and partial results are preserved; a complete scan is not guaranteed. Cleanup/report writing may briefly continue after the deadline as described below.

`--output` must name a **new directory outside the checkout**; existing directories are rejected, so earlier evidence cannot be rotated away. Without it, the runner creates a new directory in the system temporary directory and prints its path immediately. Reports survive the command, but temporary storage may later be cleaned by the OS: copy evidence you want to retain to persistent storage outside the checkout.

The runner inventories generated mutations before the shuffled scan, records the repository's explicit scope exclusions below, uses copied-tree isolation without copying `target/`, and explicitly runs the baseline. It ignores `.cargo/mutants.toml` (`--no-config`) rather than silently applying local exclusions; approved repository exclusions are passed explicitly with `--exclude-re`. Ordinary cargo-mutants eligibility defaults still apply. The tool derives its per-mutation test timeout from the baseline; this is separate from the runner's total elapsed budget.

Budget expiry, Ctrl-C, or SIGTERM stops dispatch and kills launched build/test processes across the execution session, including Cargo's separate process groups. A short cleanup interval (normally under five seconds) may follow the deadline to terminate/reap processes and let the supervisor remove its scratch copy; it does not grant more mutation evaluation time. No restart or budget extension is automatic. Linux session tracking covers the Cargo/build/test tree, not intentionally detached daemons that start a new session. SIGKILL, a hard host shutdown, or uninterruptible kernel I/O cannot guarantee cleanup or complete reports; disclose missing evidence rather than treating it as a pass.

## Scope exclusions

The shared local/manual-CI runner excludes exactly **17 reviewed mutations**: seven RNG scope-policy exclusions and ten valid-state equivalents. They remain separate from evaluated outcomes; an exclusion is never a catch.

### RNG scope policy — seven mutations

The seven mutations in `random_choice` are specified in [#38](https://github.com/qrilka/exploderous/issues/38):

| Change | Count |
| --- | ---: |
| Rejection threshold `%` → `/` | 1 |
| Mixer shifts `>>` → `<<` | 3 |
| Second mixer XOR (`value ^ (value >> 27)`) → OR/AND | 2 |
| Acceptance `>=` → `<` (known mutant nontermination) | 1 |

This is a policy decision not to pursue these generator-quality and mutant-termination findings, **not evidence that they are caught, equivalent, or harmless**. Existing fixed-seed gameplay-policy tests and every other RNG mutation remain in scope, including the return-value `%` → `/` mutation. The production generator is unchanged; no tighter probability bands or exact-sequence tests are required merely to reject these mutations.

### Valid-state equivalence — ten mutations

These exclusions were individually reviewed under [#36](https://github.com/qrilka/exploderous/issues/36#issuecomment-6318365597) and [#37](https://github.com/qrilka/exploderous/issues/37#issuecomment-6325963319), then approved for exclusion under [#33](https://github.com/qrilka/exploderous/issues/33#issuecomment-6326522128). Each argument concerns one mutation at a time, not combinations of mutations.

| Owner and expression | Exact changes | Count | Equivalence argument |
| --- | --- | ---: | --- |
| `Motion::boundary_times`: first boundary index `... + 1` | `+` → `-`, `*` (M249/M250) | 2 | Retains all required boundary events; added samples inspect actual paths or already-checked endpoints. |
| Same function: `start % WORLD_UNITS_PER_CELL == 0` | `%` → `/`, `+`; `==` → `!=` (M252/M253/M251) | 3 | Changes redundant segment-start samples. Negative edge starts are already in the ordinary boundary loop; incoming bend endpoints and tick start remain checked. |
| `Motion::contacts`, only inside `times.dedup_by`: `a * d == c * b` | Left `*` → `/`; right `*` → `/` (M270/M272) | 2 | May retain duplicate times, but cannot delete distinct required events under the current sorted, nonnegative tick-time constraints and callback argument order. |
| Same deduplication callback: left `a * d` | `*` → `+` (M269) | 1 | Equivalent only for reachable current movement states: Buroru boundary events occur at tick endpoints and interior player events share a speed denominator. Not a general rational-equality replacement. |
| `Game::blast_coverage`: `axis_length - 1` | `-` → `+`, `/` | 2 | Only loosens the iteration allowance; the unchanged neighbor check still stops at the true edge. Both variants retain a finite Arena-derived work bound, and `+ 1` fits validated dimensions. |

Reconsider boundary exclusions if actual paths, bend/endpoint handling or Cell-boundary semantics change. Reconsider deduplication exclusions if time ranges, ordering or callback argument semantics change. **M269 additionally requires centred Buroru starts, canonical Footprints, eight-unit enemy movement and the current player-event denominators**; review it when any of those assumptions change. Reconsider ray exclusions if dimension limits or ray/neighbor semantics change. Surviving tests alone do not establish these equivalence arguments.

### Structural resolution and reporting

`RULES` in `scripts/mutation_exclusions.py` pairs structural Rust expressions and owning functions/files with the original operators, exact approved replacements and rationale. The runner uses ast-grep inside the nearest named function; contact products must also be inside `times.dedup_by`, leaving sort and midpoint arithmetic eligible. Targets live in `crates/core/src/game.rs` and `crates/core/src/game/simulation.rs`. It joins current AST ranges with the generated mutation inventory, resolving each approved replacement to exactly one mutation, then writes its current identity, `kind` (`rng-policy` or `equivalent`) and individual `reason` to `excluded.json`. The readable summary lists the seven RNG policy exclusions separately from the ten equivalents, including their reasons.

Fully anchored, escaped filters pass those resolved names to cargo-mutants; there are no stored source coordinates. Scan inventory, exclusion metadata and outcomes must reconcile with the approved policy and remaining eligible set; excluded results cannot silently become catches. Local and manual CI use this same path despite `--no-config`. Nearby replacements, other functions/files, sorting, midpoint sampling and unapproved RNG changes stay eligible.

Line movement, whitespace, comments and formatting do not require policy edits. If a target expression is missing or ambiguous, or its approved mutation cannot be uniquely resolved, setup fails **before baseline or mutation evaluation**. Review the policy when the target expressions, owning function/file or rationale assumptions change; do not widen filters to a whole function to avoid that review. Recorded AST matches and generated inventory remain available after setup failure, but are not evidence of a completed assessment. Filtering never rewrites results from earlier scans.

## Reading results

Open `summary.md` first. It contains the baseline result, completion status, elapsed time, exit status, reconciled generated/excluded/eligible execution counts, separate RNG-policy and valid-state-equivalence exclusion lists with individual reasons, and these evaluation outcomes:

| Outcome | Meaning | Interpretation |
| --- | --- | --- |
| Caught | Mutated code compiled; tests failed. | Evidence that the tests detect this change, not proof of overall correctness. |
| Surviving (`missed`) | Mutated code compiled; tests passed. | A possible test gap, or an equivalent/nonessential change. Review required behavior before proposing a test or exclusion. |
| Unviable | The mutation did not compile. | Inconclusive about test effectiveness; not a build failure in the original code. |
| Timed out | The mutation's build/test evaluation exceeded its limit. | Unresolved; never automatically counted as caught or surviving. |

A **completed evaluation** has a terminal outcome, including a timeout. **Untested** means an eligible mutation never started; **interrupted** means started without a terminal outcome. **Generated = excluded + eligible; eligible = completed + untested + interrupted.** Excluded mutations were not evaluated and are not any of the four outcomes above. Each mutation scenario log is evidence that evaluation started; persisted outcomes identify completed evaluations. Counts become unavailable if a report is malformed rather than being guessed. Completion refers to all eligible mutations, not excluded ones. Completing every eligible evaluation does not mean a clean mutation result, and an empty eligible inventory is not accepted as a successful assessment.

The runner preserves cargo-mutants' nonzero exit codes, notably **2** for survivors, **3** for mutation timeouts, and **4** for baseline failure/timeout. It uses **124** for the total budget, **130/143** for interruption by SIGINT/SIGTERM, and a nonzero status for setup, missing evidence, incomplete execution, or parsing errors. Timeouts and survivors remain visibly non-successful. Artifact/summary publication does not turn those outcomes green.

The output directory includes:

- `summary.md` and `summary.json`: readable and machine-readable execution summaries.
- `metadata.json`: source revision, actual versions, command arguments, budget/concurrency, relevant environment/resources, timestamps, and process-cleanup records.
- `checkout.txt` and `source.patch`: checkout state and tracked differences from `HEAD`. A dirty checkout is allowed and explicitly recorded; untracked source contents are not captured by the patch, so preserve them separately if needed for reproduction.
- `inventory.json`: the complete generated inventory before repository policy exclusions, including names, source spans, and diffs.
- `ast-matches.json`: ast-grep's structural matches, source ranges and rule identities; `metadata.json` records the actual inline rules and tool version.
- `excluded.json`: the 17 resolved current mutation records with their rationale category and individual reason, separate from evaluation outcomes.
- `console.log` and setup-command output/stderr files.
- `mutants.out/`: the tool's shuffled eligible inventory, baseline and mutation logs, diffs, and incremental outcomes/category lists.

On early setup failure, inventory, baseline, or outcomes may not exist. On forced interruption, `outcomes.json` may be truncated; raw files remain available and parsing failure is reported instead of claiming reconciled counts. Inspect local patches/logs for sensitive data before sharing them.

Authoritative references: [ast-grep structural patterns](https://ast-grep.github.io/guide/pattern-syntax.html), [relational scope](https://ast-grep.github.io/guide/rule-config/relational-rule.html), [zero-based JSON ranges](https://ast-grep.github.io/guide/tools/json.html), [full-name mutation filtering](https://mutants.rs/filter_mutants.html), [outcomes and how to investigate survivors](https://mutants.rs/using-results.html#mutant-outcomes), [timeouts](https://mutants.rs/timeouts.html), [exit codes](https://mutants.rs/exit-codes.html), [copied-tree safety](https://mutants.rs/cautions.html), and [raw reports](https://mutants.rs/mutants-out.html). The runner's interpretation is pinned to 27.1.0; its raw format is not guaranteed stable across tool versions. Linux cleanup uses [session IDs from `/proc/PID/stat`](https://man7.org/linux/man-pages/man5/proc_pid_stat.5.html) because a single process-group kill misses Cargo's separately grouped children.

## Manual CI workflow

Once `.github/workflows/mutation-tests.yml` is on the default branch, open **Actions → Manual core mutation tests → Run workflow**. There are no push, PR, or scheduled mutation scans. Select the revision to assess; the workflow must exist on the selected revision as well. GitHub's [manual trigger documentation](https://docs.github.com/en/actions/how-tos/managing-workflow-runs-and-deployments/managing-workflow-runs/manually-running-a-workflow) describes the default-branch prerequisite.

CI installs cargo-mutants 27.1.0 and ast-grep 0.51.0 through commit-pinned [taiki-e/install-action](https://github.com/taiki-e/install-action#supported-tools), then invokes **the same Python command** with the same 600-second/two-worker defaults. cargo-mutants uses the action's binary manifest; ast-grep uses its cargo-binstall fallback and upstream binary metadata. If binary installation fails, the fallback may compile from source with the lockfile. The mutation step has a 12-minute safety timeout and the job a 30-minute outer timeout, leaving room for installation, cleanup, and upload. Installation has its own 15-minute limit and is outside the runner's mutation budget. These provisional settings are subject to measured CI performance and findings review, not a guarantee based on local workstation timing.

The run summary displays `summary.md`; download **core-mutation-reports** from the run's Artifacts section for the complete available evidence. Artifacts are retained for **7 days**. Upload and summary steps run even after an earlier failure and fail if required output is missing; neither masks the execution step's failure. A cancelled job or lost runner may still prevent publication, so a missing artifact is not evidence of a successful scan.

## Lightweight validation

```sh
python3 scripts/test_mutation_runner.py
```

This requires ast-grep 0.51.0 and exercises the public CLI in a temporary clone with real structural parsing, a small Rust source fixture, controlled external Cargo/report fixtures and real process cleanup. It does not execute mutation tests or change gameplay tests. Ordinary CI installs the pinned parser and runs these checks, not a mutation scan. They cover all 17 exclusion identities and rationale categories; nearby eligible operators, sorting/midpoint expressions, functions and files; moved, compactly reformatted and commented source; non-ASCII character columns; missing/ambiguous RNG, contact and Blast targets; missing inventory targets; parser version/failure and ignored-filter cases; outcome/status propagation, baseline failure/timeout, partial counts, malformed/missing evidence, unsafe arguments/output locations, elapsed deadlines, and cancellation with children in separate process groups. They do not validate cargo-mutants itself or GitHub artifact transport; those require a real local scan and a manual CI run, respectively.
