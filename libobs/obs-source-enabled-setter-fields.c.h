#include "libobs/build/libobs.pch.h"

bool *obs_source_enabled_setter_field(obs_source_t *source)
{
	return &source->enabled;
}

signal_handler_t *obs_source_enabled_setter_signals(obs_source_t *source)
{
	return source->context.signals;
}
