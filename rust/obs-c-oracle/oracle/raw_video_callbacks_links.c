/* Test-only link stand-ins: start_raw_video() and stop_raw_video() under
 * libobs's own (hidden) names, for the test binaries that link the obs-core
 * shims without libobs.
 *
 * The raw video callback shim (rust/obs-core/src/ffi/raw_video_callbacks.rs)
 * exports obs_add_raw_video_callback2() and obs_remove_raw_video_callback(),
 * whose libobs calls name them. The MSVC, MinGW and macOS linkers want every
 * symbol an object names resolved, used or not, so obs-tests-core needs a
 * definition; libobs has the real ones. obs_get_video_fields()
 * (oracle/get_video.c) and blog() already have theirs.
 *
 * The tests never reach these: they run the shim with Calls of their own. */

#include "obs.h"
#include "obs-internal.h"

void start_raw_video(video_t *v, const struct video_scale_info *conversion, uint32_t frame_rate_divisor,
		     void (*callback)(void *param, struct video_data *frame), void *param)
{
	UNUSED_PARAMETER(v);
	UNUSED_PARAMETER(conversion);
	UNUSED_PARAMETER(frame_rate_divisor);
	UNUSED_PARAMETER(callback);
	UNUSED_PARAMETER(param);
}

void stop_raw_video(video_t *v, void (*callback)(void *param, struct video_data *frame), void *param)
{
	UNUSED_PARAMETER(v);
	UNUSED_PARAMETER(callback);
	UNUSED_PARAMETER(param);
}
