/* Tier 3: the unchanged extracted
 * obs_source_audio_output_capture_device_changed() and its activate handler
 * (libobs/obs-source-output-capture-device.c.h), renamed oracle_*, and the
 * production adapters the Rust shims call
 * (obs-source-output-capture-device-fields.c.h, under their real names, the
 * one definition of each in the test binaries), over the real private
 * layout of noise-filled sources and a stand-in core.
 *
 * Both compile against the same recording stand-ins for every call they
 * make (devices_match(), the host default device, bfree(), the signal
 * handler, the calldata, the settings): each records its arguments as
 * indexes (a node, a string of the case's table, NULL) and answers from the
 * case. The core's monitoring fields and the sources' flags, names and
 * signal handlers are real fields. The log lines go through the real blog(),
 * captured by a handler installed for the call. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

#define SOCD_NODES 3
#define SOCD_STRINGS 4
#define SOCD_EVENTS 12
/* An index for NULL, and for a pointer that is none of the case's. */
#define SOCD_NULL 0xfe
#define SOCD_OTHER 0xff

/* One call: changed (which 0) of node `source` with string `device_id`, or
 * the activate handler (1) with a calldata whose "source" is node `source`
 * (-1: NULL). Strings and nodes are indexes, -1 for NULL. The host answers
 * when `host` is set, with `host_def`; every devices_match() answers
 * `match`; the settings' "device_id" is `settings_id`. */
struct socd_case {
	uint8_t which;
	int8_t source;
	uint8_t noise;
	bool monitoring_set;
	int8_t device_id;
	int8_t monitoring_id;
	int8_t duplicating;
	bool host;
	int8_t host_def;
	bool match;
	int8_t settings_id;
	uint32_t flags[SOCD_NODES];
};

/* What one side did: each call in order as {kind, a, b, c}, its log lines
 * (count, level of the last, the last), and whether every byte of the nodes
 * is as before. */
struct socd_result {
	uint32_t events;
	int64_t trace[SOCD_EVENTS][4];
	uint32_t log_count;
	int32_t log_level;
	char log[256];
	bool untouched;
};

typedef void (*socd_handler)(void *, calldata_t *);

struct socd_fixture {
	struct obs_source nodes[SOCD_NODES];
	char names[SOCD_NODES][8];
	char handlers[SOCD_NODES + 1];
	char settings[SOCD_NODES];
	calldata_t cd;
	struct obs_core *core;
	const struct socd_case *c;
	struct socd_result *out;
	socd_handler self;
	void *cd_value;
};

/* Per thread: the Rust test harness runs cases on several threads. */
static _Thread_local struct socd_fixture *socd_current;

/* Writable, so the host stand-in can hand one out as its char *. */
static char socd_strings[SOCD_STRINGS][16] = {"sink_a", "sink_b", "default", "sink_a.monitor"};

static int64_t socd_node_index(const void *source)
{
	struct socd_fixture *f = socd_current;
	if (!source)
		return SOCD_NULL;
	for (size_t i = 0; i < SOCD_NODES; i++) {
		if (source == &f->nodes[i])
			return (int64_t)i;
	}
	return SOCD_OTHER;
}

static obs_source_t *socd_node(int8_t index)
{
	return index < 0 || index >= SOCD_NODES ? NULL : &socd_current->nodes[index];
}

static int64_t socd_string_index(const char *s)
{
	if (!s)
		return SOCD_NULL;
	for (size_t i = 0; i < SOCD_STRINGS; i++) {
		if (s == socd_strings[i])
			return (int64_t)i;
	}
	return SOCD_OTHER;
}

static char *socd_string(int8_t index)
{
	return index < 0 || index >= SOCD_STRINGS ? NULL : socd_strings[index];
}

/* 0: the core's handler; 1 + i: node i's; else OTHER. */
static int64_t socd_handler_index(const signal_handler_t *handler)
{
	struct socd_fixture *f = socd_current;
	for (size_t i = 0; i < SOCD_NODES + 1; i++) {
		if ((const void *)handler == &f->handlers[i])
			return (int64_t)i;
	}
	return SOCD_OTHER;
}

static int64_t socd_settings_index(const void *settings)
{
	struct socd_fixture *f = socd_current;
	for (size_t i = 0; i < SOCD_NODES; i++) {
		if (settings == &f->settings[i])
			return (int64_t)i;
	}
	return SOCD_OTHER;
}

static void socd_event(int64_t kind, int64_t a, int64_t b, int64_t c)
{
	struct socd_result *out = socd_current->out;
	if (out->events < SOCD_EVENTS) {
		int64_t *e = out->trace[out->events];
		e[0] = kind;
		e[1] = a;
		e[2] = b;
		e[3] = c;
	}
	out->events++;
}

/* ------------------------------------------------------------------------- */
/* the stand-ins */

bool socd_devices_match(const char *id1, const char *id2);
bool socd_devices_match(const char *id1, const char *id2)
{
	socd_event('M', socd_string_index(id1), socd_string_index(id2), 0);
	return socd_current->c->match;
}

static bool socd_host_default(const char *device_id, char **def_id)
{
	const struct socd_case *c = socd_current->c;
	socd_event('H', socd_string_index(device_id), def_id != NULL && *def_id == NULL, 0);
	if (!c->host)
		return false;
	*def_id = socd_string(c->host_def);
	return true;
}

static void socd_bfree(void *ptr)
{
	socd_event('F', socd_string_index(ptr), 0, 0);
}

static void socd_cd_init(calldata_t *data, uint8_t *stack, size_t size)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(stack);
	UNUSED_PARAMETER(size);
	socd_current->cd_value = (void *)(uintptr_t)1;
}

static void socd_cd_set_ptr(calldata_t *data, const char *name, void *ptr)
{
	UNUSED_PARAMETER(data);
	socd_current->cd_value = strcmp(name, "source") == 0 ? ptr : (void *)(uintptr_t)1;
}

static void *socd_cd_ptr(const calldata_t *data, const char *name)
{
	socd_event('P', data == &socd_current->cd, strcmp(name, "source") == 0, 0);
	return socd_node(socd_current->c->source);
}

static void socd_signal(signal_handler_t *handler, const char *signal, calldata_t *params)
{
	UNUSED_PARAMETER(params);
	socd_event('S', socd_handler_index(handler), strcmp(signal, "deduplication_changed") == 0,
		   socd_node_index(socd_current->cd_value));
}

static void socd_connect(signal_handler_t *handler, const char *signal, signal_callback_t callback, void *data)
{
	socd_event('C', socd_handler_index(handler), strcmp(signal, "activate") == 0,
		   (callback == socd_current->self) + 2 * (data != NULL));
}

static void socd_disconnect(signal_handler_t *handler, const char *signal, signal_callback_t callback, void *data)
{
	socd_event('D', socd_handler_index(handler), strcmp(signal, "activate") == 0,
		   (callback == socd_current->self) + 2 * (data != NULL));
}

static obs_data_t *socd_get_settings(const obs_source_t *source)
{
	int64_t i = socd_node_index(source);
	socd_event('G', i, 0, 0);
	return i < SOCD_NODES ? (obs_data_t *)&socd_current->settings[i] : NULL;
}

static const char *socd_get_string(obs_data_t *settings, const char *name)
{
	socd_event('T', socd_settings_index(settings), strcmp(name, "device_id") == 0, 0);
	return socd_string(socd_current->c->settings_id);
}

static void socd_data_release(obs_data_t *settings)
{
	socd_event('R', socd_settings_index(settings), 0, 0);
}

/* ------------------------------------------------------------------------- */
/* the original and the production adapters, over the stand-ins */

#define obs (socd_current->core)
#define devices_match socd_devices_match
#define obs_host_default_monitoring_device socd_host_default
#define bfree socd_bfree
#define calldata_init_fixed socd_cd_init
#define calldata_set_ptr socd_cd_set_ptr
#define calldata_ptr socd_cd_ptr
#define signal_handler_signal socd_signal
#define signal_handler_connect socd_connect
#define signal_handler_disconnect socd_disconnect
#define obs_source_get_settings socd_get_settings
#define obs_data_get_string socd_get_string
#define obs_data_release socd_data_release
#define obs_source_audio_output_capture_device_changed oracle_obs_source_audio_output_capture_device_changed
#define obs_source_audio_output_capture_device_activated oracle_obs_source_audio_output_capture_device_activated
void oracle_obs_source_audio_output_capture_device_changed(obs_source_t *source, const char *device_id);
#include "source-output-capture-device.inc"
#undef obs_source_audio_output_capture_device_activated
#undef obs_source_audio_output_capture_device_changed
#include "source_output_capture_device_fields.inc"
#undef obs_data_release
#undef obs_data_get_string
#undef obs_source_get_settings
#undef signal_handler_disconnect
#undef signal_handler_connect
#undef signal_handler_signal
#undef calldata_ptr
#undef calldata_set_ptr
#undef calldata_init_fixed
#undef bfree
#undef obs_host_default_monitoring_device
#undef devices_match
#undef obs

/* ------------------------------------------------------------------------- */
/* the parity run */

#if defined(__GNUC__) || defined(__clang__)
static void socd_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void socd_capture(int level, const char *format, va_list args, void *param)
{
	struct socd_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef void (*socd_changed)(obs_source_t *, const char *);
typedef void (*socd_get_log)(log_handler_t *, void **);
typedef void (*socd_set_log)(log_handler_t, void *);

/* The original calls, for the parity test's other side. */
void oracle_source_output_capture_device_originals(socd_changed *changed, socd_handler *activated);
void oracle_source_output_capture_device_originals(socd_changed *changed, socd_handler *activated)
{
	*changed = oracle_obs_source_audio_output_capture_device_changed;
	*activated = oracle_obs_source_audio_output_capture_device_activated;
}

/* Runs one case through `changed` or `activated` on fresh noise-filled
 * nodes and a stand-in core. False when the fixture could not be built or
 * an adapter does not reach its field. */
bool oracle_source_output_capture_device_run(socd_changed changed, socd_handler activated, socd_get_log get,
					     socd_set_log set, const struct socd_case *c, struct socd_result *out);
bool oracle_source_output_capture_device_run(socd_changed changed, socd_handler activated, socd_get_log get,
					     socd_set_log set, const struct socd_case *c, struct socd_result *out)
{
	struct socd_fixture *f = malloc(sizeof(*f));
	struct obs_source *before = malloc(sizeof(f->nodes));
	struct obs_core *core = calloc(1, sizeof(*core));
	if (!f || !before || !core) {
		free(f);
		free(before);
		free(core);
		return false;
	}
	memset(f, c->noise, sizeof(*f));
	memset(out, 0, sizeof(*out));
	out->log_level = -1;
	f->c = c;
	f->out = out;
	f->self = activated;
	f->cd_value = NULL;
	f->core = core;
	core->audio.monitoring_device_name = c->monitoring_set ? socd_strings[0] : NULL;
	core->audio.monitoring_device_id = socd_string(c->monitoring_id);
	core->signals = (signal_handler_t *)&f->handlers[0];
	for (size_t i = 0; i < SOCD_NODES; i++) {
		memcpy(f->names[i], "node 0", sizeof("node 0"));
		f->names[i][5] = (char)('0' + i);
		f->nodes[i].info.output_flags = c->flags[i];
		f->nodes[i].context.name = f->names[i];
		f->nodes[i].context.signals = (signal_handler_t *)&f->handlers[1 + i];
	}
	socd_current = f;
	core->audio.monitoring_duplicating_source = socd_node(c->duplicating);

	/* The real adapters reach the fields they name. */
	bool valid = obs_source_output_capture_device_monitoring_set() == c->monitoring_set &&
		     obs_source_output_capture_device_monitoring_id() == socd_string(c->monitoring_id) &&
		     obs_source_output_capture_device_duplicating() == socd_node(c->duplicating);
	for (size_t i = 0; i < SOCD_NODES; i++) {
		valid = valid && obs_source_output_capture_device_flags(&f->nodes[i]) == c->flags[i] &&
			obs_source_output_capture_device_name(&f->nodes[i]) == f->names[i];
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	set(socd_capture, out);
	if (c->which == 0)
		changed(socd_node(c->source), socd_string(c->device_id));
	else
		activated(NULL, &f->cd);
	set(old_handler, old_param);
	socd_current = NULL;

	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(core);
	free(before);
	free(f);
	return valid;
}
