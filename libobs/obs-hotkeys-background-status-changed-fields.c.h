#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_hotkeys_background_status_changed(). The
 * core, its hotkey signal handler and signal_handler_signal() stay in C;
 * Rust decides whether to notify and owns the signal name. The handler is
 * read again on every call: a callback may replace it, notify again or
 * remove the core before the notification returns. */
signal_handler_t *obs_hotkeys_background_status_signals(void);
signal_handler_t *obs_hotkeys_background_status_signals(void)
{
	return obs ? obs->hotkeys.signals : NULL;
}

void obs_hotkeys_background_status_signal(signal_handler_t *handler, const char *name);
void obs_hotkeys_background_status_signal(signal_handler_t *handler, const char *name)
{
	signal_handler_signal(handler, name, NULL);
}
