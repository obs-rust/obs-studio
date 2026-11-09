/* The unchanged extracted drain and the real production adapter. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include "obs-internal.h"
#include "video_graphics_tasks_fields.inc"

static _Thread_local struct obs_core *graphics_tasks_obs;
#define obs graphics_tasks_obs
#include "video-graphics-tasks.inc"
#undef obs

struct graphics_tasks_fixture;
struct graphics_tasks_item {
	struct graphics_tasks_fixture *fixture;
	uint32_t id;
	uint8_t flags;
};
struct graphics_tasks_fixture {
	struct obs_core core;
	struct graphics_tasks_item items[64];
	uint32_t *trace;
	size_t count;
	bool unlocked;
};

static void graphics_tasks_callback(void *param)
{
	struct graphics_tasks_item *item = param;
	struct graphics_tasks_fixture *fixture = item->fixture;
	struct obs_core_video *video = &fixture->core.video;
	fixture->trace[fixture->count++] = item->id;
	if (pthread_mutex_trylock(&video->task_mutex) != 0) {
		fixture->unlocked = false;
		return;
	}
	pthread_mutex_unlock(&video->task_mutex);
	if (item->flags & 1) {
		/* Same-thread public dispatch is inline, not a queue insertion. */
		struct graphics_tasks_item nested = {fixture, item->id + 1000, 0};
		graphics_tasks_callback(&nested);
	}
	if (item->flags & 2) {
		struct graphics_tasks_item *child = &fixture->items[32 + item->id];
		*child = (struct graphics_tasks_item){fixture, item->id + 2000, 0};
		struct obs_task_info info = {graphics_tasks_callback, child};
		pthread_mutex_lock(&video->task_mutex);
		deque_push_back(&video->tasks, &info, sizeof(info));
		pthread_mutex_unlock(&video->task_mutex);
	}
}

/* flags has at most 32 items; trace has room for 96 callback ids.
 * state: callback count, remaining bytes, callbacks unlocked, final unlocked. */
void oracle_graphics_tasks_run(void (*drain)(void *), const uint8_t *flags, size_t count, uint32_t *trace,
			       size_t *state);
void oracle_graphics_tasks_run(void (*drain)(void *), const uint8_t *flags, size_t count, uint32_t *trace,
			       size_t *state)
{
	struct graphics_tasks_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture || count > 32)
		abort();
	fixture->trace = trace;
	fixture->unlocked = true;
	struct obs_core_video *video = &fixture->core.video;
	if (pthread_mutex_init(&video->task_mutex, NULL) != 0)
		abort();
	for (size_t i = 0; i < count; i++) {
		fixture->items[i] = (struct graphics_tasks_item){fixture, (uint32_t)i, flags[i]};
		struct obs_task_info info = {graphics_tasks_callback, &fixture->items[i]};
		deque_push_back(&video->tasks, &info, sizeof(info));
	}
	if (drain) {
		drain(video);
	} else {
		struct obs_core *previous = graphics_tasks_obs;
		graphics_tasks_obs = &fixture->core;
		execute_graphics_tasks();
		graphics_tasks_obs = previous;
	}
	state[0] = fixture->count;
	state[1] = video->tasks.size;
	state[2] = fixture->unlocked;
	state[3] = pthread_mutex_trylock(&video->task_mutex) == 0;
	if (state[3])
		pthread_mutex_unlock(&video->task_mutex);
	deque_free(&video->tasks);
	pthread_mutex_destroy(&video->task_mutex);
	free(fixture);
}
