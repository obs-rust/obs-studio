#pragma once

#include "obs.h"

/* Private seam: preserves the existing signal builder and synchronous dispatch. */
void obs_source_push_to_enable_signal_changed(obs_source_t *source, const char *signal, bool enabled);

/* Borrowed fields: read and write only while holding audio_mutex. */
struct obs_source_push_to_enable_fields {
	void *audio_mutex;
	bool *mute_enabled;
	bool *talk_enabled;
	const uint32_t *output_flags;
	char *const *name;
	void (*signal_changed)(obs_source_t *, const char *, bool);
};
struct obs_source_push_to_enable_fields obs_source_push_to_enable_fields(obs_source_t *source);
