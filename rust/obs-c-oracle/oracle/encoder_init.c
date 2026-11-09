/* Original body and real production bridge, with observable boundary effects. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

struct encoder_init_probe {
	struct obs_encoder encoder;
	int fail;
	uint32_t events[16];
	size_t count;
	unsigned stage;
	unsigned callbacks;
	unsigned value;
	unsigned order;
	bool forwarded;
	bool replace_defaults2;
};
static _Thread_local struct encoder_init_probe *encoder_init_current;

static void encoder_init_event(uint32_t event)
{
	struct encoder_init_probe *probe = encoder_init_current;
	if (probe->count < 16)
		probe->events[probe->count++] = event;
}

static unsigned encoder_init_index(pthread_mutex_t *mutex)
{
	struct obs_encoder *encoder = &encoder_init_current->encoder;
	pthread_mutex_t *locks[] = {&encoder->init_mutex,  &encoder->callbacks_mutex, &encoder->outputs_mutex,
				    &encoder->pause.mutex, &encoder->roi_mutex,       &encoder->last_error_mutex};
	for (unsigned i = 0; i < 6; i++)
		if (mutex == locks[i])
			return i;
	return 99;
}

static void encoder_init_probe_sentinel(pthread_mutex_t *mutex)
{
	encoder_init_event(encoder_init_index(mutex));
	memset(mutex, 0x5a, sizeof(*mutex));
}

static int encoder_init_probe_lock(pthread_mutex_t *mutex, bool recursive)
{
	struct encoder_init_probe *probe = encoder_init_current;
	unsigned index = encoder_init_index(mutex);
	encoder_init_event(20 + index + (recursive ? 100 : 0));
	if ((int)++probe->stage == probe->fail)
		return 37;
	memset(mutex, recursive ? 0x11 : 0x22, sizeof(*mutex));
	return 0;
}

static void encoder_init_probe_modern(obs_data_t *settings, void *type_data)
{
	struct encoder_init_probe *probe = encoder_init_current;
	probe->forwarded = probe->forwarded && settings == (obs_data_t *)probe && type_data == &probe->value;
	encoder_init_event(41);
	probe->order = probe->order * 10 + 2;
	probe->value = probe->value * 3 + 22;
}

static void encoder_init_probe_legacy(obs_data_t *settings)
{
	struct encoder_init_probe *probe = encoder_init_current;
	probe->forwarded = probe->forwarded && settings == (obs_data_t *)probe;
	encoder_init_event(40);
	probe->order = 1;
	probe->value = probe->value * 5 + 21;
	/* The original reads get_defaults2 after this callback returns. */
	if (probe->replace_defaults2)
		probe->encoder.orig_info.get_defaults2 =
			probe->encoder.orig_info.get_defaults2 ? NULL : encoder_init_probe_modern;
}

static bool encoder_init_probe_context(struct obs_context_data *context, enum obs_obj_type type, obs_data_t *settings,
				       const char *name, const char *uuid, obs_data_t *hotkeys, bool private)
{
	struct encoder_init_probe *probe = encoder_init_current;
	encoder_init_event(6);
	probe->forwarded = context == &probe->encoder.context && type == OBS_OBJ_TYPE_ENCODER &&
			   settings == (obs_data_t *)&probe->value && name == (const char *)probe && !uuid &&
			   hotkeys == (obs_data_t *)&probe->order && !private;
	context->settings = (obs_data_t *)probe;
	return probe->fail != 0;
}

#undef pthread_mutex_init_value
#define pthread_mutex_init_value(mutex) encoder_init_probe_sentinel(mutex)
#define pthread_mutex_init_recursive(mutex) encoder_init_probe_lock(mutex, true)
#define pthread_mutex_init(mutex, attr) encoder_init_probe_lock(mutex, false)
#define obs_context_data_init encoder_init_probe_context
#define init_encoder encoder_init_original
#include "libobs/obs-encoder-init.c.h"
#undef init_encoder
#define init_encoder encoder_init_bridge
#include "encoder-init-fields.inc"
#undef init_encoder
#undef obs_context_data_init
#undef pthread_mutex_init
#undef pthread_mutex_init_recursive
#undef pthread_mutex_init_value

/* Calling the actual bridge resolves obs_encoder_init_steps to Rust. */
void oracle_encoder_init_run(bool rust, int fail, unsigned callbacks, unsigned value, bool replace, uint32_t *out);
void oracle_encoder_init_run(bool rust, int fail, unsigned callbacks, unsigned value, bool replace, uint32_t *out)
{
	struct encoder_init_probe probe = {0};
	probe.fail = fail;
	probe.callbacks = callbacks;
	probe.value = value;
	probe.forwarded = true;
	probe.replace_defaults2 = replace;
	probe.encoder.orig_info.get_defaults = callbacks & 1 ? encoder_init_probe_legacy : NULL;
	probe.encoder.orig_info.get_defaults2 = callbacks & 2 ? encoder_init_probe_modern : NULL;
	probe.encoder.orig_info.type_data = &probe.value;
	encoder_init_current = &probe;
	bool result = rust ? encoder_init_bridge(&probe.encoder, (const char *)&probe, (obs_data_t *)&probe.value,
						 (obs_data_t *)&probe.order)
			   : encoder_init_original(&probe.encoder, (const char *)&probe, (obs_data_t *)&probe.value,
						   (obs_data_t *)&probe.order);
	out[0] = result;
	out[1] = probe.forwarded;
	out[2] = probe.value;
	out[3] = probe.order;
	out[4] = (uint32_t)probe.count;
	for (size_t i = 0; i < 16; i++)
		out[5 + i] = probe.events[i];
	pthread_mutex_t *locks[] = {&probe.encoder.init_mutex,    &probe.encoder.callbacks_mutex,
				    &probe.encoder.outputs_mutex, &probe.encoder.pause.mutex,
				    &probe.encoder.roi_mutex,     &probe.encoder.last_error_mutex};
	for (size_t i = 0; i < 6; i++)
		out[21 + i] = *(const unsigned char *)locks[i];
	encoder_init_current = NULL;
}

void oracle_encoder_init_layout(size_t *out);
void oracle_encoder_init_layout(size_t *out)
{
	out[0] = sizeof(struct obs_encoder_init_fields);
	out[1] = _Alignof(struct obs_encoder_init_fields);
	out[2] = offsetof(struct obs_encoder_init_fields, sentinel);
	out[3] = offsetof(struct obs_encoder_init_fields, context);
	out[4] = offsetof(struct obs_encoder_init_fields, mutex);
	out[5] = offsetof(struct obs_encoder_init_fields, defaults);
	out[6] = offsetof(struct obs_encoder_init_fields, defaults2);
}
