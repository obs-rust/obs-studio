/* Test-only stand-in for the graphics functions the effect parser's compile
 * step calls (effect-parser-compile.c.h): gs_vertexshader_create,
 * gs_pixelshader_create and gs_shader_get_param_by_name, with
 * gs_shader_destroy for effect_free(). The Rust shim reaches them through
 * its adapters (obs_graphics_vertexshader_create and the others), which
 * oracle/image_file.c builds into the test binary, and the C oracle
 * (oracle/effect_parser_compile.c) calls them directly, so both sides hit
 * the same stand-in.
 *
 * Each call is written to one event log, with what it answered: the n-th
 * create call (vertex and pixel counted together, from 0) returns NULL when
 * bit n of the null mask is set, and hands back the error text "fail <n>"
 * or "" when bit n of the error or empty-error mask is; a created shader is
 * the tag n + 1. A shader param is a fresh tag from 0x1000, except that the
 * shaders in the missing mask (bit n: the n-th create call's) lack the
 * missing name. The log handler adds every line blog() writes on the
 * recording thread. */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics.h"
#include "util/base.h"
#include "util/bmem.h"

void oracle_compile_device_reset(uint32_t null_mask, uint32_t error_mask, uint32_t empty_mask, uint32_t missing_mask,
				 const char *missing);
void oracle_compile_device_stop(void);
const char *oracle_compile_device_events(size_t *len);
log_handler_t oracle_compile_log_handler_ptr(void);

#define COMPILE_EVENTS_MAX (1 << 18)
static char compile_events[COMPILE_EVENTS_MAX];
static size_t compile_events_len;
static size_t compile_creates;
static size_t compile_sparams;
static uint32_t compile_null_mask, compile_error_mask, compile_empty_mask, compile_missing_mask;
static const char *compile_missing;

/* Only the test thread's lines: other tests in the binary blog too. */
#if defined(_MSC_VER)
static __declspec(thread) bool compile_recording;
#else
static _Thread_local bool compile_recording;
#endif

#if defined(__GNUC__) || defined(__clang__)
#define COMPILE_FORMAT __attribute__((format(printf, 1, 2)))
#else
#define COMPILE_FORMAT
#endif

static void compile_event(const char *fmt, ...) COMPILE_FORMAT;
static void compile_event(const char *fmt, ...)
{
	va_list args;
	int n;

	if (compile_events_len >= COMPILE_EVENTS_MAX - 1)
		return;
	va_start(args, fmt);
	n = vsnprintf(compile_events + compile_events_len, COMPILE_EVENTS_MAX - compile_events_len, fmt, args);
	va_end(args);
	if (n > 0)
		compile_events_len += (size_t)n;
	if (compile_events_len >= COMPILE_EVENTS_MAX)
		compile_events_len = COMPILE_EVENTS_MAX - 1;
}

void oracle_compile_device_reset(uint32_t null_mask, uint32_t error_mask, uint32_t empty_mask, uint32_t missing_mask,
				 const char *missing)
{
	compile_events_len = 0;
	compile_events[0] = 0;
	compile_creates = 0;
	compile_sparams = 0;
	compile_null_mask = null_mask;
	compile_error_mask = error_mask;
	compile_empty_mask = empty_mask;
	compile_missing_mask = missing_mask;
	compile_missing = missing;
	compile_recording = true;
}

void oracle_compile_device_stop(void)
{
	compile_recording = false;
}

const char *oracle_compile_device_events(size_t *len)
{
	*len = compile_events_len;
	return compile_events;
}

static bool compile_bit(uint32_t mask, size_t n)
{
	return n < 32 && (mask & (1u << n)) != 0;
}

static gs_shader_t *compile_create(const char *kind, const char *shader, const char *file, char **error_string)
{
	size_t n = compile_creates++;

	compile_event("create %s %u file=%s errors_in=%s\n", kind, (unsigned)n, file ? file : "(null)",
		      *error_string ? "set" : "null");
	if (shader)
		compile_event("text %u [%s]\n", (unsigned)strlen(shader), shader);
	else
		compile_event("text (null)\n");
	if (compile_bit(compile_error_mask, n)) {
		char text[32];

		snprintf(text, sizeof(text), "fail %u", (unsigned)n);
		*error_string = bstrdup(text);
	} else if (compile_bit(compile_empty_mask, n)) {
		*error_string = bstrdup("");
	}
	if (compile_bit(compile_null_mask, n)) {
		compile_event("  -> NULL\n");
		return NULL;
	}
	compile_event("  -> shader %u\n", (unsigned)(n + 1));
	return (gs_shader_t *)(uintptr_t)(n + 1);
}

gs_shader_t *gs_vertexshader_create(const char *shader, const char *file, char **error_string)
{
	return compile_create("vertex", shader, file, error_string);
}

gs_shader_t *gs_pixelshader_create(const char *shader, const char *file, char **error_string)
{
	return compile_create("pixel", shader, file, error_string);
}

gs_sparam_t *oracle_compile_param_by_name(gs_shader_t *shader, const char *name);
gs_sparam_t *oracle_compile_param_by_name(gs_shader_t *shader, const char *name)
{
	uintptr_t tag = (uintptr_t)shader;

	if (tag >= 1 && compile_bit(compile_missing_mask, tag - 1) && compile_missing && name &&
	    strcmp(name, compile_missing) == 0) {
		compile_event("sparam %u %s -> NULL\n", (unsigned)tag, name);
		return NULL;
	}
	compile_sparams++;
	compile_event("sparam %u %s -> %u\n", (unsigned)tag, name ? name : "(null)",
		      (unsigned)(0x1000 + compile_sparams));
	return (gs_sparam_t *)(uintptr_t)(0x1000 + compile_sparams);
}

void gs_shader_destroy(gs_shader_t *shader)
{
	(void)shader;
}

static void compile_log_handler(int lvl, const char *msg, va_list args, void *p)
{
	char line[1024];

	(void)p;
	if (!compile_recording)
		return;
	/* blog()'s own format and arguments, passed on as a log handler gets
	 * them. */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wformat-nonliteral"
#endif
	vsnprintf(line, sizeof(line), msg, args);
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#endif
	compile_event("log %d %s\n", lvl, line);
}

log_handler_t oracle_compile_log_handler_ptr(void)
{
	return compile_log_handler;
}
