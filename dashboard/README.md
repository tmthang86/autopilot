# The autopilot dashboard

A read-only view over the run journals. It shows what is running now, what died without saying so,
what each tier is costing, and what is paused waiting for a person — across every project on the
machine.

**Read-only, always.** No handler mutates anything and no route accepts `POST`. Acting on what it
shows happens through the skills or by hand. The runner never knows it exists.

## Build and run

```sh
cd dashboard && go build -o /tmp/apdash .
/tmp/apdash --addr 127.0.0.1:8787
```

It binds `127.0.0.1` by default and refuses any other address unless `--addr` is passed explicitly.
It discovers projects from `~/.local/share/autopilot/jobs/*.plist`.

## The honest fallback

The dashboard's headline feature — an orphaned role — is one query, and this page must never become
load-bearing:

```sh
jq -s '
  (map(select(.event == "role_start")) | length) as $starts
  | (map(select(.event == "role_end")) | length) as $ends
  | "starts \($starts) ends \($ends)"
' .autopilot/journal.jsonl
```

`autopilot status` reports the same orphans from the CLI. If the dashboard is not running, nothing
is lost.

## What it deliberately cannot do

- Remove a `STOP` file, answer a blocked issue, or merge anything.
- Write to any journal or repository.
- Serve beyond localhost without an explicit flag.
- Replace `autopilot status`, which remains the primary and scriptable route.
