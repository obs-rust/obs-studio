#include "libobs/build/libobs.pch.h"

char *const *obs_core_global_locale_field(void)
{
	return &obs->locale;
}

signal_handler_t *const *obs_core_global_signals_field(void)
{
	return &obs->signals;
}

proc_handler_t *const *obs_core_global_procs_field(void)
{
	return &obs->procs;
}
