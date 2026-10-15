#pragma once

/* The unclean-shutdown step of OBSApp::checkForUncleanShutdown, kept free of
 * Qt so it can be unit tested (obs-rust/obs-studio#809). */

struct UncleanLaunchDecision {
	/* Show the "Crash Detected" dialog (Safe Mode or normal launch). */
	bool showPrompt;
	/* The prompt was skipped on request: log it, launch in normal mode and
	 * do not upload a crash log. */
	bool logSkippedPrompt;
};

inline UncleanLaunchDecision decideUncleanLaunch(bool hasUncleanShutdown, bool disableShutdownCheck)
{
	if (!hasUncleanShutdown) {
		return {false, false};
	}

	return {!disableShutdownCheck, disableShutdownCheck};
}
