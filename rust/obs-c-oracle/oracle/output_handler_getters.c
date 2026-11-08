/* Extracted original C and discovered adapter over the real private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_signal_handler oracle_obs_output_get_signal_handler
#define obs_output_get_proc_handler oracle_obs_output_get_proc_handler
#include "obs-output-handler-getters.c.h"
#undef obs_output_get_proc_handler
#undef obs_output_get_signal_handler
#include "output_handler_getters_fields.inc"

struct output_handlers_fixture {
	struct obs_output output;
	struct obs_output before;
	/* Borrowed addresses only; align each token without a C library alignment typedef. */
	_Alignas(long double) unsigned char tokens[2][sizeof(long double)];
};

void *oracle_output_handlers_new(bool signal, bool proc, bool same, uint32_t flags);
void *oracle_output_handlers_new(bool signal, bool proc, bool same, uint32_t flags)
{
	struct output_handlers_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.context.signals = signal ? (void *)&fixture->tokens[0] : NULL;
		fixture->output.context.procs = proc ? (void *)&fixture->tokens[same ? 0 : 1] : NULL;
		fixture->output.info.flags = flags;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}

void oracle_output_handlers_tokens(void *handle, void **tokens);
void oracle_output_handlers_tokens(void *handle, void **tokens)
{
	struct output_handlers_fixture *fixture = handle;
	tokens[0] = &fixture->tokens[0];
	tokens[1] = &fixture->tokens[1];
}

bool oracle_output_handlers_unchanged(const void *handle);
bool oracle_output_handlers_unchanged(const void *handle)
{
	const struct output_handlers_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}

void oracle_output_handlers_free(void *handle);
void oracle_output_handlers_free(void *handle)
{
	free(handle);
}

bool oracle_output_handlers_addresses(const obs_output_t *output);
bool oracle_output_handlers_addresses(const obs_output_t *output)
{
	struct obs_output_handler_getters_fields fields = obs_output_handler_getters_fields(output);
	return fields.signal == &output->context.signals && fields.proc == &output->context.procs;
}

void oracle_output_handlers_layout(size_t *layout);
void oracle_output_handlers_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_handler_getters_fields);
	layout[1] = _Alignof(struct obs_output_handler_getters_fields);
	layout[2] = offsetof(struct obs_output_handler_getters_fields, signal);
	layout[3] = offsetof(struct obs_output_handler_getters_fields, proc);
	layout[4] = sizeof(signal_handler_t *);
	layout[5] = _Alignof(signal_handler_t *);
	layout[6] = sizeof(proc_handler_t *);
	layout[7] = _Alignof(proc_handler_t *);
}

static void *output_handlers_original_signal(const obs_output_t *output)
{
	return oracle_obs_output_get_signal_handler(output);
}

static void *output_handlers_original_proc(const obs_output_t *output)
{
	return oracle_obs_output_get_proc_handler(output);
}

struct output_handlers_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_handlers_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_handlers_capture(int level, const char *format, va_list args, void *param)
{
	struct output_handlers_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef void *(*output_handlers_getter)(const obs_output_t *);
typedef void (*output_handlers_get_log)(log_handler_t *, void **);
typedef void (*output_handlers_set_log)(log_handler_t, void *);
bool oracle_output_handlers_logs(output_handlers_getter signal, output_handlers_getter proc,
				 output_handlers_get_log get_log, output_handlers_set_log set_log);
bool oracle_output_handlers_logs(output_handlers_getter signal, output_handlers_getter proc,
				 output_handlers_get_log get_log, output_handlers_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	output_handlers_getter rust[] = {signal, proc};
	output_handlers_getter original[] = {output_handlers_original_signal, output_handlers_original_proc};
	const char *expected[] = {"obs_output_get_signal_handler: Null 'output' parameter",
				  "obs_output_get_proc_handler: Null 'output' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		struct output_handlers_log r = {0}, c = {0};
		set_log(output_handlers_capture, &r);
		void *rust_value = rust[i](NULL);
		set_log(output_handlers_capture, &c);
		void *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
