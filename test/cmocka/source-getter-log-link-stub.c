/* Standalone Rust source-step tests do not link libobs logging. The unrelated
 * getter shims sharing their archive member must never call it. */
#include <stdlib.h>
#include "util/base.h"

void blog(int level, const char *format, ...)
{
	(void)level;
	(void)format;
	abort();
}
