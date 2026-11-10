/* Original timer bodies, real private backend table, per-thread fixture. */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#define gs_timer_destroy oracle_gs_timer_destroy
#define gs_timer_begin oracle_gs_timer_begin
#define gs_timer_end oracle_gs_timer_end
#define gs_timer_get_data oracle_gs_timer_get_data
#include "graphics/graphics-internal.h"
#include "util/base.h"
static THREAD_LOCAL struct graphics_subsystem td_graphics;
static THREAD_LOCAL graphics_t *td_context;
static graphics_t *td_get_context(void)
{
	return td_context;
}
static THREAD_LOCAL char td_log[160], td_events[8];
static THREAD_LOCAL int td_level;
static THREAD_LOCAL bool td_result, td_write;
static THREAD_LOCAL uint64_t td_value;
static THREAD_LOCAL uint64_t *td_expected_output;
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void td_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	td_level = level;
	vsnprintf(td_log, sizeof(td_log), format, args);
	va_end(args);
}
#define gs_get_context td_get_context
#define blog td_blog
#include "timer-dispatch.inc"
#include "timer-dispatch-fields.inc"
#undef blog
#undef gs_get_context
static void td_event(gs_timer_t *timer, char event)
{
	if (timer != (gs_timer_t *)&td_graphics) {
		td_events[0] = '!';
		return;
	}
	size_t n = strlen(td_events);
	td_events[n] = event;
	td_events[n + 1] = 0;
}
static void td_destroy(gs_timer_t *timer)
{
	td_event(timer, 'D');
}
static void td_begin(gs_timer_t *timer)
{
	td_event(timer, 'B');
}
static void td_end(gs_timer_t *timer)
{
	td_event(timer, 'E');
}
static bool td_data(gs_timer_t *timer, uint64_t *ticks)
{
	td_event(timer, 'G');
	if (ticks != td_expected_output)
		td_events[0] = '!';
	if (td_write)
		*ticks = td_value;
	return td_result;
}
bool oracle_td_run(bool context, bool timer, bool ticks, unsigned op, bool result, bool write, uint64_t value,
		   uint64_t *output);
bool oracle_td_run(bool context, bool timer, bool ticks, unsigned op, bool result, bool write, uint64_t value,
		   uint64_t *output)
{
	memset(&td_graphics, 0, sizeof(td_graphics));
	td_graphics.exports.gs_timer_destroy = td_destroy;
	td_graphics.exports.gs_timer_begin = td_begin;
	td_graphics.exports.gs_timer_end = td_end;
	td_graphics.exports.gs_timer_get_data = td_data;
	td_context = context ? &td_graphics : NULL;
	td_log[0] = td_events[0] = 0;
	td_level = 0;
	td_result = result;
	td_write = write;
	td_value = value;
	td_expected_output = output;
	gs_timer_t *handle = timer ? (gs_timer_t *)&td_graphics : NULL;
	if (op == 3)
		return oracle_gs_timer_get_data(handle, ticks ? output : NULL);
	if (op == 0)
		oracle_gs_timer_destroy(handle);
	if (op == 1)
		oracle_gs_timer_begin(handle);
	if (op == 2)
		oracle_gs_timer_end(handle);
	return false;
}
const char *oracle_td_log(void);
const char *oracle_td_log(void)
{
	return td_log;
}
const char *oracle_td_events(void);
const char *oracle_td_events(void)
{
	return td_events;
}
int oracle_td_level(void);
int oracle_td_level(void)
{
	return td_level;
}
#undef gs_timer_destroy
#undef gs_timer_begin
#undef gs_timer_end
#undef gs_timer_get_data

void *oracle_td_timer(void);
void *oracle_td_timer(void)
{
	return &td_graphics;
}
