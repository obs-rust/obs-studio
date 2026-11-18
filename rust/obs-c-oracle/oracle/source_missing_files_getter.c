/* Original getter, original data_valid expression, and real private layout. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_missing_files oracle_obs_source_get_missing_files
#include "libobs/build/libobs.pch.h"
#include "source-missing-files-getter.inc"
#undef obs_source_get_missing_files
#include "source_missing_files_getter_fields.inc"

void oracle_source_missing_files_layout(size_t *out);
void oracle_source_missing_files_layout(size_t *out)
{
	out[0] = sizeof(struct obs_source_missing_files_getter_fields);
	out[1] = _Alignof(struct obs_source_missing_files_getter_fields);
	out[2] = offsetof(struct obs_source_missing_files_getter_fields, data);
	out[3] = offsetof(struct obs_source_missing_files_getter_fields, callback);
}

struct source_missing_files_state {
	unsigned calls;
	void *received;
	obs_missing_files_t *result;
};

static obs_missing_files_t *source_missing_files_callback(void *data)
{
	struct source_missing_files_state *state = data;
	state->calls++;
	state->received = data;
	return state->result;
}

struct source_missing_files_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_missing_files_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_missing_files_capture(int level, const char *format, va_list args, void *param)
{
	struct source_missing_files_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef obs_missing_files_t *(*source_missing_files_getter)(const obs_source_t *);
typedef void (*source_missing_files_get_log)(log_handler_t *, void **);
typedef void (*source_missing_files_set_log)(log_handler_t, void *);

bool oracle_source_missing_files_check(source_missing_files_getter proc, source_missing_files_get_log get_log,
				       source_missing_files_set_log set_log, unsigned flags, uint32_t noise);
bool oracle_source_missing_files_check(source_missing_files_getter proc, source_missing_files_get_log get_log,
				       source_missing_files_set_log set_log, unsigned flags, uint32_t noise)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	bool equal = true;
	bool present = (flags & 1) != 0, data = (flags & 2) != 0, callback = (flags & 4) != 0;
	bool dispatch = present && data && callback;
	source_missing_files_getter getters[] = {proc, oracle_obs_source_get_missing_files};
	for (size_t g = 0; g < 2; g++) {
		struct source_missing_files_state state = {0};
		if (dispatch && !(flags & 8))
			state.result = obs_missing_files_create();
		struct obs_source source = {0}, before;
		source.context.data = data ? &state : NULL;
		source.info.missing_files = callback ? source_missing_files_callback : NULL;
		source.info.output_flags = noise;
		source.info.type_data = &source;
		memcpy(&before, &source, sizeof(source));
		struct source_missing_files_log log = {0};
		set_log(source_missing_files_capture, &log);
		obs_missing_files_t *results[2];
		for (size_t i = 0; i < 2; i++) {
			results[i] = getters[g](present ? &source : NULL);
			equal = equal && memcmp(&before, &source, sizeof(source)) == 0;
			if (dispatch) {
				equal = equal && results[i] == state.result && state.calls == i + 1 &&
					state.received == &state;
			} else {
				equal = equal && results[i] && obs_missing_files_count(results[i]) == 0 &&
					state.calls == 0 && !state.received;
			}
		}
		if (!dispatch)
			equal = equal && results[0] != results[1];
		equal = equal && log.count == (present ? 0u : 2u);
		if (!present)
			equal = equal && log.level == LOG_DEBUG &&
				strcmp(log.message, "obs_source_get_missing_files: Null 'source' parameter") == 0;
		set_log(previous, param);
		bool destroy_extra = state.result && state.result != results[0] && state.result != results[1];
		if (results[1] && results[1] != results[0])
			obs_missing_files_destroy(results[1]);
		if (results[0])
			obs_missing_files_destroy(results[0]);
		if (destroy_extra)
			obs_missing_files_destroy(state.result);
	}
	return equal;
}
