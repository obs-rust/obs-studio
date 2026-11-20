/* test_hotkey_background_state_rust: the real private adapters of the
 * background-state query, bound to the unchanged characterization's own
 * recording core and lock pair. One unity group includes this after the
 * test, so record_core(), lock() and unlock() are the test's own, exactly as
 * its non-external branch binds the original. */
#define obs record_core()
#include "libobs/obs-hotkey-background-state-fields.c.h"
#undef obs
