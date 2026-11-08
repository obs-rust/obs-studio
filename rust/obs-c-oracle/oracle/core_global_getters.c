/* The verbatim C getter and production adapter bodies over a test-owned core.
 * The stand-in names only the three pointer fields; it is not a layout oracle.
 * Tier 2 exercises the adapters with the real private header and real core. */
#include <stddef.h>
#include "callback/signal.h"
#include "callback/proc.h"

struct obs_core {
	char *locale;
	signal_handler_t *signals;
	proc_handler_t *procs;
};
static struct obs_core global_getters_core;
static struct obs_core *obs = &global_getters_core;

#define obs_get_locale oracle_obs_get_locale
#define obs_get_signal_handler oracle_obs_get_signal_handler
#define obs_get_proc_handler oracle_obs_get_proc_handler
#include "obs.h"

char *const *obs_core_global_locale_field(void);
signal_handler_t *const *obs_core_global_signals_field(void);
proc_handler_t *const *obs_core_global_procs_field(void);
void oracle_core_global_set(char *locale, signal_handler_t *signals, proc_handler_t *procs);

#include "obs-core-global-getters.inc"
#include "core_global_fields.inc"

void oracle_core_global_set(char *locale, signal_handler_t *signals, proc_handler_t *procs)
{
	global_getters_core.locale = locale;
	global_getters_core.signals = signals;
	global_getters_core.procs = procs;
}
