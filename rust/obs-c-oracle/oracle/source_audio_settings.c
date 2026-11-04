/* Test-only oracle: the original libobs/obs-source-audio-settings.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next to
 * the Rust shims (rust/obs-source-seam/src/ffi/source_audio_settings.rs).
 *
 * The Rust shims reach the fields through the seam,
 * libobs/obs-source-audio-settings-seam.c.h, compiled here under its own
 * names, as it stays C in libobs.core-source.
 *
 * A test source is a zeroed struct obs_source with its mutexes, a real signal
 * handler and a small context of its own. Each signal, audio_monitor_create()
 * and audio_monitor_destroy() append to the context's log, so a test compares
 * the order of the calls and the fields each one saw. */

/* Under their own names, so no other test binary that links this library pulls
 * this object in for libobs's global or the monitor calls. */
#define obs oracle_audio_settings_obs
#define audio_monitor_create oracle_audio_settings_monitor_create
#define audio_monitor_destroy oracle_audio_settings_monitor_destroy

#define obs_source_set_volume oracle_obs_source_set_volume
#define obs_source_get_volume oracle_obs_source_get_volume
#define obs_source_set_sync_offset oracle_obs_source_set_sync_offset
#define obs_source_get_sync_offset oracle_obs_source_get_sync_offset
#define obs_source_muted oracle_obs_source_muted
#define obs_source_set_muted oracle_obs_source_set_muted
#define obs_source_set_monitoring_enabled oracle_obs_source_set_monitoring_enabled
#define obs_source_get_monitoring_enabled oracle_obs_source_get_monitoring_enabled
#define obs_source_set_monitoring_type oracle_obs_source_set_monitoring_type
#define obs_source_get_monitoring_type oracle_obs_source_get_monitoring_type
#define obs_source_set_balance_value oracle_obs_source_set_balance_value
#define obs_source_get_balance_value oracle_obs_source_get_balance_value

/* the file's precompiled header brings obs.h, so the oracle_* prototypes come
 * from obs.h through the renames */
#include "obs-source-audio-settings.c.h"

#undef obs_source_get_balance_value
#undef obs_source_set_balance_value
#undef obs_source_get_monitoring_type
#undef obs_source_set_monitoring_type
#undef obs_source_get_monitoring_enabled
#undef obs_source_set_monitoring_enabled
#undef obs_source_set_muted
#undef obs_source_muted
#undef obs_source_get_sync_offset
#undef obs_source_set_sync_offset
#undef obs_source_get_volume
#undef obs_source_set_volume

#include "obs-source-audio-settings-seam.h"
#include "obs-source-audio-settings-seam.c.h"

#include <string.h>

/* libobs has this one in obs.c, under its own name. The tests never free it. */
struct obs_core *obs;

struct settings_context {
	char log[16384];
	size_t len;
	/* what a handler puts in the calldata */
	bool replace;
	double replace_float;
	long long replace_int;
};

obs_source_t *oracle_audio_settings_new(bool private_source, bool replace, double replace_float, long long replace_int,
					float volume, float balance);
void oracle_audio_settings_free(obs_source_t *source);
size_t oracle_audio_settings_state(const obs_source_t *source, char *out, size_t cap);

/* No <stdio.h>: on MSVC it makes this object define snprintf(), and another
 * test binary that links this library then pulls the object in for it. Values
 * are written as the hex of their bits, so they compare exactly. */
static void put_str(char *out, size_t cap, size_t *len, const char *text)
{
	size_t n = strlen(text);

	if (*len + n < cap) {
		memcpy(out + *len, text, n);
		*len += n;
		out[*len] = 0;
	}
}

static void put_hex(char *out, size_t cap, size_t *len, uint64_t value, int digits)
{
	char text[17];

	for (int i = 0; i < digits; i++)
		text[i] = "0123456789abcdef"[(value >> ((digits - 1 - i) * 4)) & 15];
	text[digits] = 0;
	put_str(out, cap, len, text);
}

static void put_f32(char *out, size_t cap, size_t *len, float value)
{
	uint32_t bits;

	memcpy(&bits, &value, sizeof(bits));
	put_hex(out, cap, len, bits, 8);
}

static void put_fields(char *out, size_t cap, size_t *len, const obs_source_t *source)
{
	put_str(out, cap, len, "vol ");
	put_f32(out, cap, len, source->user_volume);
	put_str(out, cap, len, " sync ");
	put_hex(out, cap, len, (uint64_t)source->sync_offset, 16);
	put_str(out, cap, len, " bal ");
	put_f32(out, cap, len, source->balance);
	put_str(out, cap, len, " type ");
	put_hex(out, cap, len, (uint32_t)source->monitoring_type, 8);
	put_str(out, cap, len, source->user_muted ? " muted" : " unmuted");
	put_str(out, cap, len, source->monitor_only_muted ? " only" : " notonly");
	put_str(out, cap, len, source->monitoring_enabled ? " on" : " off");
}

/* name, value and every field the call saw */
static void add_log(obs_source_t *source, const char *name, uint64_t value, int digits)
{
	struct settings_context *c = source->context.data;
	size_t cap = sizeof(c->log);

	put_str(c->log, cap, &c->len, name);
	put_str(c->log, cap, &c->len, " ");
	if (digits)
		put_hex(c->log, cap, &c->len, value, digits);
	put_str(c->log, cap, &c->len, " [");
	put_fields(c->log, cap, &c->len, source);
	put_str(c->log, cap, &c->len, "];");
}

static uint64_t f64_bits(double value)
{
	uint64_t bits;

	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

static void on_float(obs_source_t *source, calldata_t *cd, const char *signal, const char *field)
{
	struct settings_context *c = source->context.data;

	add_log(source, signal, f64_bits(calldata_float(cd, field)), 16);
	if (c->replace)
		calldata_set_float(cd, field, c->replace_float);
}

static void on_volume(void *param, calldata_t *cd)
{
	on_float(param, cd, "volume", "volume");
}

static void on_core_volume(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	obs_source_t *source = calldata_ptr(cd, "source");

	add_log(source, "source_volume", f64_bits(calldata_float(cd, "volume")), 16);
}

static void on_balance(void *param, calldata_t *cd)
{
	on_float(param, cd, "audio_balance", "balance");
}

static void on_sync(void *param, calldata_t *cd)
{
	obs_source_t *source = param;
	struct settings_context *c = source->context.data;

	add_log(source, "audio_sync", (uint64_t)calldata_int(cd, "offset"), 16);
	if (c->replace)
		calldata_set_int(cd, "offset", c->replace_int);
}

static void on_mute(void *param, calldata_t *cd)
{
	add_log(param, "mute", calldata_bool(cd, "muted") ? 1 : 0, 1);
}

static void on_monitor(void *param, calldata_t *cd)
{
	add_log(param, "monitor", calldata_bool(cd, "monitor") ? 1 : 0, 1);
}

static void on_audio_monitoring(void *param, calldata_t *cd)
{
	add_log(param, "audio_monitoring", (uint64_t)calldata_int(cd, "type"), 16);
}

struct audio_monitor *audio_monitor_create(obs_source_t *source)
{
	add_log(source, "monitor_create", 0, 0);
	return (struct audio_monitor *)source;
}

void audio_monitor_destroy(struct audio_monitor *monitor)
{
	add_log((obs_source_t *)monitor, "monitor_destroy", 0, 0);
}

static pthread_once_t core_once = PTHREAD_ONCE_INIT;

static void core_init(void)
{
	obs = bzalloc(sizeof(*obs));
	obs->signals = signal_handler_create();
	signal_handler_add(obs->signals, "void source_volume(ptr source, in out float volume)");
	signal_handler_connect(obs->signals, "source_volume", on_core_volume, NULL);
}

obs_source_t *oracle_audio_settings_new(bool private_source, bool replace, double replace_float, long long replace_int,
					float volume, float balance)
{
	obs_source_t *source = bzalloc(sizeof(*source));
	struct settings_context *c = bzalloc(sizeof(*c));

	pthread_once(&core_once, core_init);

	c->replace = replace;
	c->replace_float = replace_float;
	c->replace_int = replace_int;
	source->context.data = c;
	source->context.private = private_source;
	source->user_volume = volume;
	source->balance = balance;
	pthread_mutex_init(&source->audio_buf_mutex, NULL);
	pthread_mutex_init(&source->audio_actions_mutex, NULL);

	signal_handler_t *h = signal_handler_create();
	source->context.signals = h;
	signal_handler_add(h, "void volume(ptr source, in out float volume)");
	signal_handler_add(h, "void audio_balance(ptr source, in out float balance)");
	signal_handler_add(h, "void audio_sync(ptr source, in out int offset)");
	signal_handler_add(h, "void mute(ptr source, bool muted)");
	signal_handler_add(h, "void monitor(ptr source, bool monitor)");
	signal_handler_add(h, "void audio_monitoring(ptr source, int type)");
	signal_handler_connect(h, "volume", on_volume, source);
	signal_handler_connect(h, "audio_balance", on_balance, source);
	signal_handler_connect(h, "audio_sync", on_sync, source);
	signal_handler_connect(h, "mute", on_mute, source);
	signal_handler_connect(h, "monitor", on_monitor, source);
	signal_handler_connect(h, "audio_monitoring", on_audio_monitoring, source);
	return source;
}

void oracle_audio_settings_free(obs_source_t *source)
{
	signal_handler_destroy(source->context.signals);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	pthread_mutex_destroy(&source->audio_actions_mutex);
	da_free(source->audio_actions);
	bfree(source->context.data);
	bfree(source);
}

/* The log, then the queued actions (their values, as the clocks differ), then
 * every field, and whether the two mutexes are free. */
size_t oracle_audio_settings_state(const obs_source_t *source, char *out, size_t cap)
{
	const struct settings_context *c = source->context.data;
	obs_source_t *s = (obs_source_t *)(uintptr_t)source;
	size_t len = 0;

	out[0] = 0;
	put_str(out, cap, &len, c->log);
	put_str(out, cap, &len, "|");

	for (size_t i = 0; i < source->audio_actions.num; i++) {
		const struct audio_action *a = &source->audio_actions.array[i];
		if (a->type == AUDIO_ACTION_VOL) {
			put_str(out, cap, &len, "action vol ");
			put_f32(out, cap, &len, a->vol);
		} else {
			put_str(out, cap, &len, a->set ? "action set 1" : "action set 0");
			put_hex(out, cap, &len, (uint32_t)a->type, 8);
		}
		put_str(out, cap, &len, ";");
	}

	bool buf_free = pthread_mutex_trylock(&s->audio_buf_mutex) == 0;
	if (buf_free)
		pthread_mutex_unlock(&s->audio_buf_mutex);
	bool actions_free = pthread_mutex_trylock(&s->audio_actions_mutex) == 0;
	if (actions_free)
		pthread_mutex_unlock(&s->audio_actions_mutex);

	put_str(out, cap, &len, "|");
	put_fields(out, cap, &len, source);
	put_str(out, cap, &len, buf_free && actions_free ? " free" : " held");
	return len;
}
