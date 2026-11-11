#include "libobs/build/libobs.pch.h"

/* Reference-only copy of the original translation-unit-local signal helper. */
static void signal_parent(obs_scene_t *parent, const char *command, calldata_t *params)
{
	calldata_set_ptr(params, "scene", parent);
	signal_handler_signal(parent->source->context.signals, command, params);
}
