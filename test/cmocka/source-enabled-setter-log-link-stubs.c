/* Unused obs-util logging services pulled into standalone source-step tests.
 * The fixtures own blog(); reaching either service here is a test failure. */
#include <stdlib.h>
#include "util/base.h"

log_handler_t base_default_log_handler(void);
void base_log_literal(log_handler_t handler, void *param, int level, const char *msg);

log_handler_t base_default_log_handler(void)
{
	abort();
}

void base_log_literal(log_handler_t handler, void *param, int level, const char *msg)
{
	(void)handler;
	(void)param;
	(void)level;
	(void)msg;
	abort();
}
