/* Original obs_set_audio_monitoring_device() and
 * obs_reset_audio_monitoring() (#5491), and the real-header adapters of
 * obs-audio-monitoring-device-fields.c.h, on one recording fixture core.
 * Under cargo test these adapters are the only definitions of their real
 * names, so the Rust shim reaches this fixture. Every core read, the
 * availability check, the mutex, the compare, the frees and copies, each
 * monitor reset, the enumeration and each source's settings read, notice
 * and release are traced; the probe runs the original and a candidate on
 * the same fixture and reports any difference in the trace, the result or
 * the stored strings, so mutants are observable. The monitoring mutex is
 * recorded, never taken, so a mutant that locks twice shows in the trace
 * instead of deadlocking. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define AMD_MONITORS 8
#define AMD_SOURCES 8
#define AMD_COPIES 4
#define AMD_TEXT 48
#define AMD_TRACE 512

static _Thread_local struct obs_core amd_core;
static _Thread_local struct obs_core *amd_current;
static _Thread_local struct obs_source amd_sources[AMD_SOURCES];
static _Thread_local size_t amd_source_count;
static _Thread_local const char *amd_device_ids[AMD_SOURCES];
static _Thread_local char amd_settings[AMD_SOURCES];
static _Thread_local char amd_monitor_tokens[AMD_MONITORS];
static _Thread_local struct audio_monitor *amd_monitors[AMD_MONITORS];
static _Thread_local bool amd_is_available;
static _Thread_local bool amd_locked;
static _Thread_local char amd_old_name[AMD_TEXT];
static _Thread_local char amd_old_id[AMD_TEXT];
static _Thread_local char amd_copies[AMD_COPIES][AMD_TEXT];
static _Thread_local size_t amd_copy_count;
static _Thread_local char amd_trace[AMD_TRACE];
static _Thread_local size_t amd_trace_len;

static void amd_event(char value)
{
	if (amd_trace_len + 1 < sizeof(amd_trace)) {
		amd_trace[amd_trace_len++] = value;
		amd_trace[amd_trace_len] = '\0';
	}
}

static char amd_digit(size_t index)
{
	return index < 10 ? (char)('0' + index) : '?';
}

static void amd_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	if (!text)
		text = "<null>";
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* 'o' reads the core. */
static struct obs_core *amd_read_core(void)
{
	amd_event('o');
	return amd_current;
}

/* 'A' asks whether monitoring is available. */
static bool amd_available(void)
{
	amd_event('A');
	return amd_is_available;
}

/* 'L' locks; 'R' marks a lock already held or another mutex. */
static int amd_lock(pthread_mutex_t *mutex)
{
	if (mutex != &amd_core.audio.monitoring_mutex || amd_locked)
		amd_event('R');
	amd_event('L');
	amd_locked = true;
	return 0;
}

/* 'U' unlocks; 'R' marks a lock not held or another mutex. */
static int amd_unlock(pthread_mutex_t *mutex)
{
	if (mutex != &amd_core.audio.monitoring_mutex || !amd_locked)
		amd_event('R');
	amd_event('U');
	amd_locked = false;
	return 0;
}

/* 'X' marks a step on the monitoring fields outside the lock. */
static void amd_need_lock(void)
{
	if (!amd_locked)
		amd_event('X');
}

/* 'C' compares the id. */
static int amd_strcmp(const char *a, const char *b)
{
	amd_need_lock();
	amd_event('C');
	return strcmp(a, b);
}

/* 'F' frees, then 'n' the stored name, 'i' the stored id, 'c' a copy. */
static void amd_bfree(void *ptr)
{
	amd_need_lock();
	amd_event('F');
	if (ptr == amd_old_name)
		amd_event('n');
	else if (ptr == amd_old_id)
		amd_event('i');
	else if (ptr)
		amd_event('c');
	else
		amd_event('0');
}

/* 'S' copies a string into the next copy slot. */
static char *amd_bstrdup(const char *text)
{
	amd_need_lock();
	amd_event('S');
	if (!text || amd_copy_count >= AMD_COPIES) {
		amd_event('!');
		return NULL;
	}
	char *copy = amd_copies[amd_copy_count++];
	amd_copy(copy, AMD_TEXT, text);
	return copy;
}

/* 'M' resets the monitor named next. */
static void amd_monitor_reset(struct audio_monitor *monitor)
{
	amd_need_lock();
	amd_event('M');
	for (size_t i = 0; i < AMD_MONITORS; i++) {
		if (monitor == (struct audio_monitor *)&amd_monitor_tokens[i]) {
			amd_event(amd_digit(i));
			return;
		}
	}
	amd_event('?');
}

static size_t amd_source_index(const obs_source_t *source)
{
	for (size_t i = 0; i < AMD_SOURCES; i++) {
		if (source == &amd_sources[i])
			return i;
	}
	return AMD_SOURCES;
}

static size_t amd_settings_index(const obs_data_t *settings)
{
	for (size_t i = 0; i < AMD_SOURCES; i++) {
		if ((const void *)settings == (const void *)&amd_settings[i])
			return i;
	}
	return AMD_SOURCES;
}

/* 'E' enumerates the sources in order; '!' marks a parameter, 'R' the lock
 * held, 'b' a callback that stopped it. */
static void amd_enum_sources(bool (*visit)(void *, obs_source_t *), void *param)
{
	amd_event('E');
	if (param)
		amd_event('!');
	if (amd_locked)
		amd_event('R');
	for (size_t i = 0; i < amd_source_count; i++) {
		if (!visit(param, &amd_sources[i])) {
			amd_event('b');
			break;
		}
	}
}

/* 'g' reads the settings of the source named next. */
static obs_data_t *amd_get_settings(const obs_source_t *source)
{
	size_t index = amd_source_index(source);
	amd_event('g');
	amd_event(amd_digit(index));
	return index < AMD_SOURCES ? (obs_data_t *)(void *)&amd_settings[index] : NULL;
}

/* 's' reads a string of those settings; '?' marks another key. */
static const char *amd_get_string(obs_data_t *settings, const char *name)
{
	size_t index = amd_settings_index(settings);
	amd_event('s');
	amd_event(amd_digit(index));
	if (strcmp(name, "device_id") != 0)
		amd_event('?');
	return index < AMD_SOURCES ? amd_device_ids[index] : NULL;
}

/* 'N' tells the source named next; '?' marks another device id. */
static void amd_device_changed(obs_source_t *source, const char *device_id)
{
	size_t index = amd_source_index(source);
	amd_event('N');
	amd_event(amd_digit(index));
	if (index >= AMD_SOURCES || device_id != amd_device_ids[index])
		amd_event('?');
}

/* 'r' releases the settings named next. */
static void amd_data_release(obs_data_t *settings)
{
	amd_event('r');
	amd_event(amd_digit(amd_settings_index(settings)));
}

void oracle_obs_reset_audio_monitoring(void);
bool oracle_obs_set_audio_monitoring_device(const char *name, const char *id);

/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name for the rest of the file and use the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#pragma push_macro("strcmp")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#undef strcmp
#define pthread_mutex_lock amd_lock
#define pthread_mutex_unlock amd_unlock
#define strcmp amd_strcmp
#define obs amd_read_core()
#define obs_audio_monitoring_available amd_available
#define bfree amd_bfree
#define bstrdup amd_bstrdup
#define audio_monitor_reset amd_monitor_reset
#define obs_enum_sources amd_enum_sources
#define obs_source_get_settings amd_get_settings
#define obs_data_get_string amd_get_string
#define obs_source_audio_output_capture_device_changed amd_device_changed
#define obs_data_release amd_data_release
#define obs_reset_audio_monitoring oracle_obs_reset_audio_monitoring
#define obs_set_audio_monitoring_device oracle_obs_set_audio_monitoring_device
#include "libobs/obs-audio-monitoring-device.c.h"
#undef obs_set_audio_monitoring_device
#undef obs_reset_audio_monitoring
#include "audio_monitoring_device_fields.inc"
#undef obs_data_release
#undef obs_source_audio_output_capture_device_changed
#undef obs_data_get_string
#undef obs_source_get_settings
#undef obs_enum_sources
#undef audio_monitor_reset
#undef bstrdup
#undef bfree
#undef obs_audio_monitoring_available
#undef obs
#undef strcmp
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("strcmp")
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

typedef bool (*amd_set_fn)(const char *name, const char *id);
typedef void (*amd_reset_fn)(void);

struct amd_result {
	char trace[AMD_TRACE];
	char name[AMD_TEXT];
	char id[AMD_TEXT];
	bool result;
	bool locked;
};

/* A fixture core storing "Old name" and `stored_id`, `monitors` live
 * monitors, and `sources` enumerated sources with `flags` and
 * `device_ids`. */
static bool amd_arm(bool available, const char *stored_id, size_t monitors, const uint32_t *flags,
		    const char *const *device_ids, size_t sources)
{
	if (monitors > AMD_MONITORS || sources > AMD_SOURCES)
		return false;
	memset(&amd_core, 0, sizeof(amd_core));
	memset(amd_sources, 0, sizeof(amd_sources));
	memset(amd_copies, 0, sizeof(amd_copies));
	amd_copy(amd_old_name, AMD_TEXT, "Old name");
	amd_copy(amd_old_id, AMD_TEXT, stored_id);
	amd_core.audio.monitoring_device_name = amd_old_name;
	amd_core.audio.monitoring_device_id = amd_old_id;
	for (size_t i = 0; i < monitors; i++)
		amd_monitors[i] = (struct audio_monitor *)(void *)&amd_monitor_tokens[i];
	amd_core.audio.monitors.array = amd_monitors;
	amd_core.audio.monitors.num = monitors;
	amd_core.audio.monitors.capacity = AMD_MONITORS;
	for (size_t i = 0; i < sources; i++) {
		amd_sources[i].info.output_flags = flags[i];
		amd_device_ids[i] = device_ids[i];
	}
	amd_source_count = sources;
	amd_is_available = available;
	amd_current = &amd_core;
	amd_locked = false;
	amd_copy_count = 0;
	amd_trace_len = 0;
	amd_trace[0] = '\0';
	return true;
}

static void amd_collect(bool result, struct amd_result *out)
{
	memcpy(out->trace, amd_trace, sizeof(out->trace));
	amd_copy(out->name, AMD_TEXT, amd_core.audio.monitoring_device_name);
	amd_copy(out->id, AMD_TEXT, amd_core.audio.monitoring_device_id);
	out->result = result;
	out->locked = amd_locked;
	amd_current = NULL;
}

/* 0: match. 1: the traces differ, 2: the results differ, 4: the stored
 * strings differ, 8: the mutex was left held. 0x100: fixture too large. */
static unsigned amd_diff(const struct amd_result *original, const struct amd_result *actual)
{
	unsigned diff = 0;
	if (strcmp(original->trace, actual->trace) != 0)
		diff |= 1;
	if (original->result != actual->result)
		diff |= 2;
	if (strcmp(original->name, actual->name) != 0 || strcmp(original->id, actual->id) != 0)
		diff |= 4;
	if (original->locked || actual->locked)
		diff |= 8;
	return diff;
}

static bool amd_run_set(amd_set_fn set, const char *name, const char *id, bool available, const char *stored_id,
			size_t monitors, const uint32_t *flags, const char *const *device_ids, size_t sources,
			struct amd_result *out)
{
	if (!amd_arm(available, stored_id, monitors, flags, device_ids, sources))
		return false;
	bool result = set(name, id);
	amd_collect(result, out);
	return true;
}

unsigned oracle_audio_monitoring_device_set_probe(amd_set_fn candidate, const char *name, const char *id,
						  bool available, const char *stored_id, size_t monitors,
						  const uint32_t *flags, const char *const *device_ids, size_t sources);
unsigned oracle_audio_monitoring_device_set_probe(amd_set_fn candidate, const char *name, const char *id,
						  bool available, const char *stored_id, size_t monitors,
						  const uint32_t *flags, const char *const *device_ids, size_t sources)
{
	static _Thread_local struct amd_result original, actual;
	if (!amd_run_set(oracle_obs_set_audio_monitoring_device, name, id, available, stored_id, monitors, flags,
			 device_ids, sources, &original) ||
	    !amd_run_set(candidate, name, id, available, stored_id, monitors, flags, device_ids, sources, &actual))
		return 0x100;
	return amd_diff(&original, &actual);
}

unsigned oracle_audio_monitoring_device_reset_probe(amd_reset_fn candidate, bool available, size_t monitors);
unsigned oracle_audio_monitoring_device_reset_probe(amd_reset_fn candidate, bool available, size_t monitors)
{
	static _Thread_local struct amd_result original, actual;
	if (!amd_arm(available, "id", monitors, NULL, NULL, 0))
		return 0x100;
	oracle_obs_reset_audio_monitoring();
	amd_collect(false, &original);
	amd_arm(available, "id", monitors, NULL, NULL, 0);
	candidate();
	amd_collect(false, &actual);
	return amd_diff(&original, &actual);
}

/* The original setter's trace, result and stored strings, for the
 * characterization checks. */
bool oracle_audio_monitoring_device_set_original(const char *name, const char *id, bool available,
						 const char *stored_id, size_t monitors, const uint32_t *flags,
						 const char *const *device_ids, size_t sources, char *trace,
						 size_t trace_size, char *stored, size_t stored_size);
bool oracle_audio_monitoring_device_set_original(const char *name, const char *id, bool available,
						 const char *stored_id, size_t monitors, const uint32_t *flags,
						 const char *const *device_ids, size_t sources, char *trace,
						 size_t trace_size, char *stored, size_t stored_size)
{
	static _Thread_local struct amd_result result;
	if (!amd_run_set(oracle_obs_set_audio_monitoring_device, name, id, available, stored_id, monitors, flags,
			 device_ids, sources, &result))
		return false;
	amd_copy(trace, trace_size, result.trace);
	if (stored_size) {
		amd_copy(stored, stored_size, result.name);
		size_t n = strlen(stored);
		if (n + 1 < stored_size) {
			stored[n] = '/';
			amd_copy(stored + n + 1, stored_size - n - 1, result.id);
		}
	}
	return result.result;
}
