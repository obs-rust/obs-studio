#include "libobs/build/libobs.pch.h"

/* The full reset the legacy obs_reset_audio hands its two fields to. It
 * owns the gates, the defaults and the reset message. */
typedef bool (*obs_reset_audio_reset2_fn)(const struct obs_audio_info2 *);

obs_reset_audio_reset2_fn obs_reset_audio_fields(void);
obs_reset_audio_reset2_fn obs_reset_audio_fields(void)
{
	return obs_reset_audio2;
}
