# Autopilot documentation

Docs-as-code: everything here is Markdown, versioned with the code, and updated in the same commit
as the change it describes. The binding sync rules live in [`../CLAUDE.md` §5](../CLAUDE.md).

Three standards are combined:

- **[arc42](https://arc42.org)** — architecture documentation (`design/`, `decisions/`)
- **[Diátaxis](https://diataxis.fr)** — split by need (`guides/`, `reference/`)
- **[Nygard ADRs](https://adr.github.io)** — decision records (`decisions/`)

## Start here

| If you want to… | Read |
|---|---|
| Know the rules of this project | [../CLAUDE.md](../CLAUDE.md) |
| **Learn what the CLIs actually do** | [reference/observed-behaviour.md](reference/observed-behaviour.md) — **highest priority** |
| Install and run the loop | [guides/install.md](guides/install.md) |
| Know why something is built this way | [decisions/](decisions/) |

## Status

Documents are written when the work they describe begins, not speculatively. An empty stub is worse
than an honest gap.

| Document | Status |
|---|---|
| [reference/observed-behaviour.md](reference/observed-behaviour.md) | ✅ Current — seven defects and the 2026-08-19 harness measurements recorded |
| [guides/install.md](guides/install.md) | ✅ Current |
| [guides/cutover-verification.md](guides/cutover-verification.md) | ✅ Current — runbook for the six needs-human items |
| [decisions/0001-one-task-per-wake-over-persistent-daemon.md](decisions/0001-one-task-per-wake-over-persistent-daemon.md) | ✅ Accepted |
| [decisions/0002-off-until-explicitly-started.md](decisions/0002-off-until-explicitly-started.md) | ✅ Accepted |
| [decisions/0003-plist-lives-with-the-project.md](decisions/0003-plist-lives-with-the-project.md) | ⚠️ Superseded by ADR-0004 |
| [decisions/0004-job-files-live-on-the-internal-disk.md](decisions/0004-job-files-live-on-the-internal-disk.md) | ✅ Accepted |
| [decisions/0005-intent-binding.md](decisions/0005-intent-binding.md) | ✅ Accepted |
| [decisions/0006-rust-over-posix-shell.md](decisions/0006-rust-over-posix-shell.md) | ✅ Accepted |
| [design/2026-08-15-skill-layer-design.md](design/2026-08-15-skill-layer-design.md) | ✅ Approved — delivered |
| [design/2026-08-16-multi-harness-role-pipeline-design.md](design/2026-08-16-multi-harness-role-pipeline-design.md) | ✅ Approved — amended 2026-08-18, plans being implemented |
| [plans/2026-08-14-runner-implementation.md](plans/2026-08-14-runner-implementation.md) | ✅ Delivered |
| [plans/2026-08-15-intent-binding.md](plans/2026-08-15-intent-binding.md) | ✅ Delivered — end-to-end run still open |
| [plans/2026-08-15-plugin-foundations.md](plans/2026-08-15-plugin-foundations.md) | ✅ Delivered |
| [plans/2026-08-15-project-bootstrap.md](plans/2026-08-15-project-bootstrap.md) | ⚠️ Superseded by the skill-layer design |
| [plans/2026-08-15-skill-layer.md](plans/2026-08-15-skill-layer.md) | ✅ Delivered |
| [plans/2026-08-16-rust-runner.md](plans/2026-08-16-rust-runner.md) | 📝 Implemented — real run and cutover pending |
| [plans/2026-08-16-deliver-preflight.md](plans/2026-08-16-deliver-preflight.md) | 📝 Implemented — never run against the machine as-is |
| [plans/2026-08-16-dashboard.md](plans/2026-08-16-dashboard.md) | 📝 Implemented — exercised against fixtures only, never a real running process |
| [plans/2026-08-16-planning-skill.md](plans/2026-08-16-planning-skill.md) | 📝 Implemented — end-to-end run pending |
| [product/open-items.md](product/open-items.md) | ✅ Current |

## Layout

```
docs/
├── README.md      this map, and the status of every document
├── product/       open questions and accepted debt
├── design/        validated designs, written before the plans that implement them
├── decisions/     ADRs — why it is built this way
├── reference/     what the tools actually do
├── guides/        how to install, configure, tune, troubleshoot
└── plans/         what we are about to do, written before the code
```
