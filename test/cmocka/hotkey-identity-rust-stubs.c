/* The Windows-key staticlib harness never calls the hotkey identity getters.
 * Its obs-hotkey archive member still needs this private seam at link time. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_hotkey_identity_fields obs_hotkey_identity_fields(const obs_hotkey_t *key)
{
	(void)key;
	abort();
}
