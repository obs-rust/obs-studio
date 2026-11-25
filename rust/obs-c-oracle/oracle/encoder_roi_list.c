/* Real encoder/header fixture; the original list policy is preserved verbatim. */
#include "obs-internal.h"
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct roi_list_fixture {
	struct obs_encoder encoder;
	uint32_t width, height;
	unsigned locks, unlocks, video_reads;
	bool locked;
};
static struct roi_list_fixture *roi_list_owner(pthread_mutex_t *mutex)
{
	return (void *)((char *)mutex - offsetof(struct obs_encoder, roi_mutex));
}
static int roi_list_lock(pthread_mutex_t *mutex)
{
	struct roi_list_fixture *f = roi_list_owner(mutex);
	f->locks++;
	int result = pthread_mutex_lock(mutex);
	f->locked = true;
	return result;
}
static int roi_list_unlock(pthread_mutex_t *mutex)
{
	struct roi_list_fixture *f = roi_list_owner(mutex);
	f->unlocks++;
	f->locked = false;
	return pthread_mutex_unlock(mutex);
}
static uint32_t roi_list_width(void *media)
{
	struct roi_list_fixture *f = media;
	f->video_reads++;
	return f->width;
}
static uint32_t roi_list_height(void *media)
{
	struct roi_list_fixture *f = media;
	f->video_reads++;
	return f->height;
}

bool oracle_obs_encoder_add_roi(obs_encoder_t *, const struct obs_encoder_roi *);
void oracle_obs_encoder_enum_roi(obs_encoder_t *, void (*)(void *, struct obs_encoder_roi *), void *);
bool oracle_obs_encoder_roi_admit(uint32_t, const struct obs_encoder_roi *);
bool oracle_obs_encoder_roi_scale_factors(uint32_t, uint32_t, uint32_t, uint32_t, float *, float *);
void oracle_obs_encoder_roi_scale(const struct obs_encoder_roi *, float, float, struct obs_encoder_roi *);
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock roi_list_lock
#define pthread_mutex_unlock roi_list_unlock
#define video_output_get_width roi_list_width
#define video_output_get_height roi_list_height
#define obs_encoder_add_roi oracle_obs_encoder_add_roi
#define obs_encoder_enum_roi oracle_obs_encoder_enum_roi
#define obs_encoder_roi_admit oracle_obs_encoder_roi_admit
#define obs_encoder_roi_scale_factors oracle_obs_encoder_roi_scale_factors
#define obs_encoder_roi_scale oracle_obs_encoder_roi_scale
#include "rust/obs-c-oracle/legacy/obs-encoder-roi-list.inc"
#include "encoder-roi-list-fields.inc"
#undef obs_encoder_roi_scale
#undef obs_encoder_roi_scale_factors
#undef obs_encoder_roi_admit
#undef obs_encoder_enum_roi
#undef obs_encoder_add_roi
#undef video_output_get_height
#undef video_output_get_width
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

void *oracle_roi_list_new(uint32_t caps, uint32_t sw, uint32_t sh, uint32_t w, uint32_t h, uint32_t increment);
void *oracle_roi_list_new(uint32_t caps, uint32_t sw, uint32_t sh, uint32_t w, uint32_t h, uint32_t increment)
{
	struct roi_list_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	if (pthread_mutex_init(&f->encoder.roi_mutex, NULL) != 0) {
		free(f);
		return NULL;
	}
	f->encoder.info.caps = caps;
	f->encoder.scaled_width = sw;
	f->encoder.scaled_height = sh;
	f->encoder.media = (void *)f;
	f->encoder.roi_increment = increment;
	f->encoder.encoded_frames = 71;
	f->width = w;
	f->height = h;
	return f;
}
void oracle_roi_list_free(void *ptr);
void oracle_roi_list_free(void *ptr)
{
	struct roi_list_fixture *f = ptr;
	da_free(f->encoder.roi);
	pthread_mutex_destroy(&f->encoder.roi_mutex);
	free(f);
}
void oracle_roi_list_state(void *ptr, uint32_t *state);
void oracle_roi_list_state(void *ptr, uint32_t *state)
{
	struct roi_list_fixture *f = ptr;
	state[0] = (uint32_t)f->encoder.roi.num;
	state[1] = f->encoder.roi_increment;
	state[2] = f->locks;
	state[3] = f->unlocks;
	state[4] = f->video_reads;
	state[5] = f->encoder.encoded_frames == 71 && !f->locked;
}
bool oracle_roi_list_locked(void *ptr);
bool oracle_roi_list_locked(void *ptr)
{
	return ((struct roi_list_fixture *)ptr)->locked;
}
void oracle_roi_list_get(void *ptr, size_t index, struct obs_encoder_roi *roi);
void oracle_roi_list_get(void *ptr, size_t index, struct obs_encoder_roi *roi)
{
	*roi = ((struct roi_list_fixture *)ptr)->encoder.roi.array[index];
}
void oracle_roi_list_layout(size_t *out);
void oracle_roi_list_layout(size_t *out)
{
	out[0] = sizeof(struct obs_encoder_roi_list_fields);
	out[1] = _Alignof(struct obs_encoder_roi_list_fields);
	out[2] = offsetof(struct obs_encoder_roi_list_fields, rois);
	out[3] = offsetof(struct obs_encoder_roi_list_fields, increment);
	out[4] = offsetof(struct obs_encoder_roi_list_fields, caps);
	out[5] = offsetof(struct obs_encoder_roi_list_fields, scaled_width);
	out[6] = offsetof(struct obs_encoder_roi_list_fields, scaled_height);
	out[7] = offsetof(struct obs_encoder_roi_list_fields, media);
	out[8] = offsetof(struct obs_encoder_roi_list_fields, mutex);
	out[9] = offsetof(struct obs_encoder_roi_list_fields, lock);
	out[10] = offsetof(struct obs_encoder_roi_list_fields, unlock);
	out[11] = offsetof(struct obs_encoder_roi_list_fields, video_size);
}
struct roi_list_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void roi_list_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void roi_list_capture(int level, const char *format, va_list args, void *param)
{
	struct roi_list_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_roi_list_null_logs(bool (*add)(obs_encoder_t *, const struct obs_encoder_roi *),
			       void (*enumerate)(obs_encoder_t *, void (*)(void *, struct obs_encoder_roi *), void *));
bool oracle_roi_list_null_logs(bool (*add)(obs_encoder_t *, const struct obs_encoder_roi *),
			       void (*enumerate)(obs_encoder_t *, void (*)(void *, struct obs_encoder_roi *), void *))
{
	struct roi_list_log log = {0};
	log_handler_t old;
	void *old_param;
	base_get_log_handler(&old, &old_param);
	base_set_log_handler(roi_list_capture, &log);
	bool ok = !add(NULL, NULL) && log.count == 1 && log.level == LOG_DEBUG &&
		  strcmp(log.message, "obs_encoder_add_roi: Null 'encoder' parameter") == 0;
	enumerate(NULL, NULL, NULL);
	ok &= log.count == 2 && log.level == LOG_DEBUG &&
	      strcmp(log.message, "obs_encoder_enum_roi: Null 'encoder' parameter") == 0;
	base_set_log_handler(old, old_param);
	return ok;
}
