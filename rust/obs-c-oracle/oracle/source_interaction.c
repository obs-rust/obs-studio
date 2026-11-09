/* The extracted original, unchanged, and the real private-layout adapter. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define obs_source_send_mouse_click oracle_obs_source_send_mouse_click
#define obs_source_send_mouse_move oracle_obs_source_send_mouse_move
#define obs_source_send_mouse_wheel oracle_obs_source_send_mouse_wheel
#define obs_source_send_focus oracle_obs_source_send_focus
#define obs_source_send_key_click oracle_obs_source_send_key_click
#include "obs-source-interaction.c.h"
#undef obs_source_send_mouse_click
#undef obs_source_send_mouse_move
#undef obs_source_send_mouse_wheel
#undef obs_source_send_focus
#undef obs_source_send_key_click
#include "source_interaction_fields.inc"

struct interaction_dispatchers {
	void (*mouse_click)(obs_source_t *, const struct obs_mouse_event *, int32_t, bool, uint32_t);
	void (*mouse_move)(obs_source_t *, const struct obs_mouse_event *, bool);
	void (*mouse_wheel)(obs_source_t *, const struct obs_mouse_event *, int, int);
	void (*focus)(obs_source_t *, bool);
	void (*key_click)(obs_source_t *, const struct obs_key_event *, bool);
};
static const struct interaction_dispatchers original = {
	oracle_obs_source_send_mouse_click, oracle_obs_source_send_mouse_move, oracle_obs_source_send_mouse_wheel,
	oracle_obs_source_send_focus,       oracle_obs_source_send_key_click,
};
struct interaction_trace {
	unsigned calls[5];
	void *data[5];
	const struct obs_mouse_event *mouse[3];
	const struct obs_key_event *key;
	int32_t type;
	uint32_t count;
	int x, y;
	bool up, leave, focus, key_up;
};
#if defined(_MSC_VER)
static __declspec(thread) struct interaction_trace *active;
#else
static _Thread_local struct interaction_trace *active;
#endif
static void interaction_click(void *data, const struct obs_mouse_event *event, int32_t type, bool up, uint32_t count)
{
	active->calls[0]++;
	active->data[0] = data;
	active->mouse[0] = event;
	active->type = type;
	active->up = up;
	active->count = count;
}
static void interaction_move(void *data, const struct obs_mouse_event *event, bool leave)
{
	active->calls[1]++;
	active->data[1] = data;
	active->mouse[1] = event;
	active->leave = leave;
}
static void interaction_wheel(void *data, const struct obs_mouse_event *event, int x, int y)
{
	active->calls[2]++;
	active->data[2] = data;
	active->mouse[2] = event;
	active->x = x;
	active->y = y;
}
static void interaction_focus(void *data, bool focus)
{
	active->calls[3]++;
	active->data[3] = data;
	active->focus = focus;
}
static void interaction_key(void *data, const struct obs_key_event *event, bool up)
{
	active->calls[4]++;
	active->data[4] = data;
	active->key = event;
	active->key_up = up;
}
static void interaction_run(const struct interaction_dispatchers *functions, obs_source_t *source,
			    const struct obs_mouse_event *mouse, const struct obs_key_event *key, int32_t type,
			    bool toggle, uint32_t count, int x, int y)
{
	functions->mouse_click(source, mouse, type, toggle, count);
	functions->mouse_move(source, mouse, toggle);
	functions->mouse_wheel(source, mouse, x, y);
	functions->focus(source, toggle);
	functions->key_click(source, key, toggle);
}

bool oracle_source_interaction_check(const struct interaction_dispatchers *rust, uint32_t flags, uint8_t mask,
				     bool null_source, bool null_data, bool null_event, int32_t type, bool toggle,
				     uint32_t count, int x, int y);
bool oracle_source_interaction_check(const struct interaction_dispatchers *rust, uint32_t flags, uint8_t mask,
				     bool null_source, bool null_data, bool null_event, int32_t type, bool toggle,
				     uint32_t count, int x, int y)
{
	struct obs_source source = {0};
	struct obs_mouse_event mouse = {.modifiers = UINT32_MAX, .x = INT32_MIN, .y = INT32_MAX};
	char text[] = "key payload";
	struct obs_key_event key = {.modifiers = UINT32_MAX,
				    .text = text,
				    .native_modifiers = 0x87654321,
				    .native_scancode = 0x12345678,
				    .native_vkey = UINT32_MAX};
	source.info.output_flags = flags;
	source.context.data = null_data ? NULL : &source;
	source.info.mouse_click = mask & 1 ? interaction_click : NULL;
	source.info.mouse_move = mask & 2 ? interaction_move : NULL;
	source.info.mouse_wheel = mask & 4 ? interaction_wheel : NULL;
	source.info.focus = mask & 8 ? interaction_focus : NULL;
	source.info.key_click = mask & 16 ? interaction_key : NULL;
	struct obs_source before;
	memcpy(&before, &source, sizeof(source));
	struct interaction_trace traces[2];
	memset(traces, 0, sizeof(traces));
	struct interaction_trace *saved = active;
	const struct interaction_dispatchers *implementations[] = {&original, rust};
	bool equal = true;
	for (size_t n = 0; n < 2; n++) {
		active = &traces[n];
		interaction_run(implementations[n], null_source ? NULL : &source, null_event ? NULL : &mouse,
				null_event ? NULL : &key, type, toggle, count, x, y);
		for (size_t i = 0; i < 5; i++) {
			unsigned expected = !null_source && (flags & OBS_SOURCE_INTERACTION) && (mask & (1u << i));
			equal = equal && active->calls[i] == expected;
			if (expected)
				equal = equal && active->data[i] == source.context.data;
		}
		equal = equal && memcmp(&before, &source, sizeof(source)) == 0;
	}
	active = saved;
	return equal && memcmp(&traces[0], &traces[1], sizeof(traces[0])) == 0;
}

void oracle_source_interaction_layout(size_t *out);
void oracle_source_interaction_layout(size_t *out)
{
#define LAYOUT(type) *out++ = sizeof(type); *out++ = _Alignof(type)
#define OFFSET(type, field) *out++ = offsetof(type, field)
	LAYOUT(struct obs_mouse_event);
	OFFSET(struct obs_mouse_event, modifiers);
	OFFSET(struct obs_mouse_event, x);
	OFFSET(struct obs_mouse_event, y);
	LAYOUT(struct obs_key_event);
	OFFSET(struct obs_key_event, modifiers);
	OFFSET(struct obs_key_event, text);
	OFFSET(struct obs_key_event, native_modifiers);
	OFFSET(struct obs_key_event, native_scancode);
	OFFSET(struct obs_key_event, native_vkey);
	LAYOUT(struct obs_source_interaction_fields);
	OFFSET(struct obs_source_interaction_fields, flags);
	OFFSET(struct obs_source_interaction_fields, data);
	OFFSET(struct obs_source_interaction_fields, mouse_click);
	OFFSET(struct obs_source_interaction_fields, mouse_move);
	OFFSET(struct obs_source_interaction_fields, mouse_wheel);
	OFFSET(struct obs_source_interaction_fields, focus);
	OFFSET(struct obs_source_interaction_fields, key_click);
	LAYOUT(struct interaction_dispatchers);
	OFFSET(struct interaction_dispatchers, mouse_click);
	OFFSET(struct interaction_dispatchers, mouse_move);
	OFFSET(struct interaction_dispatchers, mouse_wheel);
	OFFSET(struct interaction_dispatchers, focus);
	OFFSET(struct interaction_dispatchers, key_click);
	*out++ = OBS_SOURCE_INTERACTION;
#undef LAYOUT
#undef OFFSET
}

struct interaction_log {
	unsigned count;
	int level;
	char message[512];
};
#if defined(__GNUC__) || defined(__clang__)
static void interaction_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void interaction_capture(int level, const char *format, va_list args, void *param)
{
	struct interaction_log *log = param;
	log->count++;
	log->level = level;
	size_t used = strlen(log->message);
	vsnprintf(log->message + used, sizeof(log->message) - used, format, args);
}
typedef void (*interaction_get_log)(log_handler_t *, void **);
typedef void (*interaction_set_log)(log_handler_t, void *);
bool oracle_source_interaction_logs(const struct interaction_dispatchers *rust, interaction_get_log get_log,
				    interaction_set_log set_log);
bool oracle_source_interaction_logs(const struct interaction_dispatchers *rust, interaction_get_log get_log,
				    interaction_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct interaction_log r = {0}, c = {0};
	set_log(interaction_capture, &r);
	interaction_run(rust, NULL, NULL, NULL, 0, false, 0, 0, 0);
	set_log(interaction_capture, &c);
	interaction_run(&original, NULL, NULL, NULL, 0, false, 0, 0, 0);
	set_log(old_handler, old_param);
	return r.count == 5 && c.count == 5 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
	       strcmp(r.message, c.message) == 0;
}
