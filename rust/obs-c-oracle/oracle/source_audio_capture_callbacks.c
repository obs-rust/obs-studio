/* Original C registration and production adapter over real source fixtures (audio-capture list). */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_add_audio_capture_callback oracle_obs_source_add_audio_capture_callback
#define obs_source_remove_audio_capture_callback oracle_obs_source_remove_audio_capture_callback
#include "obs-source-audio-capture-callbacks.c.h"
#undef obs_source_add_audio_capture_callback
#undef obs_source_remove_audio_capture_callback
#include "source-audio-capture-callback-fields.inc"

void *oracle_audio_capture_new(void);
void *oracle_audio_capture_new(void)
{
	struct obs_source *source = calloc(1, sizeof(*source));
	if (source && pthread_mutex_init(&source->audio_cb_mutex, NULL) != 0) {
		free(source);
		return NULL;
	}
	return source;
}
void oracle_audio_capture_free(obs_source_t *source);
void oracle_audio_capture_free(obs_source_t *source)
{
	da_free(source->audio_cb_list);
	pthread_mutex_destroy(&source->audio_cb_mutex);
	free(source);
}
size_t oracle_audio_capture_count(obs_source_t *source);
size_t oracle_audio_capture_count(obs_source_t *source)
{
	return source->audio_cb_list.num;
}
struct audio_cb_info oracle_audio_capture_pair(obs_source_t *source, size_t index);
struct audio_cb_info oracle_audio_capture_pair(obs_source_t *source, size_t index)
{
	return source->audio_cb_list.array[index];
}
bool oracle_audio_capture_unlocked(obs_source_t *source);
bool oracle_audio_capture_unlocked(obs_source_t *source)
{
	if (pthread_mutex_trylock(&source->audio_cb_mutex) != 0)
		return false;
	pthread_mutex_unlock(&source->audio_cb_mutex);
	return true;
}
void oracle_audio_capture_layout(size_t *layout);
void oracle_audio_capture_layout(size_t *layout)
{
	layout[0] = sizeof(struct audio_cb_info);
	layout[1] = _Alignof(struct audio_cb_info);
	layout[2] = offsetof(struct audio_cb_info, callback);
	layout[3] = offsetof(struct audio_cb_info, param);
	layout[4] = sizeof(struct darray);
	layout[5] = _Alignof(struct darray);
	layout[6] = offsetof(struct darray, array);
	layout[7] = offsetof(struct darray, num);
	layout[8] = offsetof(struct darray, capacity);
}
struct audio_capture_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void audio_capture_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void audio_capture_capture(int level, const char *format, va_list args, void *param)
{
	struct audio_capture_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
typedef void (*audio_capture_operation)(obs_source_t *, obs_source_audio_capture_t, void *);
typedef void (*audio_capture_get_log)(log_handler_t *, void **);
typedef void (*audio_capture_set_log)(log_handler_t, void *);
bool oracle_audio_capture_logs(audio_capture_operation add, audio_capture_operation remove, audio_capture_get_log get,
			       audio_capture_set_log set);
bool oracle_audio_capture_logs(audio_capture_operation add, audio_capture_operation remove, audio_capture_get_log get,
			       audio_capture_set_log set)
{
	log_handler_t old;
	void *param;
	get(&old, &param);
	struct audio_capture_log r = {0}, c = {0};
	set(audio_capture_capture, &r);
	add(NULL, NULL, NULL);
	set(audio_capture_capture, &c);
	oracle_obs_source_add_audio_capture_callback(NULL, NULL, NULL);
	bool match = r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
		     strcmp(r.message, "obs_source_add_audio_capture_callback: Null 'source' parameter") == 0 &&
		     strcmp(r.message, c.message) == 0;
	r.count = c.count = 0;
	set(audio_capture_capture, &r);
	remove(NULL, NULL, NULL);
	set(audio_capture_capture, &c);
	oracle_obs_source_remove_audio_capture_callback(NULL, NULL, NULL);
	set(old, param);
	return match && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_source_remove_audio_capture_callback: Null 'source' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
