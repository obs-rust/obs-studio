/* Test-only link stand-in: obs_get_source_by_name() under libobs's own name,
 * for the test binaries that link the obs-source-seam shims without libobs.
 *
 * The transition load shim (rust/obs-source-seam/src/ffi/source_transition_restore.rs)
 * exports obs_transition_load(), whose libobs calls name it. The MSVC, MinGW
 * and macOS linkers want every symbol an object names resolved, used or not,
 * so obs-tests-core needs a definition; libobs has the real one.
 * obs_source_release() (oracle/view.c), the active-child add and remove
 * (oracle/source_active_child_links.c) and the obs_data reads (obs-data)
 * already have theirs.
 *
 * The tests never reach it: they run the shim with Calls of their own. */

#include "obs.h"

obs_source_t *obs_get_source_by_name(const char *name)
{
	UNUSED_PARAMETER(name);
	return NULL;
}
