/* Original extracted C and production adapter, with real-header fixtures. */
#include "obs-internal.h"
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct clear_roi_fixture {
	struct obs_encoder encoder;
	struct obs_encoder_roi storage[8];
	unsigned locks;
	unsigned unlocks;
	bool empty_on_lock;
	bool ordering_ok;
	uint32_t initial_increment;
};
static int clear_roi_lock(pthread_mutex_t *mutex)
{
	struct clear_roi_fixture *fixture = (void *)((char *)mutex - offsetof(struct obs_encoder, roi_mutex));
	fixture->ordering_ok &= fixture->encoder.roi.num > 0 && fixture->locks == fixture->unlocks;
	fixture->locks++;
	int result = pthread_mutex_lock(mutex);
	if (fixture->empty_on_lock)
		fixture->encoder.roi.num = 0;
	return result;
}
static int clear_roi_unlock(pthread_mutex_t *mutex)
{
	struct clear_roi_fixture *fixture = (void *)((char *)mutex - offsetof(struct obs_encoder, roi_mutex));
	fixture->ordering_ok &= fixture->encoder.roi.num == 0 &&
				fixture->encoder.roi_increment == fixture->initial_increment + UINT32_C(1);
	fixture->unlocks++;
	return pthread_mutex_unlock(mutex);
}
void oracle_obs_encoder_clear_roi(obs_encoder_t *encoder);

#define pthread_mutex_lock clear_roi_lock
#define pthread_mutex_unlock clear_roi_unlock
#define obs_encoder_clear_roi oracle_obs_encoder_clear_roi
#include "obs-encoder-clear-roi.c.h"
#undef obs_encoder_clear_roi
#include "encoder-clear-roi-fields.inc"
#undef pthread_mutex_unlock
#undef pthread_mutex_lock

void *oracle_encoder_clear_roi_new(size_t count, uint32_t increment, bool empty_on_lock);
void *oracle_encoder_clear_roi_new(size_t count, uint32_t increment, bool empty_on_lock)
{
	struct clear_roi_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	if (pthread_mutex_init(&fixture->encoder.roi_mutex, NULL) != 0) {
		free(fixture);
		return NULL;
	}
	fixture->encoder.roi.array = fixture->storage;
	fixture->encoder.roi.num = count;
	fixture->encoder.roi.capacity = 8;
	fixture->encoder.roi_increment = increment;
	fixture->encoder.encoded_frames = 71;
	memset(fixture->storage, 0x5a, sizeof(fixture->storage));
	fixture->initial_increment = increment;
	fixture->empty_on_lock = empty_on_lock;
	fixture->ordering_ok = true;
	return fixture;
}

bool oracle_encoder_clear_roi_check(void *ptr, size_t before, uint32_t increment);
bool oracle_encoder_clear_roi_check(void *ptr, size_t before, uint32_t increment)
{
	struct clear_roi_fixture *fixture = ptr;
	bool ok = fixture->encoder.roi.num == 0 &&
		  fixture->encoder.roi_increment == increment + (before ? UINT32_C(1) : UINT32_C(0)) &&
		  fixture->encoder.roi.array == fixture->storage && fixture->encoder.roi.capacity == 8 &&
		  fixture->encoder.encoded_frames == 71 && fixture->ordering_ok &&
		  fixture->locks == (before ? 1u : 0u) && fixture->unlocks == fixture->locks;
	const unsigned char *bytes = (const void *)fixture->storage;
	for (size_t i = 0; i < sizeof(fixture->storage); i++)
		ok &= bytes[i] == 0x5a;
	int result = pthread_mutex_trylock(&fixture->encoder.roi_mutex);
	ok &= result == 0;
	if (result == 0)
		pthread_mutex_unlock(&fixture->encoder.roi_mutex);
	return ok;
}
void oracle_encoder_clear_roi_free(void *ptr);
void oracle_encoder_clear_roi_free(void *ptr)
{
	struct clear_roi_fixture *fixture = ptr;
	pthread_mutex_destroy(&fixture->encoder.roi_mutex);
	free(fixture);
}
void oracle_encoder_clear_roi_layout(size_t *layout);
void oracle_encoder_clear_roi_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_clear_roi_fields);
	layout[1] = _Alignof(struct obs_encoder_clear_roi_fields);
	layout[2] = offsetof(struct obs_encoder_clear_roi_fields, count);
	layout[3] = offsetof(struct obs_encoder_clear_roi_fields, increment);
	layout[4] = offsetof(struct obs_encoder_clear_roi_fields, mutex);
	layout[5] = offsetof(struct obs_encoder_clear_roi_fields, lock);
	layout[6] = offsetof(struct obs_encoder_clear_roi_fields, unlock);
}
struct clear_roi_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void clear_roi_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void clear_roi_capture(int level, const char *format, va_list args, void *param)
{
	struct clear_roi_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_encoder_clear_roi_null_log(void (*clear)(obs_encoder_t *));
bool oracle_encoder_clear_roi_null_log(void (*clear)(obs_encoder_t *))
{
	struct clear_roi_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(clear_roi_capture, &log);
	clear(NULL);
	base_set_log_handler(old_handler, old_param);
	return log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_encoder_clear_roi: Null 'encoder' parameter") == 0;
}
