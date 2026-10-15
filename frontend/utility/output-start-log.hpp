#pragma once

#include <util/base.h>
#include <util/c99defs.h>

/* Logs that a start of an output (streaming, recording, replay buffer,
 * virtual camera) was refused because the outputs are disabled. Only the
 * Settings dialog and the auto-configuration wizard disable them, and on an
 * unattended host nobody sees those (obs-rust/obs-studio#817). Kept free of Qt
 * so it can be unit tested. */
inline void LogOutputStartRefused(const char *output)
{
	blog(LOG_WARNING,
	     "%s not started: outputs are disabled while the Settings dialog or the "
	     "Auto-Configuration Wizard is open",
	     output);
}
