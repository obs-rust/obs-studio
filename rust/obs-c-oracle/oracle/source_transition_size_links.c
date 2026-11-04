/* Test-only link stand-ins: obs_source_get_width() and
 * obs_source_get_height() under libobs's own names, for the test binaries
 * that link the obs-source-seam shims without libobs.
 *
 * The transition sizing shims (rust/obs-source-seam/src/ffi/source_transition_size.rs)
 * export obs_transition_tick() and the size helpers, whose libobs calls name
 * these two. The MSVC, MinGW and macOS linkers want every symbol an object
 * names resolved, used or not, so obs-tests-core needs a definition; libobs
 * has the real ones. gs_texrender_reset() (obs-graphics), add_alignment()
 * (obs-scene), calc_torquef() and the matrix calls (obs-graphics) already
 * have theirs.
 *
 * The tests never reach these: they run the shims with Calls of their own. */

#include "obs.h"

uint32_t obs_source_get_width(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

uint32_t obs_source_get_height(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}
