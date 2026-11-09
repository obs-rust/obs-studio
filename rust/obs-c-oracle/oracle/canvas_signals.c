/* Original helper bodies and the real private canvas layout. This fixture is
 * process-isolated by its Rust test because the global getter fixture is shared. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"

void oracle_canvas_dosignal(obs_canvas_t *canvas, const char *global, const char *local);
void oracle_canvas_dosignal_source(const char *signal, obs_canvas_t *canvas, obs_source_t *source);

#define canvas_dosignal oracle_canvas_dosignal
#define canvas_dosignal_source oracle_canvas_dosignal_source
#define obs_canvas_rules_public oracle_obs_canvas_rules_public
bool oracle_obs_canvas_rules_public(bool private);
static struct obs_core canvas_signals_core;
#define obs (&canvas_signals_core)
#include "canvas-signals.inc"
#undef obs
#undef obs_canvas_rules_public
#undef canvas_dosignal_source
#undef canvas_dosignal
#include "canvas_signals_fields.inc"

void oracle_core_global_set(char *locale, signal_handler_t *signals, proc_handler_t *procs);
struct canvas_signal_fixture {
	struct obs_canvas canvas;
	signal_handler_t *alternate;
	uintptr_t source;
	bool mutate;
	size_t *trace;
};
struct canvas_signal_listener {
	struct canvas_signal_fixture *fixture;
	size_t route;
};
static void canvas_signal_record(void *param, calldata_t *data)
{
	struct canvas_signal_listener *listener = param;
	struct canvas_signal_fixture *f = listener->fixture;
	size_t n = f->trace[0]++;
	if (n >= 2)
		return;
	size_t *row = &f->trace[1 + n * 8];
	row[0] = listener->route;
	row[1] = calldata_ptr(data, "canvas") == &f->canvas;
	row[2] = (uintptr_t)calldata_ptr(data, "source");
	row[3] = data->size;
	row[4] = data->capacity;
	row[5] = data->fixed;
	row[6] = calldata_get_data(data, "source", &row[7], sizeof(void *));
	if (listener->route == 1 && f->mutate) {
		calldata_set_ptr(data, "source", (void *)f->source);
		f->canvas.context.signals = f->alternate;
	}
}
void oracle_canvas_signals_run(bool rust, bool private, bool global, bool local, bool source_mode, bool mutate,
			       uintptr_t source, size_t *trace);
void oracle_canvas_signals_run(bool rust, bool private, bool global, bool local, bool source_mode, bool mutate,
			       uintptr_t source, size_t *trace)
{
	memset(trace, 0, 17 * sizeof(*trace));
	struct canvas_signal_fixture f = {0};
	f.canvas.context.private = private;
	f.source = source;
	f.mutate = mutate;
	f.trace = trace;
	signal_handler_t *gh = signal_handler_create();
	signal_handler_t *ch = signal_handler_create();
	f.alternate = signal_handler_create();
	f.canvas.context.signals = ch;
	canvas_signals_core.signals = gh;
	oracle_core_global_set(NULL, gh, NULL);
	struct canvas_signal_listener listeners[] = {{&f, 1}, {&f, 2}, {&f, 3}};
	signal_handler_t *handlers[] = {gh, ch, f.alternate};
	for (size_t i = 0; i < 3; i++) {
		signal_handler_add(handlers[i], "void event(ptr canvas, ptr source)");
		signal_handler_connect(handlers[i], "event", canvas_signal_record, &listeners[i]);
	}
	if (source_mode) {
		if (rust)
			canvas_dosignal_source("event", &f.canvas, (void *)source);
		else
			oracle_canvas_dosignal_source("event", &f.canvas, (void *)source);
	} else if (rust) {
		canvas_dosignal(&f.canvas, global ? "event" : NULL, local ? "event" : NULL);
	} else {
		oracle_canvas_dosignal(&f.canvas, global ? "event" : NULL, local ? "event" : NULL);
	}
	oracle_core_global_set(NULL, NULL, NULL);
	canvas_signals_core.signals = NULL;
	signal_handler_destroy(gh);
	signal_handler_destroy(ch);
	signal_handler_destroy(f.alternate);
}
