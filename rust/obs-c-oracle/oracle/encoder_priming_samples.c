/* Original C getter and real production adapter over real-header encoder fixtures. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"

uint32_t oracle_obs_encoder_get_priming_samples(const obs_encoder_t *encoder);
#define obs_encoder_get_priming_samples oracle_obs_encoder_get_priming_samples
#include "encoder-priming-samples.inc"
#undef obs_encoder_get_priming_samples
#include "encoder-priming-samples-fields.inc"

struct eps_fixture {
	uint32_t result;
	uint32_t token;
	unsigned calls;
	unsigned orig_calls;
	void *seen;
};
static THREAD_LOCAL struct eps_fixture *eps_active;

static uint32_t eps_callback(void *data)
{
	eps_active->calls++;
	eps_active->seen = data;
	return eps_active->result;
}

static uint32_t eps_orig_callback(void *data)
{
	(void)data;
	eps_active->orig_calls++;
	return eps_active->result ^ UINT32_MAX;
}

typedef uint32_t (*eps_getter)(const obs_encoder_t *encoder);
void oracle_encoder_priming_samples_run(eps_getter getter, unsigned flags, uint32_t result, uint32_t token,
					uint64_t *report);
void oracle_encoder_priming_samples_run(eps_getter getter, unsigned flags, uint32_t result, uint32_t token,
					uint64_t *report)
{
	struct eps_fixture fixture = {.result = result, .token = token};
	struct obs_encoder encoder = {0};
	encoder.info.get_priming_samples = (flags & 1) ? eps_callback : NULL;
	encoder.orig_info.get_priming_samples = (flags & 2) ? eps_orig_callback : NULL;
	encoder.context.data = (flags & 4) ? &fixture.token : NULL;
	/* Distinct from either expected callback argument, including NULL. */
	fixture.seen = &encoder;
	struct obs_encoder before;
	memcpy(&before, &encoder, sizeof(encoder));
	eps_active = &fixture;
	report[0] = getter(&encoder);
	eps_active = NULL;
	report[1] = fixture.calls;
	report[2] = fixture.orig_calls;
	report[3] = fixture.seen == ((flags & 1) ? encoder.context.data : &encoder);
	report[4] = encoder.context.data == before.context.data;
	report[5] = encoder.info.get_priming_samples == before.info.get_priming_samples;
	report[6] = fixture.token;
	report[7] = memcmp(&encoder, &before, sizeof(encoder)) == 0;
}

void oracle_encoder_priming_samples_layout(size_t *layout);
void oracle_encoder_priming_samples_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_priming_samples_fields);
	layout[1] = _Alignof(struct obs_encoder_priming_samples_fields);
	layout[2] = offsetof(struct obs_encoder_priming_samples_fields, callback);
	layout[3] = offsetof(struct obs_encoder_priming_samples_fields, data);
}
