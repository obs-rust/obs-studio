/* Test-only link stand-ins: obs_source_get_color_space() and the graphics
 * call gs_ortho()
 * under libobs's own names,
 * for the test binaries that link the obs-source-seam shims without
 * libobs.
 *
 * The transition video render shims
 * (rust/obs-source-seam/src/ffi/source_transition_video.rs) export the four
 * obs_transition_video_* functions, whose libobs calls name these. The
 * MSVC, MinGW and macOS linkers want every symbol an object names
 * resolved, used or not, so obs-tests-core needs a definition; libobs has
 * the real ones. The texrender calls, gs_get_color_space(),
 * obs_source_video_render(), obs_source_active()
 * (oracle/audio_callback_links.c) and the reference calls already have
 * theirs. Blend-state push/pop, gs_blend_function(), gs_clear(),
 * gs_matrix_push(), gs_matrix_mul() and gs_matrix_pop() now come from the
 * real obs-graphics shims.
 *
 * The tests never reach these: they run the shims with Calls of their own. */

#include "obs.h"

enum gs_color_space obs_source_get_color_space(obs_source_t *source, size_t count,
					       const enum gs_color_space *preferred_spaces)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(count);
	UNUSED_PARAMETER(preferred_spaces);
	return GS_CS_SRGB;
}

void gs_ortho(float left, float right, float top, float bottom, float znear, float zfar)
{
	UNUSED_PARAMETER(left);
	UNUSED_PARAMETER(right);
	UNUSED_PARAMETER(top);
	UNUSED_PARAMETER(bottom);
	UNUSED_PARAMETER(znear);
	UNUSED_PARAMETER(zfar);
}
