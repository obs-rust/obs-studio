/* test_hotkey_key_translation_rust: the real private key-translation adapters,
 * bound to the unchanged characterization's own core. One unity group includes
 * this after the test, so record_core() is the test's own recording reader,
 * exactly as its non-external branch binds the original. */
#define obs record_core()
#include "libobs/obs-hotkey-key-translation-fields.c.h"
#undef obs
