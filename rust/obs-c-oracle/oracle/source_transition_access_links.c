/* Test-only link stand-in: obs_transition_set() under libobs's own name, for
 * the test binaries that link the obs-source-seam shims without libobs.
 *
 * The transition access shims (rust/obs-source-seam/src/ffi/source_transition_access.rs)
 * export obs_transition_set_manual_time(), whose libobs calls name it. The
 * MSVC, MinGW and macOS linkers want every symbol an object names resolved,
 * used or not, so obs-tests-core needs a definition; libobs has the real one.
 * obs_source_get_ref() (oracle/view.c) already has its own.
 *
 * The tests never reach it: they run the shims with Calls of their own. */

#include "obs.h"

void obs_transition_set(obs_source_t *transition, obs_source_t *source)
{
	UNUSED_PARAMETER(transition);
	UNUSED_PARAMETER(source);
}
