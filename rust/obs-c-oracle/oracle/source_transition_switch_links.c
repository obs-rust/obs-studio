/* Test-only link stand-in: obs_source_enum_full_tree() under libobs's own
 * name, for the test binaries that link the obs-source-seam shims without
 * libobs.
 *
 * The transition start and set shims
 * (rust/obs-source-seam/src/ffi/source_transition_switch.rs) export
 * obs_transition_start() and obs_transition_set(), whose libobs calls name
 * it. The MSVC, MinGW and macOS linkers want every symbol an object names
 * resolved, used or not, so obs-tests-core needs a definition; libobs has
 * the real one. obs_source_get_ref() and obs_source_release()
 * (oracle/view.c), the active-child add and remove
 * (oracle/source_active_child_links.c), obs_source_get_name() (the
 * obs-missing-files tests), os_gettime_ns() and the seam's accessors
 * (oracle/source_transition_settings.c) already have theirs, and the shims
 * define obs_transition_set(), which the transition access shims call.
 *
 * The tests never reach it: they run the shims with Calls of their own. */

#include "obs.h"

void obs_source_enum_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(enum_callback);
	UNUSED_PARAMETER(param);
}
