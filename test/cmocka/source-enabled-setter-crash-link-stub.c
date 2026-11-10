/* Standalone source-step fixtures never invoke obs-util's crash service.
 * Keep this member separate so a fixture's own bcrash definition takes priority. */
#include <stdlib.h>
#include "util/base.h"

void bcrash(const char *format, ...)
{
	(void)format;
	abort();
}
