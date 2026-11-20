/* test_hotkeys_background_status_changed_rust: the real private adapters of
 * the background status notification, bound to the unchanged
 * characterization's own core and recording signal. One unity group
 * includes this after the test, so current_core and record_signal are the
 * test's own, exactly as its non-external branch binds the original. */
#define obs current_core
#define signal_handler_signal record_signal
#include "libobs/obs-hotkeys-background-status-changed-fields.c.h"
#undef signal_handler_signal
#undef obs
