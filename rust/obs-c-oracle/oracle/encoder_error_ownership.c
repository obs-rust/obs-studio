/* Original C, renamed only; real private layout and mutex adapter. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#define obs_encoder_set_last_error oracle_obs_encoder_set_last_error
#define obs_encoder_dup_last_error oracle_obs_encoder_dup_last_error
#include "obs-encoder-error-ownership.c.h"
#undef obs_encoder_set_last_error
#undef obs_encoder_dup_last_error
#include "encoder_error_ownership_fields.inc"

void *oracle_encoder_error_new(void);
void *oracle_encoder_error_new(void)
{
	struct obs_encoder *encoder = calloc(1, sizeof(*encoder));
	if (encoder && pthread_mutex_init(&encoder->last_error_mutex, NULL) != 0) {
		free(encoder);
		return NULL;
	}
	return encoder;
}
const char *oracle_encoder_error_read(obs_encoder_t *encoder);
const char *oracle_encoder_error_read(obs_encoder_t *encoder)
{
	return encoder->last_error_message;
}
void oracle_encoder_error_free(obs_encoder_t *encoder);
void oracle_encoder_error_free(obs_encoder_t *encoder)
{
	bfree(encoder->last_error_message);
	pthread_mutex_destroy(&encoder->last_error_mutex);
	free(encoder);
}

typedef void (*encoder_error_setter)(obs_encoder_t *, const char *);
struct encoder_error_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void encoder_error_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void encoder_error_capture(int level, const char *format, va_list args, void *param)
{
	struct encoder_error_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_encoder_error_null_log(encoder_error_setter setter);
bool oracle_encoder_error_null_log(encoder_error_setter setter)
{
	struct encoder_error_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(encoder_error_capture, &log);
	setter(NULL, NULL);
	bool ok = log.count == 1 && log.level == LOG_DEBUG &&
		  strcmp(log.message, "obs_encoder_set_last_error: Null 'encoder' parameter") == 0;
	base_set_log_handler(old_handler, old_param);
	return ok;
}

#define encoder_error_assert(value) do { if (!(value)) abort(); } while (0)
#define encoder_error_assert_equal(a, b) encoder_error_assert((a) == (b))
#define encoder_error_assert_string(a, b) encoder_error_assert(strcmp((a), (b)) == 0)
struct encoder_error_lock_probe {
	struct obs_encoder encoder;
	pthread_mutex_t mutex;
	pthread_cond_t condition;
	bool started, done, setter;
	char *copy;
	void (*set)(obs_encoder_t *, const char *);
	char *(*dup)(obs_encoder_t *);
};

static void *encoder_error_locked_call(void *data)
{
	struct encoder_error_lock_probe *probe = data;
	pthread_mutex_lock(&probe->mutex);
	probe->started = true;
	pthread_cond_signal(&probe->condition);
	pthread_mutex_unlock(&probe->mutex);
	if (probe->setter)
		probe->set(&probe->encoder, "after");
	else
		probe->copy = probe->dup(&probe->encoder);
	pthread_mutex_lock(&probe->mutex);
	probe->done = true;
	pthread_cond_signal(&probe->condition);
	pthread_mutex_unlock(&probe->mutex);
	return NULL;
}

void oracle_encoder_error_locking(void (*set)(obs_encoder_t *, const char *), char *(*dup)(obs_encoder_t *));
void oracle_encoder_error_locking(void (*set)(obs_encoder_t *, const char *), char *(*dup)(obs_encoder_t *))
{
	for (unsigned setter = 0; setter < 2; setter++) {
		struct encoder_error_lock_probe probe = {0};
		probe.setter = setter != 0;
		probe.set = set;
		probe.dup = dup;
		encoder_error_assert_equal(pthread_mutex_init(&probe.encoder.last_error_mutex, NULL), 0);
		encoder_error_assert_equal(pthread_mutex_init(&probe.mutex, NULL), 0);
		encoder_error_assert_equal(pthread_cond_init(&probe.condition, NULL), 0);
		set(&probe.encoder, "before");
		pthread_mutex_lock(&probe.encoder.last_error_mutex);
		pthread_t thread;
		encoder_error_assert_equal(pthread_create(&thread, NULL, encoder_error_locked_call, &probe), 0);
		pthread_mutex_lock(&probe.mutex);
		while (!probe.started)
			pthread_cond_wait(&probe.condition, &probe.mutex);
		struct timespec deadline;
		encoder_error_assert_equal(timespec_get(&deadline, TIME_UTC), TIME_UTC);
		deadline.tv_nsec += 50000000;
		if (deadline.tv_nsec >= 1000000000) {
			deadline.tv_sec++;
			deadline.tv_nsec -= 1000000000;
		}
		int result = 0;
		while (!probe.done && result == 0)
			result = pthread_cond_timedwait(&probe.condition, &probe.mutex, &deadline);
		bool blocked = !probe.done && result == ETIMEDOUT;
		pthread_mutex_unlock(&probe.mutex);
		/* Change while holding the same mutex: dup must see the later value. */
		bfree(probe.encoder.last_error_message);
		probe.encoder.last_error_message = bstrdup("under lock");
		pthread_mutex_unlock(&probe.encoder.last_error_mutex);
		encoder_error_assert_equal(pthread_join(thread, NULL), 0);
		encoder_error_assert(blocked);
		encoder_error_assert(probe.done);
		if (probe.setter)
			encoder_error_assert_string(probe.encoder.last_error_message, "after");
		else
			encoder_error_assert_string(probe.copy, "under lock");
		bfree(probe.copy);
		set(&probe.encoder, NULL);
		pthread_cond_destroy(&probe.condition);
		pthread_mutex_destroy(&probe.mutex);
		pthread_mutex_destroy(&probe.encoder.last_error_mutex);
	}
}

#undef encoder_error_assert_string
#undef encoder_error_assert_equal
#undef encoder_error_assert
