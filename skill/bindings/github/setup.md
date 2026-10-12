## Setup

`SCRIPT ensure-states` creates the six state labels. `gh` refuses to attach a label that does not
exist, and the error arrives at issue creation — the analyst's last step, after all the analysis is
done. That is why `create` creates every label first and attaches second: the domain names its own
priority scale and rule book, so no setup script can have created them in advance. Label creation is
idempotent, so it costs one call and removes the failure mode entirely.

Attribution labels are created on demand by `claim` and `create`, so the set stays exactly as wide as
the runtimes actually in use. Labels are what make attribution *queryable*:
`gh issue list --label "dev:codex"` answers "what is that runtime holding right now" in one call,
where parsing prose for the same answer breaks on any rewording.

**Adding `done` to a repository that already ran without it** leaves closed issues still wearing the
state they were in when someone closed them. They keep showing up in `review` queries. Repair once,
one pass per state — deliberately
plainer than a single clever query, because this runs against real history and you want to be able
to read it before you trust it:

```bash
for s in analysis ready in-progress review blocked; do
  gh issue list --state closed --label "status:$s" --json number --jq '.[].number' |
  while read -r n; do
    gh issue edit "$n" --add-label "status:done" --remove-label "status:$s"
  done
done
```

An issue carrying two state labels is handled by the pass for each of them; an issue already on
`status:done` is matched by none of them and left alone.

