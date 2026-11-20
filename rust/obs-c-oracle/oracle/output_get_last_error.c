/* Original output last-error query over the real private output and encoder
 * layouts. The field adapters it is compared through are the production ones
 * of their own oracles (output_last_error.c, output_video_encoder_getter.c,
 * output_encoder_getters.c, encoder_last_error.c).
 *
 * The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test. _NO_CRT_STDIO_INLINE keeps them declarations only, and the
 * log capture below reads its arguments without formatting them. */
#define _NO_CRT_STDIO_INLINE
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_output_get_last_error oracle_obs_output_get_last_error
#include "obs-output-get-last-error.c.h"
#undef obs_output_get_last_error

/* Defined by the production adapters of the oracles named above. */
obs_encoder_t *obs_output_video_encoder_getter_at(const obs_output_t *output, size_t idx);
obs_encoder_t *obs_output_encoder_getters_audio_at(const obs_output_t *output, size_t idx);

#define OGLE_VIDEO MAX_OUTPUT_VIDEO_ENCODERS
#define OGLE_AUDIO MAX_OUTPUT_AUDIO_ENCODERS
/* The output's own error, then each video, then each audio encoder's. */
#define OGLE_FIELDS (1 + OGLE_VIDEO + OGLE_AUDIO)

enum ogle_kind { OGLE_NULL, OGLE_SAME, OGLE_EMPTY, OGLE_BYTES, OGLE_KINDS };

struct output_get_last_error_fixture {
	struct obs_output output;
	struct obs_encoder video[OGLE_VIDEO];
	struct obs_encoder audio[OGLE_AUDIO];
	struct obs_output output_before;
	struct obs_encoder video_before[OGLE_VIDEO];
	struct obs_encoder audio_before[OGLE_AUDIO];
	/* Independent storage: equal bytes never share an address. */
	char messages[OGLE_FIELDS][16];
	char messages_before[OGLE_FIELDS][16];
};

static char *ogle_message(struct output_get_last_error_fixture *f, size_t field, uint8_t kind)
{
	static const char bytes[] = "\xff\x80x";
	switch (kind % OGLE_KINDS) {
	case OGLE_SAME:
		memcpy(f->messages[field], "same error", sizeof("same error"));
		return f->messages[field];
	case OGLE_EMPTY:
		return f->messages[field];
	case OGLE_BYTES:
		memcpy(f->messages[field], bytes, sizeof(bytes));
		return f->messages[field];
	default:
		return NULL;
	}
}

/* kinds[OGLE_FIELDS]; bit i of a mask attaches that slot's encoder. Detached
 * encoders keep their errors, which the query must never reach. */
void *oracle_output_get_last_error_new(const uint8_t *kinds, uint32_t video, uint32_t audio);
void *oracle_output_get_last_error_new(const uint8_t *kinds, uint32_t video, uint32_t audio)
{
	struct output_get_last_error_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->output.context.name = "last-error output";
	f->output.mixer_mask = UINT32_C(0x80000001);
	f->output.last_error_message = ogle_message(f, 0, kinds[0]);
	for (size_t i = 0; i < OGLE_VIDEO; i++) {
		f->video[i].context.name = "video";
		f->video[i].last_error_message = ogle_message(f, 1 + i, kinds[1 + i]);
		f->output.video_encoders[i] = (video >> i) & 1 ? &f->video[i] : NULL;
	}
	for (size_t i = 0; i < OGLE_AUDIO; i++) {
		f->audio[i].context.name = "audio";
		f->audio[i].last_error_message = ogle_message(f, 1 + OGLE_VIDEO + i, kinds[1 + OGLE_VIDEO + i]);
		f->output.audio_encoders[i] = (audio >> i) & 1 ? &f->audio[i] : NULL;
	}
	memcpy(&f->output_before, &f->output, sizeof(f->output));
	memcpy(f->video_before, f->video, sizeof(f->video));
	memcpy(f->audio_before, f->audio, sizeof(f->audio));
	memcpy(f->messages_before, f->messages, sizeof(f->messages));
	return f;
}

/* The storage of one field's message, whatever its kind. */
const char *oracle_output_get_last_error_storage(const void *fixture, size_t field);
const char *oracle_output_get_last_error_storage(const void *fixture, size_t field)
{
	const struct output_get_last_error_fixture *f = fixture;
	return field < OGLE_FIELDS ? f->messages[field] : NULL;
}

bool oracle_output_get_last_error_unchanged(const void *fixture);
bool oracle_output_get_last_error_unchanged(const void *fixture)
{
	const struct output_get_last_error_fixture *f = fixture;
	return memcmp(&f->output, &f->output_before, sizeof(f->output)) == 0 &&
	       memcmp(f->video, f->video_before, sizeof(f->video)) == 0 &&
	       memcmp(f->audio, f->audio_before, sizeof(f->audio)) == 0 &&
	       memcmp(f->messages, f->messages_before, sizeof(f->messages)) == 0;
}

void oracle_output_get_last_error_free(void *fixture);
void oracle_output_get_last_error_free(void *fixture)
{
	free(fixture);
}

/* The slot counts the Rust core walks. */
void oracle_output_get_last_error_layout(size_t *out);
void oracle_output_get_last_error_layout(size_t *out)
{
	out[0] = MAX_OUTPUT_VIDEO_ENCODERS;
	out[1] = MAX_OUTPUT_AUDIO_ENCODERS;
	out[2] = OGLE_FIELDS;
}

/* Every adapter the Rust shim reads through addresses exactly the field the
 * original reads, on the real private layouts. */
bool oracle_output_get_last_error_adapters(void *fixture);
bool oracle_output_get_last_error_adapters(void *fixture)
{
	struct output_get_last_error_fixture *f = fixture;
	bool ok = obs_output_last_error_fields(&f->output) == &f->output.last_error_message;
	for (size_t i = 0; i < OGLE_VIDEO; i++) {
		ok &= obs_output_video_encoder_getter_at(&f->output, i) == f->output.video_encoders[i];
		ok &= obs_encoder_last_error_fields(&f->video[i]).message == &f->video[i].last_error_message;
	}
	for (size_t i = 0; i < OGLE_AUDIO; i++) {
		ok &= obs_output_encoder_getters_audio_at(&f->output, i) == f->output.audio_encoders[i];
		ok &= obs_encoder_last_error_fields(&f->audio[i]).message == &f->audio[i].last_error_message;
	}
	return ok;
}

struct ogle_log {
	unsigned count;
	int level;
	const char *format;
	const char *function;
	const char *parameter;
};

static void ogle_capture(int level, const char *format, va_list args, void *param)
{
	struct ogle_log *log = param;
	log->count++;
	log->level = level;
	log->format = format;
	/* obs_output_valid()'s two %s arguments, read without formatting. */
	log->function = va_arg(args, const char *);
	log->parameter = va_arg(args, const char *);
}

static bool ogle_exact_null_log(const struct ogle_log *log)
{
	return log->count == 1 && log->level == LOG_DEBUG && strcmp(log->format, "%s: Null '%s' parameter") == 0 &&
	       strcmp(log->function, "obs_output_get_last_error") == 0 && strcmp(log->parameter, "output") == 0;
}

/* A NULL output: NULL, the exact diagnostic once, and no allocation. The
 * original runs first on the same capture, so both sides are checked. */
typedef const char *(*output_get_last_error_getter)(obs_output_t *output);
bool oracle_output_get_last_error_null_log(output_get_last_error_getter getter);
bool oracle_output_get_last_error_null_log(output_get_last_error_getter getter)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	struct ogle_log original = {0}, candidate = {0};
	long allocations = bnum_allocs();
	base_set_log_handler(ogle_capture, &original);
	const char *original_result = oracle_obs_output_get_last_error(NULL);
	base_set_log_handler(ogle_capture, &candidate);
	const char *candidate_result = getter(NULL);
	base_set_log_handler(old_handler, old_param);
	return !original_result && !candidate_result && ogle_exact_null_log(&original) &&
	       ogle_exact_null_log(&candidate) && bnum_allocs() == allocations;
}
