# Issue tracker: GitHub

Issues and specs for this repo live as GitHub issues. Use the `gh` CLI for all operations. The project plan records direction and broad priorities; it is not a source of slice requirements or issue status.

## Specs and implementation tickets

- A **spec issue** describes a feature or milestone: desired outcome, scope and non-goals, cross-cutting requirements, unresolved decisions, and feature-level acceptance gates. Create or refine it from the project direction before splitting multi-slice work.
- An **implementation ticket** describes one implementable slice, with its own scope, acceptance checks, relevant design/spec links, and blocking dependencies. Make it self-contained enough to implement without copying the whole spec into it.
- Small work can use one issue as both its spec and implementation ticket. For multi-slice work, use a parent spec issue with child implementation tickets. Link children as GitHub sub-issues where available; otherwise use a task list and a `Part of #<spec>` line. Represent blockers with native issue dependencies when available.
- Keep lasting behavior and invariants in the design, and consequential decision rationale in ADRs. If a spec or ticket changes a lasting rule, update the owning design (and ADR if warranted) before treating the issue as ready. Do not duplicate full design text in issue bodies; link to it and state only the slice-specific requirements.
- The issue tracker is the source of truth for issue scope, acceptance gates, dependencies, and status. Reflect durable changes in the plan only when project direction or priority changes—not to mirror ticket progress.

## Ticket branches

- Before changing files for a ticket, create or reuse its dedicated `issue-<number>-<short-slug>` branch, such as `issue-1-headless-movement`, unless the user specifies another branch.
- Before implementing, reviewing, or editing a ticket, resolve the target from the explicitly named issue or, if none is named, the active ticket branch; then read its live body and comments. Apply “this ticket” changes to that target and keep follow-ups separate unless named. If branch, issue, and requested scope disagree, clarify before editing.
- If authorization or issue state is disputed, compare the session log with GitHub's current state before changing, closing, or recreating an issue. Ask if those records do not resolve the scope or authorization.
- When the user approves behavior that differs from acceptance criteria, update the issue before treating the work as ready for review; record user-confirmed manual acceptance gates in an issue comment.
- Commit subjects describe the change; include ticket numbers only when explicitly requested.
- Once ticket work passes its checks and review, use `/pr` to draft the PR body. Show the complete proposed PR title and body inline in chat before requesting approval; a questionnaire preview or file link alone is insufficient. Get explicit approval before creating the PR; approval to implement or push is not PR approval.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --comments`, filtering comments by `jq` and also fetching labels.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --comment "..."`

Infer the repo from `git remote -v`; `gh` does this automatically when run inside a clone.

## Pull requests as a triage surface

**PRs as a request surface: no.** _(Set to `yes` if this repo treats external PRs as feature requests; `/triage` reads this flag.)_

When set to `yes`, PRs run through the same labels and states as issues, using the `gh pr` equivalents:

- **Read a PR**: `gh pr view <number> --comments` and `gh pr diff <number>` for the diff.
- **List external PRs for triage**: `gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments` then keep only `authorAssociation` of `CONTRIBUTOR`, `FIRST_TIME_CONTRIBUTOR`, or `NONE` (drop `OWNER`/`MEMBER`/`COLLABORATOR`).
- **Comment / label / close**: `gh pr comment`, `gh pr edit --add-label`/`--remove-label`, `gh pr close`.

GitHub shares one number space across issues and PRs, so a bare `#42` may be either: resolve with `gh pr view 42` and fall back to `gh issue view 42`.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments`.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a single issue with **child** issues as tickets.

- **Map**: a single issue labelled `wayfinder:map`, holding the Notes / Decisions-so-far / Fog body. `gh issue create --label wayfinder:map`.
- **Child ticket**: an issue linked to the map as a GitHub sub-issue (`gh api` on the sub-issues endpoint). Where sub-issues aren't enabled, add the child to a task list in the map body and put `Part of #<map>` at the top of the child body. Labels: `wayfinder:<type>` (`research`/`prototype`/`grilling`/`task`). Once claimed, the ticket is assigned to the driving dev.
- **Blocking**: GitHub's **native issue dependencies**, the canonical, UI-visible representation. Add an edge with `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`, where `<blocker-db-id>` is the blocker's numeric **database id** (`gh api repos/<owner>/<repo>/issues/<n> --jq .id`, _not_ the `#number` or `node_id`). GitHub reports `issue_dependencies_summary.blocked_by` (open blockers only, the live gate). Where dependencies aren't available, fall back to a `Blocked by: #<n>, #<n>` line at the top of the child body. A ticket is unblocked when every blocker is closed.
- **Frontier query**: list the map's open children (`gh issue list --state open`, scoped to the map's sub-issues / task list), drop any with an open blocker (`issue_dependencies_summary.blocked_by > 0`, or an open issue in the `Blocked by` line) or an assignee; first in map order wins.
- **Claim**: `gh issue edit <n> --add-assignee @me`, the session's first write.
- **Resolve**: `gh issue comment <n> --body "<answer>"`, then `gh issue close <n>`, then append a context pointer (gist + link) to the map's Decisions-so-far.
