#include "libobs/build/libobs.pch.h"

signal_handler_t *obs_source_get_signal_handler(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_signal_handler") ? source->context.signals : NULL;
}
