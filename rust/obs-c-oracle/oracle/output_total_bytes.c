/* Original extracted C over the real output layout, and the production adapter. No behavior exclusions. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_total_bytes oracle_obs_output_get_total_bytes
#include "obs-output-total-bytes-query-support.c.h"
#include "obs-output-total-bytes-query.c.h"
#undef obs_output_get_total_bytes
#include "output_total_bytes_fields.inc"

void oracle_output_total_bytes_layout(size_t *layout);
void oracle_output_total_bytes_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_total_bytes_fields);
	layout[1] = _Alignof(struct obs_output_total_bytes_fields);
	layout[2] = offsetof(struct obs_output_total_bytes_fields, get_total_bytes);
	layout[3] = offsetof(struct obs_output_total_bytes_fields, delay_active);
	layout[4] = offsetof(struct obs_output_total_bytes_fields, delay_capturing);
	layout[5] = offsetof(struct obs_output_total_bytes_fields, data);
	layout[6] = sizeof(bool);
	layout[7] = _Alignof(bool);
}

struct otb_state {
	unsigned first_calls;
	unsigned second_calls;
	void *last_data;
	uint64_t result;
	unsigned logs;
	int level;
	char message[128];
};
static struct otb_state *otb_active;

static uint64_t otb_first(void *data)
{
	otb_active->first_calls++;
	otb_active->last_data = data;
	return otb_active->result;
}

static uint64_t otb_second(void *data)
{
	otb_active->second_calls++;
	otb_active->last_data = data;
	return ~otb_active->result;
}

#if defined(__GNUC__) || defined(__clang__)
static void otb_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void otb_capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	otb_active->logs++;
	otb_active->level = level;
	vsnprintf(otb_active->message, sizeof(otb_active->message), format, args);
}

typedef uint64_t (*otb_getter)(const obs_output_t *output);

/* flags: bits 0-1 callback (none, first, second), 2 delay_active,
 * 3 delay_capturing, 4 NULL context.data, 5 NULL output. */
void oracle_output_total_bytes_run(otb_getter get, unsigned flags, uint64_t result, uint64_t *report);
void oracle_output_total_bytes_run(otb_getter get, unsigned flags, uint64_t result, uint64_t *report)
{
	struct otb_state state = {.result = result};
	otb_active = &state;
	struct obs_output output;
	memset(&output, 0, sizeof(output));
	unsigned which = flags & 3;
	output.info.get_total_bytes = which == 1 ? otb_first : which == 2 ? otb_second : NULL;
	os_atomic_set_bool(&output.delay_active, (flags & 4) != 0);
	os_atomic_set_bool(&output.delay_capturing, (flags & 8) != 0);
	output.context.data = (flags & 16) ? NULL : &state;
	unsigned char before[sizeof(output)];
	memcpy(before, &output, sizeof(output));

	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(otb_capture, NULL);
	uint64_t value = get((flags & 32) ? NULL : &output);
	base_set_log_handler(old_handler, old_param);

	report[0] = value;
	report[1] = state.first_calls;
	report[2] = state.second_calls;
	report[3] = state.last_data == ((flags & 16) ? NULL : (void *)&state);
	report[4] = memcmp(before, &output, sizeof(output)) == 0;
	report[5] = state.logs;
	report[6] = (uint64_t)(int64_t)state.level;
	report[7] = state.logs ? strcmp(state.message, "obs_output_get_total_bytes: Null 'output' parameter") == 0 : 1;
	otb_active = NULL;
}
