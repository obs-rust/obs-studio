/* Deterministic collaborators around the unchanged extracted C unit. */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "graphics/effect.h"
#include "util/base.h"

static THREAD_LOCAL struct graphics_subsystem ef_graphics;
static THREAD_LOCAL graphics_t *thread_graphics;
static THREAD_LOCAL struct gs_effect ef_effects[8];
static THREAD_LOCAL char ef_text[64];
static THREAD_LOCAL char ef_events[32];
static THREAD_LOCAL size_t ef_event_count;
static THREAD_LOCAL char ef_log[256];
static THREAD_LOCAL int ef_level;
static THREAD_LOCAL bool ef_read_ok, ef_create_ok, ef_error_forwarded, ef_args_ok;
static THREAD_LOCAL const char *ef_file;
static THREAD_LOCAL char **ef_error;

static void ef_event(char c)
{
	ef_events[ef_event_count++] = c;
	ef_events[ef_event_count] = 0;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void ef_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	vsnprintf(ef_log, sizeof ef_log, format, args);
	va_end(args);
	ef_level = level;
	ef_event('L');
}
static inline bool gs_valid(const char *f)
{
	if (!thread_graphics) {
		ef_blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}
	return true;
}
static inline bool gs_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		ef_blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}
	return true;
}
static char *ef_read(const char *file)
{
	ef_args_ok = ef_args_ok && file == ef_file;
	ef_event('R');
	return ef_read_ok ? ef_text : NULL;
}
static gs_effect_t *ef_create(const char *text, const char *file, char **error)
{
	ef_args_ok = ef_args_ok && text == ef_text && file == ef_file;
	ef_error_forwarded = error == ef_error;
	if (error) {
		*error = ef_create_ok ? NULL : ef_text;
	}
	ef_event('C');
	return ef_create_ok ? &ef_effects[7] : NULL;
}
static void ef_free(void *text)
{
	ef_args_ok = ef_args_ok && text == ef_text;
	ef_event('F');
}
static void ef_reset(bool context, bool read_ok, bool create_ok)
{
	memset(&ef_graphics, 0, sizeof ef_graphics);
	memset(ef_effects, 0, sizeof ef_effects);
	thread_graphics = context ? &ef_graphics : NULL;
	ef_events[0] = ef_log[0] = 0;
	ef_event_count = 0;
	ef_level = 0;
	ef_read_ok = read_ok;
	ef_create_ok = create_ok;
	ef_args_ok = true;
	ef_error_forwarded = false;
	strcpy(ef_text, "\xEF\xBB\xBFUTF-8 \xC3\xA9");
}
static graphics_t *ef_context(void)
{
	return thread_graphics;
}
#define gs_get_context ef_context
#define os_quick_read_utf8_file ef_read
#define gs_effect_create ef_create
#define bfree ef_free
#define blog ef_blog
#define gs_effect_create_from_file ef_from_file
#include "graphics_effect_file.inc"
#undef gs_effect_create_from_file
#undef blog
#undef bfree
#undef gs_effect_create
#undef os_quick_read_utf8_file
#undef gs_get_context
