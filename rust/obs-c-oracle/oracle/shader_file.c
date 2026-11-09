/* Original C wrapper, with recording dependencies; no file-wrapper logic copied. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include "util/c99defs.h"
#include "util/base.h"
#include "util/threading.h"

typedef void gs_shader_t;
gs_shader_t *oracle_gs_vertexshader_create_from_file(const char *file, char **error_string);
gs_shader_t *oracle_gs_pixelshader_create_from_file(const char *file, char **error_string);
struct shader_file_trace {
	int context, path, readable, reject, output;
	int result, error, reads, vertex, pixel, frees, logs, level, identity;
	char log[256];
};
static THREAD_LOCAL struct shader_file_trace *sf_trace;
static THREAD_LOCAL char sf_source[] = "// UTF-8: \xc3\xa9\nvoid main() {}\n";
static THREAD_LOCAL char sf_path[] = "source.shader";
static THREAD_LOCAL char sf_error;
static THREAD_LOCAL char **sf_output;

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void sf_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	sf_trace->logs++;
	sf_trace->level = level;
	vsnprintf(sf_trace->log, sizeof(sf_trace->log), format, args);
	va_end(args);
}
static bool shader_file_valid(const char *name)
{
	if (!sf_trace->context)
		sf_blog(LOG_DEBUG, "%s: called while not in a graphics context", name);
	return sf_trace->context != 0;
}
static bool shader_file_obj_valid(const void *ptr, const char *name, const char *parameter)
{
	if (!ptr)
		sf_blog(LOG_DEBUG, "%s: Null '%s' parameter", name, parameter);
	return ptr != NULL;
}
static char *sf_read(const char *path)
{
	sf_trace->reads++;
	sf_trace->identity &= path == sf_path;
	return sf_trace->readable ? sf_source : NULL;
}
static void sf_free(void *source)
{
	sf_trace->frees++;
	sf_trace->identity &= source == sf_source;
}
static gs_shader_t *sf_create(const char *source, const char *path, char **error)
{
	sf_trace->identity &= source == sf_source && path == sf_path && error == sf_output;
	if (sf_trace->reject) {
		if (error)
			*error = &sf_error;
		return NULL;
	}
	return (gs_shader_t *)&sf_source;
}
static gs_shader_t *sf_vertex(const char *source, const char *path, char **error)
{
	sf_trace->vertex++;
	return sf_create(source, path, error);
}
static gs_shader_t *sf_pixel(const char *source, const char *path, char **error)
{
	sf_trace->pixel++;
	return sf_create(source, path, error);
}
#define gs_vertexshader_create_from_file oracle_gs_vertexshader_create_from_file
#define gs_pixelshader_create_from_file oracle_gs_pixelshader_create_from_file
#define os_quick_read_utf8_file sf_read
#define gs_vertexshader_create sf_vertex
#define gs_pixelshader_create sf_pixel
#define bfree sf_free
#define blog sf_blog
#include "shader-file.inc"
#undef blog
#undef bfree
#undef gs_pixelshader_create
#undef gs_vertexshader_create
#undef os_quick_read_utf8_file
#undef gs_pixelshader_create_from_file
#undef gs_vertexshader_create_from_file

void oracle_shader_file_run(struct shader_file_trace *trace, int pixel);
void oracle_shader_file_run(struct shader_file_trace *trace, int pixel)
{
	sf_trace = trace;
	trace->identity = 1;
	char *error = trace->error ? &sf_error : NULL;
	sf_output = trace->output ? &error : NULL;
	const char *path = trace->path ? sf_path : NULL;
	gs_shader_t *result = pixel ? oracle_gs_pixelshader_create_from_file(path, sf_output)
				    : oracle_gs_vertexshader_create_from_file(path, sf_output);
	trace->result = result != NULL;
	trace->error = error != NULL;
	sf_trace = NULL;
}

/* A second inclusion compares the actual Rust ABI shim with original C using
 * the shared real file reader and existing recording shader constructors. */
void *gs_get_context(void);
void oracle_shader_file_context(bool present);
char *os_quick_read_utf8_file(const char *path);
void bfree(void *data);
gs_shader_t *gs_vertexshader_create(const char *source, const char *file, char **error);
gs_shader_t *gs_pixelshader_create(const char *source, const char *file, char **error);
gs_shader_t *oracle_live_vertexshader_create_from_file(const char *file, char **error);
gs_shader_t *oracle_live_pixelshader_create_from_file(const char *file, char **error);
static THREAD_LOCAL bool sf_live_context;
void *gs_get_context(void)
{
	return sf_live_context ? &sf_live_context : NULL;
}
void oracle_shader_file_context(bool present)
{
	sf_live_context = present;
}
static bool sf_live_valid(const char *name)
{
	if (!gs_get_context()) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", name);
		return false;
	}
	return true;
}
static bool sf_live_obj_valid(const void *ptr, const char *name, const char *parameter)
{
	if (!ptr) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", name, parameter);
		return false;
	}
	return true;
}
#define shader_file_valid sf_live_valid
#define shader_file_obj_valid sf_live_obj_valid
#define gs_vertexshader_create_from_file oracle_live_vertexshader_create_from_file
#define gs_pixelshader_create_from_file oracle_live_pixelshader_create_from_file
#include "shader-file.inc"
#undef gs_pixelshader_create_from_file
#undef gs_vertexshader_create_from_file
#undef shader_file_obj_valid
#undef shader_file_valid
