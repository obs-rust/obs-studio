#include "libobs/build/libobs.pch.h"

const char *obs_get_locale(void)
{
	return obs->locale;
}

signal_handler_t *obs_get_signal_handler(void)
{
	return obs->signals;
}

proc_handler_t *obs_get_proc_handler(void)
{
	return obs->procs;
}
