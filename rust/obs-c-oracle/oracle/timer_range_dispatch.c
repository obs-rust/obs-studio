/* Original timer-range bodies, real private backend table, per-thread fixture. */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#define gs_timer_range_destroy oracle_gs_timer_range_destroy
#define gs_timer_range_begin oracle_gs_timer_range_begin
#define gs_timer_range_end oracle_gs_timer_range_end
#define gs_timer_range_get_data oracle_gs_timer_range_get_data
#include "graphics/graphics-internal.h"
#include "util/base.h"
static THREAD_LOCAL struct graphics_subsystem trd_graphics;
static THREAD_LOCAL graphics_t *trd_context;
static graphics_t *trd_get_context(void)
{
	return trd_context;
}
static THREAD_LOCAL char trd_log[160], trd_events[8];
static THREAD_LOCAL int trd_level;
static THREAD_LOCAL bool trd_result, trd_disjoint;
static THREAD_LOCAL unsigned trd_writes;
static THREAD_LOCAL uint64_t trd_value;
static THREAD_LOCAL uint64_t *trd_expected_frequency;
static THREAD_LOCAL bool *trd_expected_disjoint;
static THREAD_LOCAL gs_timer_range_t *trd_expected_range;
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void trd_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	trd_level = level;
	vsnprintf(trd_log, sizeof(trd_log), format, args);
	va_end(args);
}
#define gs_get_context trd_get_context
#define blog trd_blog
#include "timer-range-dispatch.inc"
#include "timer-range-dispatch-fields.inc"
#undef blog
#undef gs_get_context
static void trd_event(gs_timer_range_t *timer, char event)
{
	if (timer != trd_expected_range) {
		trd_events[0] = '!';
		return;
	}
	size_t n = strlen(trd_events);
	trd_events[n] = event;
	trd_events[n + 1] = 0;
}
static void trd_destroy(gs_timer_range_t *timer)
{
	trd_event(timer, 'D');
}
static bool trd_begin(gs_timer_range_t *timer)
{
	trd_event(timer, 'B');
	return trd_result;
}
static bool trd_end(gs_timer_range_t *timer)
{
	trd_event(timer, 'E');
	return trd_result;
}
static bool trd_data(gs_timer_range_t *range, bool *disjoint, uint64_t *frequency)
{
	trd_event(range, 'G');
	if (disjoint != trd_expected_disjoint || frequency != trd_expected_frequency)
		trd_events[0] = '!';
	if (trd_writes & 1)
		*disjoint = trd_disjoint;
	if (trd_writes & 2)
		*frequency = trd_value;
	return trd_result;
}
bool oracle_trd_run(bool context, bool range, bool disjoint, bool frequency, unsigned op, bool result, unsigned writes,
		    bool value_disjoint, uint64_t value, bool *out_disjoint, uint64_t *out_frequency);
bool oracle_trd_run(bool context, bool range, bool disjoint, bool frequency, unsigned op, bool result, unsigned writes,
		    bool value_disjoint, uint64_t value, bool *out_disjoint, uint64_t *out_frequency)
{
	memset(&trd_graphics, 0, sizeof(trd_graphics));
	trd_graphics.exports.gs_timer_range_destroy = trd_destroy;
	trd_graphics.exports.gs_timer_range_begin = trd_begin;
	trd_graphics.exports.gs_timer_range_end = trd_end;
	trd_graphics.exports.gs_timer_range_get_data = trd_data;
	trd_context = context ? &trd_graphics : NULL;
	trd_log[0] = trd_events[0] = 0;
	trd_level = 0;
	trd_result = result;
	trd_writes = writes;
	trd_disjoint = value_disjoint;
	trd_value = value;
	trd_expected_disjoint = out_disjoint;
	trd_expected_frequency = out_frequency;
	gs_timer_range_t *handle = range ? (gs_timer_range_t *)&trd_graphics : NULL;
	trd_expected_range = handle;
	if (op == 3)
		return oracle_gs_timer_range_get_data(handle, disjoint ? out_disjoint : NULL,
						      frequency ? out_frequency : NULL);
	if (op == 0)
		oracle_gs_timer_range_destroy(handle);
	if (op == 1)
		oracle_gs_timer_range_begin(handle);
	if (op == 2)
		oracle_gs_timer_range_end(handle);
	return false;
}
const char *oracle_trd_log(void);
const char *oracle_trd_log(void)
{
	return trd_log;
}
const char *oracle_trd_events(void);
const char *oracle_trd_events(void)
{
	return trd_events;
}
int oracle_trd_level(void);
int oracle_trd_level(void)
{
	return trd_level;
}
#undef gs_timer_range_destroy
#undef gs_timer_range_begin
#undef gs_timer_range_end
#undef gs_timer_range_get_data

void *oracle_trd_timer(void);
void *oracle_trd_timer(void)
{
	return &trd_graphics;
}
