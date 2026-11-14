/* Real C bodies and production field adapter, with observable native primitives.
 * The shared runner also calls the Rust exports through function pointers. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include "obs-internal.h"
#include "util/threading.h"

struct group_observer {
	bool fail_init;
	struct obs_encoder_group *group;
	struct obs_encoder *encoders;
	size_t count, released, length;
	uint32_t events[512];
	bool valid;
};
#ifdef _MSC_VER
static __declspec(thread) struct group_observer group_observer;
#else
static _Thread_local struct group_observer group_observer;
#endif

static void group_event(uint32_t event)
{
	if (group_observer.length < 512)
		group_observer.events[group_observer.length++] = event;
	else
		group_observer.valid = false;
}

static void *group_zeroed(size_t size)
{
	struct obs_encoder_group *group = bzalloc(size);
	group_observer.group = group;
	group_observer.valid &= size == sizeof(*group) && !group->destroy_on_stop && !group->encoders.array &&
				!group->encoders.num && !group->encoders.capacity && !group->num_encoders_started &&
				!group->start_timestamp && !group->frame_rate_divisors_lcm &&
				!group->reconfigure_request && !group->next_pts && !group->encoders_updated_next_pts &&
				!group->encoders_reconfigured && !group->reconfigure_again;
	group_event(1);
	return group;
}

static int group_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr)
{
	group_event(2);
	group_observer.valid &= attr == NULL;
	return group_observer.fail_init ? EAGAIN : pthread_mutex_init(mutex, attr);
}

static int group_lock(pthread_mutex_t *mutex)
{
	group_event(3);
	return pthread_mutex_lock(mutex);
}

static int group_unlock(pthread_mutex_t *mutex)
{
	if (group_observer.group->destroy_on_stop && group_observer.group->num_encoders_started)
		group_event(4);
	group_event(6);
	return pthread_mutex_unlock(mutex);
}

static int group_mutex_destroy(pthread_mutex_t *mutex)
{
	int result = pthread_mutex_trylock(mutex);
	group_observer.valid &= result == 0;
	if (!result)
		pthread_mutex_unlock(mutex);
	group_event(7);
	return pthread_mutex_destroy(mutex);
}

static void group_free(void *ptr)
{
	if (ptr == group_observer.group) {
		group_observer.valid &= !group_observer.group->encoders.array && !group_observer.group->encoders.num &&
					!group_observer.group->encoders.capacity;
		group_event(8);
		group_observer.group = NULL;
	} else {
		group_event(5);
	}
	bfree(ptr);
}

/* Test dependency seam, never linked into libobs. */
void obs_encoder_release(obs_encoder_t *encoder)
{
	size_t index = group_observer.released++;
	group_observer.valid &= index < group_observer.count && encoder == &group_observer.encoders[index] &&
				!encoder->encoder_group && pthread_mutex_trylock(&group_observer.group->mutex) == EBUSY;
	group_event(100 + (uint32_t)index);
	group_event(200 + (uint32_t)index);
}

static void group_list_free(struct darray *list)
{
	group_event(5);
	darray_free(list);
}

#define darray_free group_list_free
#define bzalloc group_zeroed
#define bfree group_free
/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name outside the probes, as encoder_clear_roi.c
 * does. */
#pragma push_macro("pthread_mutex_init")
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#pragma push_macro("pthread_mutex_destroy")
#undef pthread_mutex_init
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#undef pthread_mutex_destroy
#define pthread_mutex_init group_init
#define pthread_mutex_lock group_lock
#define pthread_mutex_unlock group_unlock
#define pthread_mutex_destroy group_mutex_destroy
#include "encoder_group_lifecycle_fields.inc"
obs_encoder_group_t *oracle_obs_encoder_group_create(void);
void oracle_obs_encoder_group_destroy(obs_encoder_group_t *group);

#define obs_encoder_group_create oracle_obs_encoder_group_create
#define obs_encoder_group_actually_destroy oracle_obs_encoder_group_actually_destroy
#define obs_encoder_group_destroy oracle_obs_encoder_group_destroy
#include "encoder_group_lifecycle.inc"
#undef obs_encoder_group_destroy
#undef obs_encoder_group_actually_destroy
#undef obs_encoder_group_create
#undef pthread_mutex_destroy
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#undef pthread_mutex_init
#pragma pop_macro("pthread_mutex_destroy")
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")
#pragma pop_macro("pthread_mutex_init")
#undef bfree
#undef bzalloc
#undef darray_free

typedef obs_encoder_group_t *(*group_create_fn)(void);
typedef void (*group_destroy_fn)(obs_encoder_group_t *);

size_t oracle_encoder_group_lifecycle_run(group_create_fn create, group_destroy_fn destroy, group_destroy_fn actually,
					  size_t count, uint32_t started, size_t repeats, bool direct, bool fail_init,
					  uint32_t *events);
size_t oracle_encoder_group_lifecycle_run(group_create_fn create, group_destroy_fn destroy, group_destroy_fn actually,
					  size_t count, uint32_t started, size_t repeats, bool direct, bool fail_init,
					  uint32_t *events)
{
	memset(&group_observer, 0, sizeof(group_observer));
	group_observer.valid = count <= 16 && repeats <= 16;
	if (!group_observer.valid)
		return SIZE_MAX;
	group_observer.fail_init = fail_init;
	obs_encoder_group_t *group = create();
	if (fail_init) {
		group_observer.valid &= group == NULL && group_observer.group == NULL;
	} else {
		group_observer.encoders = calloc(count ? count : 1, sizeof(struct obs_encoder));
		group_observer.count = count;
		for (size_t i = 0; i < count; i++) {
			obs_encoder_t *encoder = &group_observer.encoders[i];
			encoder->encoder_group = group;
			da_push_back(group->encoders, &encoder);
		}
		group->num_encoders_started = started;
		for (size_t i = 0; i < repeats && started; i++) {
			destroy(group);
			group_observer.valid &= group->destroy_on_stop && group->num_encoders_started == started &&
						group->encoders.num == count && !group_observer.released;
			for (size_t j = 0; j < count; j++)
				group_observer.valid &= group_observer.encoders[j].encoder_group == group;
			int result = pthread_mutex_trylock(&group->mutex);
			group_observer.valid &= result == 0;
			if (!result)
				pthread_mutex_unlock(&group->mutex);
		}
		group->num_encoders_started = 0;
		if (direct) {
			group_lock(&group->mutex);
			actually(group);
		} else {
			destroy(group);
		}
		group_observer.valid &= !group_observer.group && group_observer.released == count;
		free(group_observer.encoders);
	}
	destroy(NULL);
	memcpy(events, group_observer.events, group_observer.length * sizeof(uint32_t));
	return group_observer.valid ? group_observer.length : SIZE_MAX;
}

void oracle_encoder_group_lifecycle_layout(size_t *layout);
void oracle_encoder_group_lifecycle_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_group_lifecycle_fields);
	layout[1] = _Alignof(struct obs_encoder_group_lifecycle_fields);
	layout[2] = offsetof(struct obs_encoder_group_lifecycle_fields, started);
	layout[3] = offsetof(struct obs_encoder_group_lifecycle_fields, destroy_on_stop);
}
