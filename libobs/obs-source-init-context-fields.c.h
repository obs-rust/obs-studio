#include "libobs/build/libobs.pch.h"

bool obs_source_context_initialize(obs_source_t *source, obs_data_t *settings, const char *name, const char *uuid,
				   obs_data_t *hotkeys, bool private)
{
	return obs_context_data_init(&source->context, OBS_OBJ_TYPE_SOURCE, settings, name, uuid, hotkeys, private);
}

bool obs_source_context_register(obs_source_t *source, const char **signals)
{
	return signal_handler_add_array(source->context.signals, signals);
}
