/* test_hotkey_dispatch_rust: the real private adapters of the hotkey
 * dispatch, bound to the unchanged characterization's own core. One unity
 * group includes this after the test, so current_core is the test's own,
 * exactly as its non-external branch binds the original. */
#define obs current_core
#include "libobs/obs-hotkey-dispatch-fields.c.h"
#undef obs
