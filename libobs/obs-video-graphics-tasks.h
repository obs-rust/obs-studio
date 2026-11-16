#pragma once

#include <stdbool.h>

struct obs_core;
struct obs_task_info;

/* The graphics thread's task drain (execute_graphics_tasks() in
 * obs-video-graphics-tasks.c), a private adapter, not a new public API. It is
 * Rust in hybrid libobs (obs-core::video_graphics_tasks) and the original C
 * in the C reference (obs-video-graphics-tasks-reference.c). Hidden, not
 * EXPORT.
 *
 * It takes task_mutex, and while the graphics task queue is not empty pops
 * the oldest task, releases the mutex, runs the task and takes the mutex
 * again; then it releases the mutex. A task queued meanwhile runs in the same
 * pass. The Rust reaches the queue only through the accessors, so the mutex
 * and the deque stay C-owned; the C reference reads core directly and
 * ignores them. */
void obs_video_graphics_tasks_execute(struct obs_core *core, void (*lock)(struct obs_core *core),
				      void (*unlock)(struct obs_core *core), bool (*pending)(struct obs_core *core),
				      void (*pop)(struct obs_core *core, struct obs_task_info *info));
