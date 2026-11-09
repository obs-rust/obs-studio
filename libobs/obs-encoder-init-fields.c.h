#include "libobs/build/libobs.pch.h"

static pthread_mutex_t *encoder_init_mutex(void *opaque, uint32_t index)
{
	struct obs_encoder *encoder = opaque;
	switch (index) {
	case 0:
		return &encoder->init_mutex;
	case 1:
		return &encoder->callbacks_mutex;
	case 2:
		return &encoder->outputs_mutex;
	case 3:
		return &encoder->pause.mutex;
	case 4:
		return &encoder->roi_mutex;
	default: /* The safe core only supplies 0..5. */
		return &encoder->last_error_mutex;
	}
}

static void encoder_init_sentinel(void *encoder, uint32_t index)
{
	pthread_mutex_init_value(encoder_init_mutex(encoder, index));
}

static bool encoder_init_context(void *opaque, const char *name, void *settings, void *hotkeys)
{
	struct obs_encoder *encoder = opaque;
	return obs_context_data_init(&encoder->context, OBS_OBJ_TYPE_ENCODER, settings, name, NULL, hotkeys, false);
}

static bool encoder_init_lock(void *encoder, uint32_t index, bool recursive)
{
	pthread_mutex_t *mutex = encoder_init_mutex(encoder, index);
	return (recursive ? pthread_mutex_init_recursive(mutex) : pthread_mutex_init(mutex, NULL)) == 0;
}

static void encoder_init_defaults(void *opaque)
{
	struct obs_encoder *encoder = opaque;
	if (encoder->orig_info.get_defaults)
		encoder->orig_info.get_defaults(encoder->context.settings);
}

static void encoder_init_defaults2(void *opaque)
{
	struct obs_encoder *encoder = opaque;
	if (encoder->orig_info.get_defaults2)
		encoder->orig_info.get_defaults2(encoder->context.settings, encoder->orig_info.type_data);
}

static bool init_encoder(struct obs_encoder *encoder, const char *name, obs_data_t *settings, obs_data_t *hotkey_data)
{
	const struct obs_encoder_init_fields fields = {
		encoder_init_sentinel, encoder_init_context,   encoder_init_lock,
		encoder_init_defaults, encoder_init_defaults2,
	};
	return obs_encoder_init_steps(encoder, name, settings, hotkey_data, &fields);
}
