/* Original setter/bootstrap and real field adapters over a test-owned core.
 * Only the unported public UI queue dependency is recorded here; unchanged
 * Tier 2 exercises the actual queue and diagnostic path in both libraries. */
#include "libobs/build/libobs.pch.h"

static THREAD_LOCAL struct obs_core ui_core;
static THREAD_LOCAL bool ui_rust;
static THREAD_LOCAL bool ui_reenter;
static THREAD_LOCAL size_t ui_count;
static THREAD_LOCAL size_t ui_tasks;
static THREAD_LOCAL size_t ui_logs;
static THREAD_LOCAL int ui_log_level;
static THREAD_LOCAL char ui_log_message[128];
struct ui_dispatch {
	unsigned handler;
	obs_task_t task;
	void *param;
	bool wait;
};
static THREAD_LOCAL struct ui_dispatch ui_calls[256];

void oracle_ui_task_handler_queue(obs_task_t task, void *param, bool wait);
void oracle_ui_task_handler_reset(bool rust, bool marked);
void oracle_ui_task_handler_step(unsigned op, size_t data, bool wait);
size_t oracle_ui_task_handler_snapshot(size_t *out);
bool oracle_obs_core_is_ui_thread(void);
void oracle_obs_set_ui_task_handler(obs_task_handler_t handler);
bool obs_core_is_ui_thread(void);

#define obs (&ui_core)
#define obs_set_ui_task_handler oracle_obs_set_ui_task_handler
#define obs_queue_task(type, task, param, wait) oracle_ui_task_handler_queue(task, param, wait)
#define obs_core_is_ui_thread oracle_obs_core_is_ui_thread
#include "obs-ui-task-handler.inc"
#undef obs_core_is_ui_thread
#undef obs_queue_task
#undef obs_set_ui_task_handler
#include "ui_task_handler_fields.inc"
#undef obs

/* Mirror the shared printf/va_list callback attribute used by the existing
 * characterization: the format is supplied by the logging API. */
#if defined(__GNUC__) || defined(__clang__)
static void ui_capture_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void ui_capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	ui_logs++;
	ui_log_level = level;
	vsnprintf(ui_log_message, sizeof(ui_log_message), format, args);
}
#if defined(__GNUC__) || defined(__clang__)
static void ui_log(int level, const char *format, ...) __attribute__((format(printf, 2, 3)));
#endif
static void ui_log(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	ui_capture_log(level, format, args, NULL);
	va_end(args);
}
static void ui_count_task(void *param)
{
	(void)param;
	ui_tasks++;
}
static void ui_record(unsigned handler, obs_task_t task, void *param, bool wait)
{
	assert(ui_count < sizeof(ui_calls) / sizeof(ui_calls[0]));
	ui_calls[ui_count++] = (struct ui_dispatch){handler, task, param, wait};
	if (ui_reenter) {
		ui_reenter = false;
		oracle_ui_task_handler_queue(ui_count_task, (void *)(uintptr_t)7, true);
	}
}
static void ui_handler_a(obs_task_t task, void *param, bool wait)
{
	ui_record(1, task, param, wait);
}
static void ui_handler_b(obs_task_t task, void *param, bool wait)
{
	ui_record(2, task, param, wait);
}
void oracle_ui_task_handler_queue(obs_task_t task, void *param, bool wait)
{
	if (ui_core.ui_task_handler) {
		ui_core.ui_task_handler(task, param, wait);
	} else {
		/* Exact UI_NONE diagnostic from obs_queue_task, independent of the
		 * process-global logger used by unrelated category tests. */
		ui_log(LOG_ERROR, "UI task could not be queued, there's no UI task handler!");
	}
}
void oracle_ui_task_handler_reset(bool rust, bool marked)
{
	ui_core.ui_task_handler = NULL;
	ui_rust = rust;
	ui_reenter = false;
	ui_count = ui_tasks = ui_logs = 0;
	ui_log_level = 0;
	ui_log_message[0] = 0;
	is_ui_thread = marked;
}
void oracle_ui_task_handler_step(unsigned op, size_t data, bool wait)
{
	if (op <= 2 || op == 5) {
		obs_task_handler_t handler = op == 0 ? NULL : op == 2 ? ui_handler_b : ui_handler_a;
		ui_reenter = op == 5;
		if (ui_rust)
			obs_set_ui_task_handler(handler);
		else
			oracle_obs_set_ui_task_handler(handler);
	} else if (op == 3) {
		oracle_ui_task_handler_queue(ui_count_task, (void *)(uintptr_t)data, wait);
	} else if (op == 4 && ui_count) {
		struct ui_dispatch call = ui_calls[data % ui_count];
		call.task(call.param);
	}
}
size_t oracle_ui_task_handler_snapshot(size_t *out)
{
	size_t n = 0;
	out[n++] = ui_count;
	out[n++] = ui_tasks;
	out[n++] = ui_logs;
	out[n++] = (size_t)ui_log_level;
	out[n++] = ui_rust ? obs_core_is_ui_thread() : oracle_obs_core_is_ui_thread();
	out[n++] = ui_core.ui_task_handler == ui_handler_a ? 1 : ui_core.ui_task_handler == ui_handler_b ? 2 : 0;
	out[n++] = ui_logs && strcmp(ui_log_message, "UI task could not be queued, there's no UI task handler!") == 0;
	obs_task_t bootstrap = NULL;
	for (size_t i = 0; i < ui_count; i++) {
		struct ui_dispatch call = ui_calls[i];
		if (call.task != ui_count_task && !bootstrap)
			bootstrap = call.task;
		out[n++] = call.handler;
		out[n++] = call.task == ui_count_task ? 2 : call.task == bootstrap ? 1 : 0;
		out[n++] = (size_t)(uintptr_t)call.param;
		out[n++] = call.wait;
	}
	return n;
}

void oracle_ui_task_handler_layout(size_t *out);
void oracle_ui_task_handler_layout(size_t *out)
{
	out[0] = sizeof(obs_task_handler_t);
	out[1] = _Alignof(obs_task_handler_t);
	out[2] = sizeof(obs_task_t);
	out[3] = _Alignof(obs_task_t);
	out[4] = sizeof(bool);
	out[5] = _Alignof(bool);
}
