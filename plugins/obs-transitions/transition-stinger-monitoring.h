#pragma once

/*
 * Pure decision of the Stinger transition's pre-33.0 audio monitoring
 * migration (obs-rust/obs-studio#2638), kept free of libobs so a test can
 * call it without a display.
 *
 * Before 33.0 the Stinger stored "audio_monitoring" as an int: 0 (off), 1
 * (Monitor Only) or 2 (Monitor and Output). 33.0 replaced it with the boolean
 * "enable_monitoring" and reads nothing else, so a transition saved by an
 * older OBS lost its monitoring on the upgrade: a Monitor Only one lost
 * monitoring and started playing into the stream and recordings, and a
 * transition has no mixer entry or mute to get monitor-only back.
 *
 * The stored value is converted once, while "enable_monitoring" has no user
 * value; a transition 33.0 saved carries one and keeps the choice the user
 * made. Transitions are saved without a version, so a 33.0-saved transition
 * that still has only the old key is read the same way, which restores what
 * the user chose in 32.x.
 */

#include <stdbool.h>

/* The "audio_monitoring" values 33.0 replaced. */
enum stinger_audio_monitoring {
	STINGER_AUDIO_MONITORING_OFF = 0,
	STINGER_AUDIO_MONITORING_MONITOR_ONLY = 1,
	STINGER_AUDIO_MONITORING_MONITOR_AND_OUTPUT = 2,
};

/* What the transition's media source is created with. */
struct stinger_monitoring {
	/* Whether monitoring is on. */
	bool enabled;
	/* Whether the media source is muted. */
	bool mute;
};

/* The monitoring to load a Stinger transition with. has_enable_monitoring is
 * whether the settings carry a user value for "enable_monitoring" (the 33.0
 * key); when they do not, the pre-33.0 "audio_monitoring" value decides: off,
 * Monitor Only (monitoring on and muted) or Monitor and Output (monitoring
 * on). An old value that is not 1 or 2 is read as off, as an unknown value
 * was.
 *
 * Muting a migrated Monitor Only transition keeps it out of every output:
 * 33.0 monitors and outputs a source with monitoring on at once, and the
 * monitor plays a muted source, so the audio is still heard in the monitor,
 * as before 33.0. */
static inline struct stinger_monitoring stinger_monitoring_migration(bool has_enable_monitoring, bool enable_monitoring,
								     long long audio_monitoring)
{
	struct stinger_monitoring plan = {false, false};

	if (has_enable_monitoring) {
		plan.enabled = enable_monitoring;
		return plan;
	}

	if (audio_monitoring == STINGER_AUDIO_MONITORING_MONITOR_ONLY) {
		plan.enabled = true;
		plan.mute = true;
	} else if (audio_monitoring == STINGER_AUDIO_MONITORING_MONITOR_AND_OUTPUT) {
		plan.enabled = true;
	}

	return plan;
}
