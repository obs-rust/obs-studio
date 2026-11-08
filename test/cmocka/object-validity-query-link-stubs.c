/* Standalone Rust-static consumers never call this unrelated context query.
 * Fail loudly if that contract changes; the actual library uses its real adapter. */
#include <stdlib.h>

void *const *obs_object_validity_field(const void *obj);
void *const *obs_object_validity_field(const void *obj)
{
	(void)obj;
	abort();
}
