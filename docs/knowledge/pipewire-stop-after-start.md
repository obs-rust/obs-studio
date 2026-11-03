---
type: concept
title: A PipeWire loop stopped right after its start can hang
description: "pw_thread_loop_stop() can wait forever when the loop thread has not entered the loop yet (PipeWire 1.6.2). Wait for thread-loop.start-signal first; a blocking pw_loop_invoke does not help."
sources: ["#3577", "#2431"]
status: active
stale_after: "a PipeWire release that fixes loop_queue_invoke's check of impl->thread"
---

# A PipeWire loop stopped right after its start can hang

## The trap

`pw_thread_loop_start()` followed at once by `pw_thread_loop_stop()` can hang.
From PipeWire 1.6.2's source (`spa/plugins/support/loop.c`, `thread-loop.c`):

1. `pw_thread_loop_stop()` calls `pw_loop_invoke(do_stop)`.
2. `loop_queue_invoke()` reads `impl->thread` with no lock. If it is still 0, it
   treats the caller as the loop thread, so `do_stop` runs on the caller, with no
   wakeup.
3. The loop thread enters the loop, sees `running` still true, and goes into
   `epoll_wait`.
4. The caller sets `running = false` and waits in `pthread_join`. Nothing wakes
   the loop thread.

## How it showed up

`test_pipewire_teardown_wait` hit the 10-minute limit in 2 of 13 asan jobs (#3577):
its log ends at the `RUN` line, with exit 124. The window is a few instructions.
Locally it hung about once in 800 asan runs under load, and not in 9600 runs of a
tsan build.

The stack has the main thread in `pthread_join` under `pw_thread_loop_stop`, and
the thread named after the loop in `epoll_wait`. Attaching `gdb` ends the hang,
because the interrupted `epoll_wait` lets the loop thread see the flag. So a hang
that goes away when you attach to it can be this one.

## What to do instead

Create the loop with `thread-loop.start-signal`, and wait for the signal, which
the loop thread sends after it has entered the loop:

```c
struct pw_properties *props = pw_properties_new("thread-loop.start-signal", "true", NULL);
struct pw_thread_loop *loop = pw_thread_loop_new("name", &props->dict);
pw_properties_free(props);

pw_thread_loop_lock(loop);
pw_thread_loop_start(loop);
pw_thread_loop_wait(loop);
pw_thread_loop_unlock(loop);
```

Take the lock before the start, or the signal can come before the wait. The wait
does not return on a PipeWire version that does not have this option.

These do not help:

- A blocking `pw_loop_invoke` first. It takes the same path: it can run on the
  caller and prove nothing (1 hang in 3744 runs).
- A sleep first. 50 ms gave 0 hangs in 9600 runs, but it only narrows the window.

## Where else

`obs_pipewire_connect_fd()` in `plugins/linux-pipewire/pipewire.c.h` has the same
order on its `fail:` path, when `pw_context_connect_fd()` fails right after the
start. Its normal path waits for a sync first, so it is not affected.
