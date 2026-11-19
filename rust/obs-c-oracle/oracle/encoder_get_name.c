/* Original C body and production adapter, with the real private encoder layout. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"
const char *oracle_obs_encoder_get_name(const obs_encoder_t *encoder);
#define obs_encoder_get_name oracle_obs_encoder_get_name
#include "encoder-get-name.inc"
#undef obs_encoder_get_name
#include "encoder-get-name-fields.inc"

struct encoder_name_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void encoder_name_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void encoder_name_capture(int level, const char *format, va_list args, void *param)
{
	struct encoder_name_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef const char *(*encoder_name_getter)(const obs_encoder_t *);
static bool encoder_name_compare(const obs_encoder_t *encoder, encoder_name_getter rust)
{
	struct encoder_name_log r = {0}, c = {0};
	base_set_log_handler(encoder_name_capture, &r);
	const char *actual = rust(encoder);
	base_set_log_handler(encoder_name_capture, &c);
	const char *expected = oracle_obs_encoder_get_name(encoder);
	return actual == expected && r.count == c.count && r.level == c.level && strcmp(r.message, c.message) == 0 &&
	       (encoder ? r.count == 0
			: r.count == 1 && r.level == LOG_DEBUG &&
				  strcmp(r.message, "obs_encoder_get_name: Null 'encoder' parameter") == 0);
}

bool oracle_encoder_get_name_probe(encoder_name_getter rust, const char *name, const char *renamed, bool null_name);
bool oracle_encoder_get_name_probe(encoder_name_getter rust, const char *name, const char *renamed, bool null_name)
{
	log_handler_t saved_handler;
	void *saved_param;
	base_get_log_handler(&saved_handler, &saved_param);
	struct obs_encoder encoder = {0};
	char first[128], second[128];
	snprintf(first, sizeof(first), "%s", name);
	snprintf(second, sizeof(second), "%s", renamed);
	encoder.context.name = null_name ? NULL : first;
	unsigned char before[sizeof(encoder)];
	memcpy(before, &encoder, sizeof(encoder));
	bool equal = encoder_name_compare(NULL, rust) && encoder_name_compare(&encoder, rust) &&
		     encoder_name_compare(&encoder, rust) && memcmp(before, &encoder, sizeof(encoder)) == 0;
	encoder.context.name = second;
	memcpy(before, &encoder, sizeof(encoder));
	equal = equal && encoder_name_compare(&encoder, rust) && memcmp(before, &encoder, sizeof(encoder)) == 0;
	base_set_log_handler(saved_handler, saved_param);
	return equal;
}
