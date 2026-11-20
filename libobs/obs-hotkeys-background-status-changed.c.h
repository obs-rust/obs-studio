#include "libobs/build/libobs.pch.h"

void obs_hotkeys_background_status_changed(void)
{
	if (obs && obs->hotkeys.signals)
		signal_handler_signal(obs->hotkeys.signals, "hotkey_background_status_changed", NULL);
}
