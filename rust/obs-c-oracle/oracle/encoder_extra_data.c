/* Original getter and real adapter over stack-only real-header fixtures. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "obs-internal.h"

bool oracle_obs_encoder_get_extra_data(const obs_encoder_t *, uint8_t **, size_t *);
#define obs_encoder_get_extra_data oracle_obs_encoder_get_extra_data
#include "encoder-extra-data.inc"
#undef obs_encoder_get_extra_data
#include "encoder-extra-data-fields.inc"

struct eed_fixture {
	unsigned flags;
	size_t size;
	unsigned calls;
	unsigned orig_calls;
	bool context_matches;
	bool outputs_match;
	uint8_t **expected_data;
	size_t *expected_size;
	uint8_t payload;
	unsigned logs;
	int level;
	char message[128];
};
static THREAD_LOCAL struct eed_fixture *eed_active;

static bool eed_callback(void *data, uint8_t **extra_data, size_t *size)
{
	struct eed_fixture *f = eed_active;
	f->calls++;
	f->context_matches = data == f;
	f->outputs_match = extra_data == f->expected_data && size == f->expected_size;
	if (f->flags & 128) {
		if (extra_data)
			*extra_data = (f->flags & 256) ? NULL : &f->payload;
		if (size)
			*size = f->size;
	}
	return (f->flags & 64) != 0;
}

static bool eed_orig_callback(void *data, uint8_t **extra_data, size_t *size)
{
	(void)data;
	(void)extra_data;
	(void)size;
	eed_active->orig_calls++;
	return false;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void eed_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	eed_active->logs++;
	eed_active->level = level;
	vsnprintf(eed_active->message, sizeof(eed_active->message), format, args);
}

typedef bool (*eed_getter)(const obs_encoder_t *, uint8_t **, size_t *);
void oracle_encoder_extra_data_run(eed_getter getter, unsigned flags, size_t size, uint64_t *report);
void oracle_encoder_extra_data_run(eed_getter getter, unsigned flags, size_t size, uint64_t *report)
{
	struct eed_fixture f = {.flags = flags, .size = size};
	struct obs_encoder encoder = {0};
	encoder.info.get_extra_data = (flags & 1) ? eed_callback : NULL;
	encoder.orig_info.get_extra_data = (flags & 4) ? eed_orig_callback : NULL;
	encoder.context.data = (flags & 2) ? &f : NULL;
	struct obs_encoder before;
	memcpy(&before, &encoder, sizeof(before));
	uint8_t sentinel = 0;
	uint8_t *data = &sentinel;
	size_t output_size = 17;
	f.expected_data = (flags & 16) ? &data : NULL;
	f.expected_size = (flags & 32) ? &output_size : NULL;
	log_handler_t saved;
	void *saved_param;
	base_get_log_handler(&saved, &saved_param);
	eed_active = &f;
	base_set_log_handler(eed_log, NULL);
	report[0] = getter((flags & 8) ? NULL : &encoder, f.expected_data, f.expected_size);
	base_set_log_handler(saved, saved_param);
	eed_active = NULL;
	report[1] = f.calls;
	report[2] = f.orig_calls;
	report[3] = f.context_matches;
	report[4] = f.outputs_match;
	report[5] = data == &sentinel ? 0 : (data == &f.payload ? 1 : (data == NULL ? 2 : 3));
	report[6] = output_size;
	report[7] = memcmp(&encoder, &before, sizeof(encoder)) == 0;
	report[8] = f.logs;
	report[9] = (uint64_t)f.level;
	report[10] = strcmp(f.message, "obs_encoder_get_extra_data: Null 'encoder' parameter") == 0;
}

void oracle_encoder_extra_data_layout(size_t *layout);
void oracle_encoder_extra_data_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_extra_data_fields);
	layout[1] = _Alignof(struct obs_encoder_extra_data_fields);
	layout[2] = offsetof(struct obs_encoder_extra_data_fields, callback);
	layout[3] = offsetof(struct obs_encoder_extra_data_fields, data);
}
