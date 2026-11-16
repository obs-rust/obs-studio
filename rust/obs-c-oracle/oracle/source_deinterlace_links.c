/* Test-only link stand-ins for the test binaries that link the
 * obs-source-seam shims without libobs: the deinterlace seam,
 * libobs/obs-source-deinterlace-seam.c.h, compiled here under its own
 * names (the mode and field-order reads, obs_source_deinterlace_fields(),
 * are oracle/source_deinterlace_getters.c's), and do-nothing definitions
 * of the libobs calls the deinterlace shims
 * (rust/obs-source-seam/src/ffi/source_deinterlace.rs) name under libobs's
 * own names: the frame cache, the async texture upload, the effect load,
 * the SDR white level and the framebuffer sRGB calls. The deinterlace
 * texture size is obs-source's own Rust export
 * (rust/obs-source/src/ffi/source_deinterlace_texture.rs).
 *
 * The seam stays C in libobs.core-source, so the shims call it in the test
 * binary as they do in the hybrid libobs. Its core reads go to
 * oracle_deinterlace_obs, renamed from the first include on: NULL until
 * the deinterlace parity test points it at its own core
 * (oracle/source_deinterlace.c).
 *
 * The MSVC, MinGW and macOS linkers want every symbol an object names
 * resolved, used or not, so the test binaries need a definition; libobs has
 * the real ones. obs_enter_graphics(), obs_leave_graphics()
 * (oracle/source_profiler_host.c), gs_get_color_space(), gs_draw_sprite(),
 * gs_texture_destroy(), and the effect, linear sRGB and texrender calls
 * (obs-graphics) already have theirs. The tests never reach the do-nothing
 * ones: they run the shims with Calls of their own. */

#define obs oracle_deinterlace_obs

#include "obs-source-deinterlace-seam.c.h"

#undef obs

struct obs_core *oracle_deinterlace_obs;

void remove_async_frame(obs_source_t *source, struct obs_source_frame *frame)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(frame);
}

bool set_async_texture_size(struct obs_source *source, const struct obs_source_frame *frame)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(frame);
	return false;
}

bool update_async_textures(struct obs_source *source, const struct obs_source_frame *frame,
			   gs_texture_t *tex[MAX_AV_PLANES], gs_texrender_t *texrender)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(frame);
	UNUSED_PARAMETER(tex);
	UNUSED_PARAMETER(texrender);
	return false;
}

void obs_source_release_frame(obs_source_t *source, struct obs_source_frame *frame)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(frame);
}

gs_effect_t *obs_load_effect(gs_effect_t **effect, const char *file)
{
	UNUSED_PARAMETER(file);
	return *effect;
}

float obs_get_video_sdr_white_level(void)
{
	return 300.0f;
}

bool gs_framebuffer_srgb_enabled(void)
{
	return false;
}

void gs_enable_framebuffer_srgb(bool enable)
{
	UNUSED_PARAMETER(enable);
}
