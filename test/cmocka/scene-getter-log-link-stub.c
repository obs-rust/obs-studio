/* Pure scene-rule tests link the production Rust archive without starting OBS.
 * The same archive member now contains public getters that reference blog.
 * Their logging is covered by actual-library and oracle tests. These rule tests
 * must never reach that dependency: fail if they unexpectedly call it. */
#include <stdlib.h>
#include <util/base.h>

void blog(int level, const char *format, ...)
{
	(void)level;
	(void)format;
	abort();
}
