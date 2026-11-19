/* Original C and real adapter over a thread-local recording backend. */
#include <assert.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#define gs_samplerstate_create oracle_gs_samplerstate_create
#define gs_samplerstate_destroy oracle_gs_samplerstate_destroy
#include "graphics/graphics-internal.h"
#include "util/base.h"

gs_samplerstate_t *oracle_gs_samplerstate_create(const struct gs_sampler_info *info);
void oracle_gs_samplerstate_destroy(gs_samplerstate_t *samplerstate);
static THREAD_LOCAL struct graphics_subsystem sl_graphics;
static THREAD_LOCAL int sl_device, sl_tokens[2];
static THREAD_LOCAL struct gs_sampler_info sl_infos[2], sl_before[2];
static THREAD_LOCAL const struct gs_sampler_info *sl_expected_info;
static THREAD_LOCAL gs_samplerstate_t *sl_result, *sl_expected_sampler;
static THREAD_LOCAL unsigned sl_calls[2], sl_logs, sl_event_count;
static THREAD_LOCAL int sl_level;
static THREAD_LOCAL char sl_log[160], sl_events[128];
static void sl_record(char event)
{
	assert(sl_event_count + 1 < sizeof(sl_events));
	sl_events[sl_event_count++] = event;
	sl_events[sl_event_count] = '\0';
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void sl_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	sl_logs++;
	sl_level = level;
	vsnprintf(sl_log, sizeof(sl_log), format, args);
	va_end(args);
	sl_record('L');
}
#define blog sl_blog
#include "graphics-samplerstate-lifecycle.inc"
/* Original validators from graphics.c.h, verbatim. */
static inline bool gs_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}
static inline bool gs_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}

	return true;
}
static graphics_t *sl_context(void)
{
	return thread_graphics;
}
#define gs_get_context sl_context
#include "graphics-samplerstate-lifecycle-fields.inc"
#undef gs_get_context
#undef blog

static gs_samplerstate_t *sl_create(gs_device_t *device, const struct gs_sampler_info *info)
{
	assert(thread_graphics == &sl_graphics);
	assert(device == (gs_device_t *)&sl_device);
	assert(info == sl_expected_info);
	sl_calls[0]++;
	sl_record('C');
	return sl_result;
}
static void sl_destroy(gs_samplerstate_t *sampler)
{
	assert(thread_graphics == &sl_graphics);
	assert(sampler == sl_expected_sampler);
	sl_calls[1]++;
	sl_record('D');
}
void oracle_samplerstate_reset(void);
void oracle_samplerstate_reset(void)
{
	memset(&sl_graphics, 0, sizeof(sl_graphics));
	sl_graphics.device = (gs_device_t *)&sl_device;
	memset(sl_infos, 0, sizeof(sl_infos));
	memset(sl_before, 0, sizeof(sl_before));
	memset(sl_calls, 0, sizeof(sl_calls));
	sl_logs = sl_event_count = 0;
	sl_level = 0;
	sl_log[0] = sl_events[0] = '\0';
	thread_graphics = NULL;
}
const struct gs_sampler_info *oracle_samplerstate_info(unsigned slot, unsigned filter, unsigned u, unsigned v,
						       unsigned w, int anisotropy, uint32_t border);
const struct gs_sampler_info *oracle_samplerstate_info(unsigned slot, unsigned filter, unsigned u, unsigned v,
						       unsigned w, int anisotropy, uint32_t border)
{
	assert(slot < 2 && filter <= GS_FILTER_MIN_MAG_LINEAR_MIP_POINT);
	assert(u <= GS_ADDRESS_MIRRORONCE && v <= GS_ADDRESS_MIRRORONCE && w <= GS_ADDRESS_MIRRORONCE);
	sl_infos[slot] = (struct gs_sampler_info){(enum gs_sample_filter)filter,
						  (enum gs_address_mode)u,
						  (enum gs_address_mode)v,
						  (enum gs_address_mode)w,
						  anisotropy,
						  border};
	memcpy(&sl_before[slot], &sl_infos[slot], sizeof(sl_infos[slot]));
	return &sl_infos[slot];
}
gs_samplerstate_t *oracle_samplerstate_token(unsigned slot);
gs_samplerstate_t *oracle_samplerstate_token(unsigned slot)
{
	assert(slot < 2);
	return (gs_samplerstate_t *)&sl_tokens[slot];
}
void oracle_samplerstate_configure(bool context, unsigned callbacks, const struct gs_sampler_info *info,
				   gs_samplerstate_t *result, gs_samplerstate_t *sampler);
void oracle_samplerstate_configure(bool context, unsigned callbacks, const struct gs_sampler_info *info,
				   gs_samplerstate_t *result, gs_samplerstate_t *sampler)
{
	thread_graphics = context ? &sl_graphics : NULL;
	sl_graphics.exports.device_samplerstate_create = (callbacks & 1) ? sl_create : NULL;
	sl_graphics.exports.gs_samplerstate_destroy = (callbacks & 2) ? sl_destroy : NULL;
	sl_expected_info = info;
	sl_result = result;
	sl_expected_sampler = sampler;
}
unsigned oracle_samplerstate_calls(unsigned index);
unsigned oracle_samplerstate_calls(unsigned index)
{
	assert(index < 2);
	return sl_calls[index];
}
unsigned oracle_samplerstate_logs(void);
unsigned oracle_samplerstate_logs(void)
{
	return sl_logs;
}
int oracle_samplerstate_level(void);
int oracle_samplerstate_level(void)
{
	return sl_level;
}
const char *oracle_samplerstate_log(void);
const char *oracle_samplerstate_log(void)
{
	return sl_log;
}
const char *oracle_samplerstate_events(void);
const char *oracle_samplerstate_events(void)
{
	return sl_events;
}
bool oracle_samplerstate_unchanged(void);
bool oracle_samplerstate_unchanged(void)
{
	return memcmp(sl_infos, sl_before, sizeof(sl_infos)) == 0;
}
#undef gs_samplerstate_create
#undef gs_samplerstate_destroy
