/* Original legacy caption text wrapper, against a candidate, each over the
 * recording obs_output_output_caption_text2() below.
 *
 * The stand-in has libobs's own name because the Rust shim calls it by that
 * name; obs-output.c.h's real one (the caption queue) is Tier 2's. It never
 * dereferences its arguments.
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
#define obs_output_output_caption_text1 oracle_obs_output_output_caption_text1
#include "obs-output-output-caption-text1.c.h"
#undef obs_output_output_caption_text1

struct oct1_trace {
	unsigned logs;
	int level;
	const char *format;
	const char *function;
	const char *parameter;
	unsigned calls;
	obs_output_t *output;
	const char *text;
	double duration;
};

static struct oct1_trace *oct1_active;

void obs_output_output_caption_text2(obs_output_t *output, const char *text, double display_duration)
{
	struct oct1_trace *trace = oct1_active;
	if (!trace)
		return;
	trace->calls++;
	trace->output = output;
	trace->text = text;
	trace->duration = display_duration;
}

static void oct1_capture(int level, const char *format, va_list args, void *param)
{
	struct oct1_trace *trace = param;
	trace->logs++;
	trace->level = level;
	trace->format = format;
	trace->function = va_arg(args, const char *);
	trace->parameter = va_arg(args, const char *);
}

typedef void (*caption_text1_fn)(obs_output_t *output, const char *text);

static void oct1_run(struct oct1_trace *trace, caption_text1_fn wrapper, obs_output_t *output, const char *text)
{
	memset(trace, 0, sizeof(*trace));
	base_set_log_handler(oct1_capture, trace);
	oct1_active = trace;
	wrapper(output, text);
	oct1_active = NULL;
}

static bool oct1_exact_null_log(const struct oct1_trace *trace)
{
	return trace->logs == 1 && trace->level == LOG_DEBUG && strcmp(trace->format, "%s: Null '%s' parameter") == 0 &&
	       strcmp(trace->function, "obs_output_output_caption_text1") == 0 &&
	       strcmp(trace->parameter, "output") == 0;
}

/* NULL or a static output, NULL or a static text: the original and the
 * candidate forward the same pointers and duration bits, or log the same
 * diagnostic and forward nothing. */
bool oracle_output_caption_text1_probe(caption_text1_fn candidate, bool output_live, bool text_live);
bool oracle_output_caption_text1_probe(caption_text1_fn candidate, bool output_live, bool text_live)
{
	static struct obs_output output;
	static const char text[] = "caption";
	obs_output_t *out = output_live ? &output : NULL;
	const char *txt = text_live ? text : NULL;
	log_handler_t saved_handler;
	void *saved_param;
	struct oct1_trace original, rust;

	base_get_log_handler(&saved_handler, &saved_param);
	oct1_run(&original, oracle_obs_output_output_caption_text1, out, txt);
	oct1_run(&rust, candidate, out, txt);
	base_set_log_handler(saved_handler, saved_param);

	if (!out)
		return oct1_exact_null_log(&original) && oct1_exact_null_log(&rust) && original.calls == 0 &&
		       rust.calls == 0;
	return original.logs == 0 && rust.logs == 0 && original.calls == 1 && rust.calls == 1 &&
	       original.output == out && rust.output == out && original.text == txt && rust.text == txt &&
	       memcmp(&original.duration, &rust.duration, sizeof(double)) == 0;
}
