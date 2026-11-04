---
type: concept
title: A PipeWire loop stopped right after its start can hang
description: "pw_thread_loop_stop() can wait forever when the loop thread has not entered the loop yet (PipeWire 1.6.2). Wait for an event source's callback first (thread-loop.start-signal is ignored before PipeWire 0.3.77); a blocking pw_loop_invoke does not help."
sources: ["#3577", "#3596", "#2431"]
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

Wait until the loop thread runs a callback of yours from inside the loop. Add an
event source before the start, signal it after, and wait under the lock for the
callback, which only runs on the loop thread once it has entered the loop:

```c
struct entered {
	struct pw_thread_loop *thread_loop;
	bool entered;
};

static void on_entered(void *data, uint64_t count)
{
	struct entered *e = data;

	e->entered = true; /* loop thread, loop lock held */
	pw_thread_loop_signal(e->thread_loop, false);
}

struct entered e = {.thread_loop = thread_loop};
struct pw_loop *loop = pw_thread_loop_get_loop(thread_loop);
struct spa_source *event = pw_loop_add_event(loop, on_entered, &e);

pw_thread_loop_lock(thread_loop);
pw_thread_loop_start(thread_loop);
pw_loop_signal_event(loop, event);
while (!e.entered)
	pw_thread_loop_wait(thread_loop);
pw_loop_destroy_source(loop, event);
pw_thread_loop_unlock(thread_loop);
```

Take the lock before the start, or the signal can come before the wait. The
loop thread sets `impl->thread` before it dispatches the event, and the lock it
hands back orders that before a later `pw_thread_loop_stop()`.

`thread-loop.start-signal` (the loop thread signals once it has entered) does
the same, but only from PipeWire 0.3.77: older versions ignore the property, so
an unconditional wait for it never returns (#3596; Debian 12 ships 0.3.65).
Use it only where the build already requires 0.3.77.

These do not help:

- A blocking `pw_loop_invoke` first. It takes the same path: it can run on the
  caller and prove nothing (1 hang in 3744 runs).
- A sleep first. 50 ms gave 0 hangs in 9600 runs, but it only narrows the window.

## Where else

`obs_pipewire_connect_fd()` in `plugins/linux-pipewire/pipewire.c.h` has the same
order on its `fail:` path, when `pw_context_connect_fd()` fails right after the
start. Its normal path waits for a sync first, so it is not affected.
