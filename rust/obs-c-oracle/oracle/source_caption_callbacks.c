/* Original C registration and production adapter over real source fixtures. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_add_caption_callback oracle_obs_source_add_caption_callback
#define obs_source_remove_caption_callback oracle_obs_source_remove_caption_callback
#include "obs-source-caption-callbacks.c.h"
#undef obs_source_add_caption_callback
#undef obs_source_remove_caption_callback
#include "source-caption-callback-fields.inc"

void *oracle_caption_new(void);
void *oracle_caption_new(void)
{
	struct obs_source *source = calloc(1, sizeof(*source));
	if (source && pthread_mutex_init(&source->caption_cb_mutex, NULL) != 0) {
		free(source);
		return NULL;
	}
	return source;
}
void oracle_caption_free(obs_source_t *source);
void oracle_caption_free(obs_source_t *source)
{
	da_free(source->caption_cb_list);
	pthread_mutex_destroy(&source->caption_cb_mutex);
	free(source);
}
size_t oracle_caption_count(obs_source_t *source);
size_t oracle_caption_count(obs_source_t *source)
{
	return source->caption_cb_list.num;
}
struct caption_cb_info oracle_caption_pair(obs_source_t *source, size_t index);
struct caption_cb_info oracle_caption_pair(obs_source_t *source, size_t index)
{
	return source->caption_cb_list.array[index];
}
bool oracle_caption_unlocked(obs_source_t *source);
bool oracle_caption_unlocked(obs_source_t *source)
{
	if (pthread_mutex_trylock(&source->caption_cb_mutex) != 0)
		return false;
	pthread_mutex_unlock(&source->caption_cb_mutex);
	return true;
}
void oracle_caption_layout(size_t *layout);
void oracle_caption_layout(size_t *layout)
{
	layout[0] = sizeof(struct caption_cb_info);
	layout[1] = _Alignof(struct caption_cb_info);
	layout[2] = offsetof(struct caption_cb_info, callback);
	layout[3] = offsetof(struct caption_cb_info, param);
	layout[4] = sizeof(struct darray);
	layout[5] = _Alignof(struct darray);
	layout[6] = offsetof(struct darray, array);
	layout[7] = offsetof(struct darray, num);
	layout[8] = offsetof(struct darray, capacity);
}
struct caption_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void caption_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void caption_capture(int level, const char *format, va_list args, void *param)
{
	struct caption_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
typedef void (*caption_operation)(obs_source_t *, obs_source_caption_t, void *);
typedef void (*caption_get_log)(log_handler_t *, void **);
typedef void (*caption_set_log)(log_handler_t, void *);
bool oracle_caption_logs(caption_operation add, caption_operation remove, caption_get_log get, caption_set_log set);
bool oracle_caption_logs(caption_operation add, caption_operation remove, caption_get_log get, caption_set_log set)
{
	log_handler_t old;
	void *param;
	get(&old, &param);
	struct caption_log r = {0}, c = {0};
	set(caption_capture, &r);
	add(NULL, NULL, NULL);
	set(caption_capture, &c);
	oracle_obs_source_add_caption_callback(NULL, NULL, NULL);
	bool match = r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
		     strcmp(r.message, "obs_source_add_caption_callback: Null 'source' parameter") == 0 &&
		     strcmp(r.message, c.message) == 0;
	r.count = c.count = 0;
	set(caption_capture, &r);
	remove(NULL, NULL, NULL);
	set(caption_capture, &c);
	oracle_obs_source_remove_caption_callback(NULL, NULL, NULL);
	set(old, param);
	return match && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_source_remove_caption_callback: Null 'source' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
