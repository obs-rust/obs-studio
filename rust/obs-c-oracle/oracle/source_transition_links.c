/* Test-only link stand-ins: obs_source_get_audio_timestamp(),
 * obs_source_get_audio_mix(), gs_enter_context() and gs_leave_context()
 * under libobs's own names, for the test binaries that link the
 * obs-source-seam shims without libobs.
 *
 * The transition audio shims (rust/obs-source-seam/src/ffi/source_transition.rs)
 * export obs_transition_audio_render(), obs_transition_init() and
 * obs_transition_free(), whose libobs calls name these. The MSVC, MinGW and
 * macOS linkers want every symbol an object names resolved, used or not, so
 * obs-tests-core needs a definition; libobs has the real ones.
 * obs_source_get_ref() and obs_source_release() (oracle/view.c),
 * obs_source_removed() (oracle/audio_callback_links.c),
 * gs_texrender_create() and gs_texrender_destroy() (obs-graphics) and the
 * seam's graphics accessor (oracle/source_transition_settings.c) already
 * have theirs.
 *
 * The tests never reach these: they run the shims with Calls of their own. */

#include "obs.h"

uint64_t obs_source_get_audio_timestamp(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

void obs_source_get_audio_mix(const obs_source_t *source, struct obs_source_audio_mix *audio)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(audio);
}

void gs_enter_context(graphics_t *graphics)
{
	UNUSED_PARAMETER(graphics);
}

void gs_leave_context(void) {}
