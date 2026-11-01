---
type: concept
title: Dispatch sanitizers on a separate ref
description: A `workflow_dispatch` of `sanitizers.yaml` on a PR branch cancels the run in progress there; dispatch on a separate ref.
sources: ["#3290", "#491"]
status: active
---

# Dispatch sanitizers on a separate ref

## The trap

`.github/workflows/sanitizers.yaml` has

```yaml
concurrency:
  group: sanitizers-${{ github.ref }}
  cancel-in-progress: true
```

One group per ref. Dispatching it on a PR branch (`gh workflow run
sanitizers.yaml --ref <branch> -f tools=asan`) while that branch's run (the
PR's own `asan` pass, or an earlier dispatch) is in progress cancels that run.
Two dispatches for different tools on the same branch also cancel each other:
only the last survives.

## What to do instead

Push the commit you want to measure to its own throwaway ref and dispatch there,
one ref per concurrent run:

```sh
git push origin HEAD:refs/heads/san/<issue>-asan
gh workflow run sanitizers.yaml --ref san/<issue>-asan -f tools=asan
```

Or use `-f tools=all` in a single dispatch. Delete the throwaway branches when
done (#3290 did this for its measurement runs).
