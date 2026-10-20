/* obs_output_delay_fields() and obs_output_delay_signal() for tests that
 * compile obs-output-delay.c without obs-output.c. They point at the test's
 * struct obs_output and signal its handler, as libobs does for a real
 * output. */

#include <obs-internal.h>

struct obs_output_delay_fields obs_output_delay_fields(const obs_output_t *output)
{
	struct obs_output *o = (struct obs_output *)(uintptr_t)output;
	struct obs_output_delay_fields f = {
		.delay_mutex = &o->delay_mutex,
		.delay_data = &o->delay_data,
		.active_delay_ns = &o->active_delay_ns,
		.delay_sec = &o->delay_sec,
		.delay_flags = &o->delay_flags,
		.delay_cur_flags = &o->delay_cur_flags,
		.delay_restart_refs = &o->delay_restart_refs,
		.delay_active = &o->delay_active,
		.delay_capturing = &o->delay_capturing,
		.delay_callback = &o->delay_callback,
		.starting = &o->starting,
		.reconnecting = &o->reconnecting,
		.flags = &o->info.flags,
		.name = &o->context.name,
	};

	return f;
}

void obs_output_delay_signal(obs_output_t *output, const char *signal)
{
	do_output_signal(output, signal);
}
