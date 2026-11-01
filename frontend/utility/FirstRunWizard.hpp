#pragma once

/* Whether OBS opens the Auto-Configuration Wizard on its first launch.
 * A launch that asks to start an output (--startstreaming, --startrecording,
 * --startreplaybuffer, --startvirtualcam) is automated: the wizard would sit
 * modal beside that output's own failure box, and neither could be dismissed
 * (#3325). The wizard stays in the Tools menu. */
inline bool ShowFirstRunWizard(bool firstRun, bool hasLastVersion, bool outputsActive, bool startRequested)
{
	(void)startRequested;
	return !firstRun && !hasLastVersion && !outputsActive;
}
