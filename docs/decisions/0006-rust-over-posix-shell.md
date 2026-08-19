# 6. The runner is rewritten in Rust, over POSIX shell

> **Status:** Accepted · **Date:** 2026-08-19 · Supersedes the language constraint in `CLAUDE.md` §1

## Context

Rule Zero stated the auditability constraint as *POSIX shell only*. The reason it gave is different:
**anything the operator must trust and cannot read is a liability**, and the codebase must be
readable in one sitting. Shell was the means; auditability was the end.

`docs/reference/observed-behaviour.md` is the argument for changing the means. Of the defects that
survived 166 passing shell tests, most were shell semantics rather than logic:

| Defect | Class |
|---|---|
| `x=$(false)` under `set -eu` kills the script even inside an `if` body | shell semantics |
| `jq`'s `//` treats `false` as absent | weak typing across a process boundary |
| Command substitution runs a subshell, discarding the queue cache | shell semantics |
| `git reset --hard` with no argument rewinds to a `HEAD` that has already moved | not shell |

Three of those four classes are unrepresentable in Rust: `Result` must be handled, `Option<bool>`
distinguishes `false` from absent, and no construct silently discards state on return.

## Decision

The runner is rewritten in Rust. `git` and `gh` remain child processes, spawned through one timed
helper. The dependency tree is four crates — `serde`, `serde_json`, `time`, `libc` — so an operator
can still enumerate everything the binary trusts. No module exceeds ~200 lines, the successor to the
~150-line shell rule.

The four invariants of `CLAUDE.md` §2 survive, reworded and extended by the multi-harness design's
invariant table (no unverified work reaches `autopilot/main`; `ProviderUnavailable` never consumes a
retry; state, the journal, and paused task branches round-trip; missing or malformed verdicts are
never passes; the agent never works on `autopilot/main`; no harness name appears outside
`src/harness/`; the runner never references a skill or the dashboard).

## Consequences

### Good

- The failure classes that cost the most are unrepresentable rather than merely avoided.
- The harness abstraction, the tier ladder, and the three-role pipeline are expressible as typed
  contracts instead of string-bearing shell functions.
- The single spawn door (a helper that takes a timeout and kills the process group) is grep-checkable
  in a way "remember to pass a timeout" never was.

### Bad

- A second toolchain (`cargo`, `rustc`) is now a build dependency, and the deployed artifact is a
  compiled binary rather than an editable script. The operator can still read the source; they can no
  longer fix a deployed copy in place.
- `panic = "abort"` in release turns a reachable panic into a wake that leaves nothing behind, so the
  no-`unwrap`/`expect` rule matters more, not less.
- The shell suite becomes a conformance/reference artifact during the cutover and is deleted only
  after a real three-role run — a transition window with two implementations.

### Neutral

- `git` and `gh` stay child processes, so every finding in `observed-behaviour.md` about their actual
  behaviour remains valid.
