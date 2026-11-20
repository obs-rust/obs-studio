/* Original output name getter and its production context-name adapter, over
 * the real private output layout.
 *
 * The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test. _NO_CRT_STDIO_INLINE keeps them declarations only, and the
 * log capture below reads its arguments without formatting them. */
#define _NO_CRT_STDIO_INLINE
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <string.h>
#define obs_output_get_name oracle_obs_output_get_name
#include "obs-output-get-name.c.h"
#undef obs_output_get_name
#include "obs-output-get-name-fields.c.h"

struct ogn_log {
	unsigned count;
	int level;
	const char *format;
	const char *function;
	const char *parameter;
};

static void ogn_capture(int level, const char *format, va_list args, void *param)
{
	struct ogn_log *log = param;
	log->count++;
	log->level = level;
	log->format = format;
	log->function = va_arg(args, const char *);
	log->parameter = va_arg(args, const char *);
}

static bool ogn_exact_null_log(const struct ogn_log *log)
{
	return log->count == 1 && log->level == LOG_DEBUG && strcmp(log->format, "%s: Null '%s' parameter") == 0 &&
	       strcmp(log->function, "obs_output_get_name") == 0 && strcmp(log->parameter, "output") == 0;
}

typedef const char *(*output_name_getter)(const obs_output_t *output);

/* The original runs on one capture and the candidate on another: same
 * borrowed pointer, and either no log (a live output) or exactly
 * obs_output_valid()'s diagnostic (NULL). */
static bool ogn_compare(const obs_output_t *output, output_name_getter candidate)
{
	struct ogn_log original = {0}, rust = {0};
	base_set_log_handler(ogn_capture, &original);
	const char *expected = oracle_obs_output_get_name(output);
	base_set_log_handler(ogn_capture, &rust);
	const char *actual = candidate(output);
	if (actual != expected)
		return false;
	if (output)
		return original.count == 0 && rust.count == 0;
	return !actual && ogn_exact_null_log(&original) && ogn_exact_null_log(&rust);
}

static bool ogn_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		return false;
	memcpy(dst, src, len + 1);
	return true;
}

/* NULL, then a live real-layout output read twice, then the same output
 * after its context-name slot changes. The output's bytes must be untouched
 * by every read, and the production adapter must address the original's
 * field. */
bool oracle_output_get_name_probe(output_name_getter candidate, const char *name, const char *renamed, bool null_name);
bool oracle_output_get_name_probe(output_name_getter candidate, const char *name, const char *renamed, bool null_name)
{
	log_handler_t saved_handler;
	void *saved_param;
	char first[256], second[256];
	if (!ogn_copy(first, sizeof(first), name) || !ogn_copy(second, sizeof(second), renamed))
		return false;

	base_get_log_handler(&saved_handler, &saved_param);
	static struct obs_output output;
	memset(&output, 0, sizeof(output));
	output.context.name = null_name ? NULL : first;
	static unsigned char before[sizeof(output)];
	memcpy(before, &output, sizeof(output));
	bool equal = ogn_compare(NULL, candidate) && ogn_compare(&output, candidate) &&
		     ogn_compare(&output, candidate) && memcmp(before, &output, sizeof(output)) == 0 &&
		     obs_output_get_name_context_name(&output) == output.context.name;
	output.context.name = second;
	memcpy(before, &output, sizeof(output));
	equal = equal && ogn_compare(&output, candidate) && memcmp(before, &output, sizeof(output)) == 0 &&
		obs_output_get_name_context_name(&output) == second;
	base_set_log_handler(saved_handler, saved_param);
	return equal;
}
