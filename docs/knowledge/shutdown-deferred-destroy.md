---
type: concept
title: A source's destroy runs later, on another thread
description: "obs_source_destroy queues its work on the destruction task thread, so obs_shutdown can free what a queued destroy still uses; wait in a loop until the queue is empty."
sources: ["#3431", "#3441", "#3442", "#3364", "#3458"]
status: active
---

# A source's destroy runs later, on another thread

## The trap

`obs_source_destroy()` does not finish the destroy. It queues
`obs_source_destroy_defer()` on `obs->destruction_task_thread`
(`libobs/obs-source.c.h`). A scene's deferred destroy then releases the sources in
its items, so it can queue more work and drop more references, on that thread,
while the main thread goes on.

`os_task_queue_wait()` does not mean "the queue is empty". It returns true if
tasks ran before its marker (`wait` in `rust/obs-util/src/task.rs`). A task
that ran can have queued another.

## How it showed up

#3431 made `obs_shutdown()` give back the control block of each source left alive.
A scene's deferred destroy could still release one of those sources, so that
release wrote to a freed block (#3441). It shipped in a beta. The ASan report depends
on timing: through the harness, with load on 4 cores, 26 of 30 runs showed it.

## What to do instead

- Before you free something a queued destroy can touch, run the queue until it is
  empty: `while (os_task_queue_wait(obs->destruction_task_thread)) ;`.
- If a destroy that starts later can still drop the last reference, hold the
  control block yourself across it, as `obs_free_data()` in `libobs/obs.c.h` does.
- To test an order like this, force the race on every run. Do not depend on load:
  `test_scene_items_are_released_not_left_alive` in
  `test/cmocka/test_shutdown_canvas_weak_ref.c` delays the first item removal by
  200 ms.

#3458 tracks the hazards that are left when the host leaks a scene at shutdown.

## Checks

`test_shutdown_canvas_weak_ref` (Tier 2, all libobs backends) and the asan job.
