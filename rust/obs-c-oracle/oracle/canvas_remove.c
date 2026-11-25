/* Original extracted removal over real canvas/control layouts and real
 * retain/release/signal APIs. Global getter fixtures require process isolation. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <string.h>

void oracle_obs_canvas_remove(obs_canvas_t *canvas);
#define obs_canvas_remove oracle_obs_canvas_remove
#include "canvas-remove.inc"
#undef obs_canvas_remove
#include "canvas_remove_fields.inc"

void oracle_core_global_set(char *locale, signal_handler_t *signals, proc_handler_t *procs);
void oracle_core_initialized_set(void *core);

struct canvas_remove_fixture {
	struct obs_canvas canvas;
	struct obs_weak_canvas control;
	size_t *trace;
	bool mutate;
	uint32_t callback_flags;
};
struct canvas_remove_listener {
	struct canvas_remove_fixture *fixture;
	size_t route;
};
static void canvas_remove_record(void *param, calldata_t *data)
{
	struct canvas_remove_listener *listener = param;
	struct canvas_remove_fixture *f = listener->fixture;
	size_t index = f->trace[0]++;
	if (index < 6) {
		size_t *row = &f->trace[1 + index * 4];
		row[0] = listener->route;
		row[1] = calldata_ptr(data, "canvas") == &f->canvas;
		row[2] = f->canvas.flags;
		row[3] = (size_t)os_atomic_load_long(&f->control.ref.refs);
	}
	if (listener->route == 1 && f->mutate)
		f->canvas.flags = f->callback_flags;
}

void oracle_canvas_remove_run(void (*candidate)(obs_canvas_t *), uint32_t flags, bool private, bool expired,
			      bool mutate, uint32_t callback_flags, unsigned repeats, size_t *trace);
void oracle_canvas_remove_run(void (*candidate)(obs_canvas_t *), uint32_t flags, bool private, bool expired,
			      bool mutate, uint32_t callback_flags, unsigned repeats, size_t *trace)
{
	memset(trace, 0, 29 * sizeof(*trace));
	struct canvas_remove_fixture f = {0};
	f.canvas.flags = flags;
	f.canvas.context.private = private;
	f.canvas.context.control = (struct obs_weak_object *)&f.control;
	f.control.canvas = &f.canvas;
	f.control.ref.refs = expired ? -1 : 2;
	f.control.ref.weak_refs = 3;
	f.trace = trace;
	f.mutate = mutate;
	f.callback_flags = callback_flags;
	signal_handler_t *global = signal_handler_create();
	signal_handler_t *local = signal_handler_create();
	f.canvas.context.signals = local;
	signal_handler_add(global, "void canvas_remove(ptr canvas)");
	signal_handler_add(local, "void remove(ptr canvas)");
	struct canvas_remove_listener listeners[] = {{&f, 1}, {&f, 2}};
	signal_handler_connect(global, "canvas_remove", canvas_remove_record, &listeners[0]);
	signal_handler_connect(local, "remove", canvas_remove_record, &listeners[1]);
	oracle_core_global_set(NULL, global, NULL);
	oracle_core_initialized_set((void *)(uintptr_t)1);
	struct obs_canvas before;
	memcpy(&before, &f.canvas, sizeof(before));
	for (unsigned i = 0; i < repeats && i < 3; i++)
		candidate(&f.canvas);
	trace[25] = f.canvas.flags;
	trace[26] = (size_t)os_atomic_load_long(&f.control.ref.refs);
	trace[27] = (size_t)os_atomic_load_long(&f.control.ref.weak_refs);
	before.flags = f.canvas.flags;
	trace[28] = memcmp(&before, &f.canvas, sizeof(before)) == 0 && f.control.canvas == &f.canvas;
	oracle_core_initialized_set(NULL);
	oracle_core_global_set(NULL, NULL, NULL);
	signal_handler_destroy(global);
	signal_handler_destroy(local);
}
