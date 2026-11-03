/* Test-only link stand-ins: obs_source_add_active_child() and
 * obs_source_remove_active_child() under libobs's own names, for the test
 * binaries that link the obs-source-seam shims without libobs.
 *
 * The transition slots shims (rust/obs-source-seam/src/ffi/source_transition_slots.rs)
 * export obs_transition_clear() and the swap, whose libobs calls name these
 * two. The MSVC, MinGW and macOS linkers want every symbol an object names
 * resolved, used or not, so obs-tests-core needs a definition; libobs has the
 * real ones. obs_source_get_ref(), obs_source_release() (oracle/view.c) and
 * obs_source_get_name() (the obs-missing-files tests) already have theirs.
 *
 * The tests never reach these: they run the shims with Calls of their own. */

#include "obs.h"

bool obs_source_add_active_child(obs_source_t *parent, obs_source_t *child)
{
	UNUSED_PARAMETER(parent);
	UNUSED_PARAMETER(child);
	return false;
}

void obs_source_remove_active_child(obs_source_t *parent, obs_source_t *child)
{
	UNUSED_PARAMETER(parent);
	UNUSED_PARAMETER(child);
}
