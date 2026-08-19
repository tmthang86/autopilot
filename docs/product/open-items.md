# Open items

Questions with no answer yet, and debt taken on deliberately. An item leaves this file only when it
is answered or paid down — never because it got old.

Bound by [`../CLAUDE.md` §5](../CLAUDE.md): raise a question with no answer, or take on debt, and it
is recorded here in the same commit.

## Unverified guarantees

| Item | Since | What would settle it |
|---|---|---|
| A reboot: that `stop` survives one, and a job left started does not come back | 2026-08-15 | Reboot the machine and observe `autopilot status` before and after |
| Intent binding end to end against the real agent | 2026-08-15 | One issue with a valid `Intent:` line through the throwaway repository, confirming the transcript shows the plan was read before code was written |
| The throttled `rate_limit_event` payload | 2026-08-14 | Observe a real throttle and record the payload verbatim |
| A real three-role run through the Rust runner against a real harness | 2026-08-19 | One issue with a `tier:` label run implement → test → review, verification green, issue closed |

## Open questions

| Question | Raised | Why it matters |
|---|---|---|
| Does `opencode` reach `ollama` and `lmstudio` in practice? | 2026-08-16 | It is the only route to a local-model tier |
| Is `opencode`'s run path usable once its user config is fixed? | 2026-08-19 | The adapter ships unproven until a real run transcript is recorded |

## Accepted debt

| Debt | Taken on | Cost if left |
|---|---|---|
| `wake_budget_usd` is best-effort where the harness reports cost | 2026-08-16 | Only `claude` and `pi` report cost today; `wake_timeout_s` is the enforceable ceiling |
| `autonomy.default` is declared but unread | 2026-08-16 | One autonomy class today; giving it more classes is a design question, not a port |
| The shell runner remains alongside the Rust binary until the cutover | 2026-08-19 | Two implementations overlap; the shell is deleted only after a real run |
