/* Original setters and signal builder, over the real private source layout.
 * No intentional differences or excluded inputs. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-source-monitoring.h"
#include "obs-source-push-to-enable.h"
uint32_t oracle_push_enable_get_flags(const obs_source_t *source);
const char *oracle_push_enable_get_name(const obs_source_t *source);
#define obs_source_get_output_flags oracle_push_enable_get_flags
#define obs_source_get_name oracle_push_enable_get_name
#include "source-push-to-enable-signal.inc"
void oracle_obs_source_enable_push_to_mute(obs_source_t *source, bool enabled);
void oracle_obs_source_enable_push_to_talk(obs_source_t *source, bool enabled);
#define obs_source_enable_push_to_mute oracle_obs_source_enable_push_to_mute
#define obs_source_enable_push_to_talk oracle_obs_source_enable_push_to_talk
#include "source-push-to-enable.inc"
#undef obs_source_enable_push_to_mute
#undef obs_source_enable_push_to_talk
#undef obs_source_get_output_flags
#undef obs_source_get_name
#include "source_push_to_enable_fields.inc"

void oracle_push_enable_layout(size_t *layout);
void oracle_push_enable_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_push_to_enable_fields);
	layout[1] = _Alignof(struct obs_source_push_to_enable_fields);
	layout[2] = offsetof(struct obs_source_push_to_enable_fields, audio_mutex);
	layout[3] = offsetof(struct obs_source_push_to_enable_fields, mute_enabled);
	layout[4] = offsetof(struct obs_source_push_to_enable_fields, talk_enabled);
	layout[5] = offsetof(struct obs_source_push_to_enable_fields, output_flags);
	layout[6] = offsetof(struct obs_source_push_to_enable_fields, name);
	layout[7] = offsetof(struct obs_source_push_to_enable_fields, signal_changed);
}
struct push_enable_trace {
	unsigned signals[2];
	unsigned logs;
	int level;
	bool at_log[2];
	bool at_signal[2];
	char message[160];
};
struct push_enable_fixture {
	struct obs_source source;
	struct push_enable_trace trace;
	bool ok;
	unsigned kind;
};
static void push_enable_changed(struct push_enable_fixture *f, calldata_t *data, unsigned kind)
{
	f->ok &= kind == f->kind;
	void *source = NULL;
	bool enabled = false;
	f->ok &= calldata_get_ptr(data, "source", &source) && source == &f->source;
	f->ok &= calldata_get_bool(data, "enabled", &enabled);
	f->ok &= enabled == (f->kind ? f->source.push_to_talk_enabled : f->source.push_to_mute_enabled);
	int locked = pthread_mutex_trylock(&f->source.audio_mutex);
	f->ok &= locked != 0;
	if (locked == 0)
		pthread_mutex_unlock(&f->source.audio_mutex);
	f->trace.signals[kind]++;
	f->trace.at_signal[0] = f->source.push_to_mute_enabled;
	f->trace.at_signal[1] = f->source.push_to_talk_enabled;
}
static void push_enable_mute_changed(void *param, calldata_t *data)
{
	push_enable_changed(param, data, 0);
}
static void push_enable_talk_changed(void *param, calldata_t *data)
{
	push_enable_changed(param, data, 1);
}
#if defined(__GNUC__) || defined(__clang__)
static void push_enable_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void push_enable_log(int level, const char *format, va_list args, void *param)
{
	struct push_enable_fixture *f = param;
	f->trace.logs++;
	f->trace.level = level;
	f->trace.at_log[0] = f->source.push_to_mute_enabled;
	f->trace.at_log[1] = f->source.push_to_talk_enabled;
	vsnprintf(f->trace.message, sizeof(f->trace.message), format, args);
}
typedef void (*push_enable_setter)(obs_source_t *, bool);
bool oracle_push_enable_compare(push_enable_setter mute, push_enable_setter talk, const uint8_t *ops, size_t count,
				uint32_t flags, bool initial_mute, bool initial_talk);
bool oracle_push_enable_compare(push_enable_setter mute, push_enable_setter talk, const uint8_t *ops, size_t count,
				uint32_t flags, bool initial_mute, bool initial_talk)
{
	struct push_enable_fixture *f = calloc(2, sizeof(*f));
	if (!f)
		return false;
	bool ok = true;
	for (unsigned i = 0; i < 2; i++) {
		if (pthread_mutex_init(&f[i].source.audio_mutex, NULL) != 0) {
			if (i)
				pthread_mutex_destroy(&f[0].source.audio_mutex);
			free(f);
			return false;
		}
		f[i].ok = true;
		f[i].source.context.name = "push fixture";
		f[i].source.info.output_flags = flags;
		f[i].source.push_to_mute_enabled = initial_mute;
		f[i].source.push_to_talk_enabled = initial_talk;
		f[i].source.push_to_mute_delay = UINT64_MAX;
		f[i].source.push_to_talk_delay = 123;
		f[i].source.push_to_mute_pressed = true;
		f[i].source.push_to_talk_stop_time = UINT64_MAX;
		f[i].source.context.signals = signal_handler_create();
		if (!f[i].source.context.signals) {
			ok = false;
			continue;
		}
		ok &= signal_handler_add(f[i].source.context.signals,
					 "void push_to_mute_changed(ptr source, bool enabled)");
		ok &= signal_handler_add(f[i].source.context.signals,
					 "void push_to_talk_changed(ptr source, bool enabled)");
		signal_handler_connect(f[i].source.context.signals, "push_to_mute_changed", push_enable_mute_changed,
				       &f[i]);
		signal_handler_connect(f[i].source.context.signals, "push_to_talk_changed", push_enable_talk_changed,
				       &f[i]);
	}
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	for (size_t n = 0; ok && n < count; n++) {
		unsigned kind = ops[n] & 1;
		bool enabled = (ops[n] & 2) != 0;
		for (unsigned i = 0; i < 2; i++) {
			f[i].kind = kind;
			struct obs_source before;
			memcpy(&before, &f[i].source, sizeof(before));
			base_set_log_handler(push_enable_log, &f[i]);
			push_enable_setter setter = i ? (kind ? talk : mute)
						      : (kind ? oracle_obs_source_enable_push_to_talk
							      : oracle_obs_source_enable_push_to_mute);
			setter(&f[i].source, enabled);
			ok &= f[i].ok;
			ok &= (kind ? f[i].source.push_to_talk_enabled : f[i].source.push_to_mute_enabled) == enabled;
			/* No field other than the selected enable bit may change. */
			before.push_to_mute_enabled = f[i].source.push_to_mute_enabled;
			before.push_to_talk_enabled = f[i].source.push_to_talk_enabled;
			memcpy(&before.audio_mutex, &f[i].source.audio_mutex, sizeof(before.audio_mutex));
			ok &= memcmp(&before, &f[i].source, sizeof(before)) == 0;
			int unlocked = pthread_mutex_trylock(&f[i].source.audio_mutex);
			ok &= unlocked == 0;
			if (unlocked == 0)
				pthread_mutex_unlock(&f[i].source.audio_mutex);
		}
		ok &= f[0].source.push_to_mute_enabled == f[1].source.push_to_mute_enabled;
		ok &= f[0].source.push_to_talk_enabled == f[1].source.push_to_talk_enabled;
		ok &= memcmp(&f[0].trace, &f[1].trace, sizeof(f[0].trace)) == 0;
	}
	base_set_log_handler(old_handler, old_param);
	for (unsigned i = 0; i < 2; i++) {
		signal_handler_destroy(f[i].source.context.signals);
		pthread_mutex_destroy(&f[i].source.audio_mutex);
	}
	free(f);
	return ok;
}
bool oracle_push_enable_null(push_enable_setter mute, push_enable_setter talk);
bool oracle_push_enable_null(push_enable_setter mute, push_enable_setter talk)
{
	struct push_enable_fixture f = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(push_enable_log, &f);
	bool ok = true;
	for (unsigned i = 0; i < 2; i++) {
		mute(NULL, i != 0);
		ok &= f.trace.logs == i * 2 + 1 && f.trace.level == LOG_DEBUG;
		ok &= strcmp(f.trace.message, "obs_source_enable_push_to_mute: Null 'source' parameter") == 0;
		talk(NULL, i != 0);
		ok &= f.trace.logs == i * 2 + 2 && f.trace.level == LOG_DEBUG;
		ok &= strcmp(f.trace.message, "obs_source_enable_push_to_talk: Null 'source' parameter") == 0;
	}
	base_set_log_handler(old_handler, old_param);
	return ok;
}
