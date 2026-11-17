/* Original extracted reroute over the real private encoder layout. The
 * registry is a stand-in core behind the registry oracle's real find_encoder;
 * the create-only permission is a per-thread test input. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>
#include "encoder_reroute_fields.inc"

struct obs_core *oracle_encoder_registry_exchange_core(struct obs_core *core);

struct reroute_fixture {
	struct obs_encoder encoder;
	struct obs_encoder_info replacement;
	struct obs_encoder_info original;
	unsigned flags;
	unsigned calls;
	bool callback_valid;
};
#if defined(_MSC_VER)
static __declspec(thread) struct reroute_fixture *reroute_current;
#else
static _Thread_local struct reroute_fixture *reroute_current;
#endif

/* Stand-in for the initializer's thread-local permission in obs-encoder.c.h:
 * only a running fixture on this thread with bit 0 set allows a reroute. */
bool obs_encoder_reroute_allowed(void)
{
	return reroute_current && (reroute_current->flags & 1) != 0;
}

void *oracle_obs_encoder_create_rerouted(obs_encoder_t *encoder, const char *reroute_id);
#define obs_encoder_create_rerouted oracle_obs_encoder_create_rerouted
#include "libobs/obs-encoder-reroute.c.h"
#undef obs_encoder_create_rerouted

typedef void *(*reroute_fn)(obs_encoder_t *, const char *);
static void *reroute_callback(obs_data_t *settings, obs_encoder_t *encoder)
{
	struct reroute_fixture *f = reroute_current;
	f->calls++;
	f->callback_valid = encoder == &f->encoder && settings == (obs_data_t *)&f->flags &&
			    memcmp(&encoder->info, &f->replacement, sizeof(encoder->info)) == 0 &&
			    memcmp(&encoder->orig_info, &f->original, sizeof(encoder->orig_info)) == 0;
	return (f->flags & 2) ? NULL : &f->calls;
}

/* flags: bit 0 admission, bit 1 the callback fails, bit 2 NULL encoder,
 * bits 3-4 the ID ("replacement", "", "missing", NULL). Returns a trace:
 * bit 0 the callback's result came back, bits 1-2 the callback count, bit 3
 * the callback saw the whole table and its arguments, bit 4 the active table
 * is the replacement, bit 5 the original info is unchanged, bit 6 the active
 * table is unchanged, bit 7 the registry is unchanged. */
uint64_t oracle_encoder_reroute_run(reroute_fn run, unsigned flags, int original_type, int replacement_type,
				    const char *original_codec, const char *replacement_codec, unsigned caps);
uint64_t oracle_encoder_reroute_run(reroute_fn run, unsigned flags, int original_type, int replacement_type,
				    const char *original_codec, const char *replacement_codec, unsigned caps)
{
	struct reroute_fixture f;
	memset(&f, 0, sizeof(f));
	f.flags = flags;
	f.encoder.orig_info.id = "original";
	f.encoder.orig_info.type = (enum obs_encoder_type)original_type;
	f.encoder.orig_info.codec = original_codec;
	f.encoder.orig_info.caps = ~caps;
	f.original = f.encoder.orig_info;
	/* Deliberately different active info: match against original, not active. */
	f.encoder.info = f.original;
	f.encoder.info.type = original_type == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
	f.encoder.info.codec = "unrelated";
	struct obs_encoder_info before = f.encoder.info;
	f.encoder.context.settings = (obs_data_t *)&f.flags;
	f.replacement.id = "replacement";
	f.replacement.type = (enum obs_encoder_type)replacement_type;
	f.replacement.codec = replacement_codec;
	f.replacement.caps = caps;
	f.replacement.create = reroute_callback;
	struct obs_encoder_info registered = f.replacement;

	struct obs_core core;
	memset(&core, 0, sizeof(core));
	core.encoder_types.array = &f.replacement;
	core.encoder_types.num = 1;
	core.encoder_types.capacity = 1;
	struct obs_core *previous = oracle_encoder_registry_exchange_core(&core);
	struct reroute_fixture *previous_fixture = reroute_current;
	reroute_current = &f;
	const char *ids[] = {"replacement", "", "missing", NULL};
	void *result = run((flags & 4) ? NULL : &f.encoder, ids[(flags >> 3) & 3]);
	reroute_current = previous_fixture;
	oracle_encoder_registry_exchange_core(previous);

	uint64_t trace = (result == &f.calls) | ((uint64_t)f.calls << 1) | ((uint64_t)f.callback_valid << 3) |
			 ((uint64_t)(memcmp(&f.encoder.info, &f.replacement, sizeof(f.replacement)) == 0) << 4) |
			 ((uint64_t)(memcmp(&f.encoder.orig_info, &f.original, sizeof(f.original)) == 0) << 5) |
			 ((uint64_t)(memcmp(&f.encoder.info, &before, sizeof(before)) == 0) << 6) |
			 ((uint64_t)(memcmp(&f.replacement, &registered, sizeof(registered)) == 0) << 7);
	return trace;
}

void oracle_encoder_reroute_layout(size_t *out);
void oracle_encoder_reroute_layout(size_t *out)
{
	out[0] = sizeof(struct obs_encoder_reroute_fields);
	out[1] = _Alignof(struct obs_encoder_reroute_fields);
	out[2] = offsetof(struct obs_encoder_reroute_fields, active);
	out[3] = offsetof(struct obs_encoder_reroute_fields, replacement);
	out[4] = offsetof(struct obs_encoder_reroute_fields, size);
	out[5] = offsetof(struct obs_encoder_reroute_fields, original_type);
	out[6] = offsetof(struct obs_encoder_reroute_fields, replacement_type);
	out[7] = offsetof(struct obs_encoder_reroute_fields, original_codec);
	out[8] = offsetof(struct obs_encoder_reroute_fields, replacement_codec);
	out[9] = offsetof(struct obs_encoder_reroute_fields, settings);
	out[10] = offsetof(struct obs_encoder_reroute_fields, create);
}

/* The adapter borrows the real fields of a live encoder and info. */
bool oracle_encoder_reroute_fields_match(void);
bool oracle_encoder_reroute_fields_match(void)
{
	struct obs_encoder encoder;
	struct obs_encoder_info info;
	memset(&encoder, 0, sizeof(encoder));
	memset(&info, 0, sizeof(info));
	encoder.orig_info.type = OBS_ENCODER_AUDIO;
	encoder.orig_info.codec = "aac";
	encoder.context.settings = (obs_data_t *)&info;
	info.type = OBS_ENCODER_VIDEO;
	info.codec = "h264";
	info.create = reroute_callback;
	struct obs_encoder_reroute_fields fields = obs_encoder_reroute_fields(&encoder, &info);
	return fields.active == (void *)&encoder.info && fields.replacement == (const void *)&info &&
	       fields.size == sizeof(info) && fields.original_type == OBS_ENCODER_AUDIO &&
	       fields.replacement_type == OBS_ENCODER_VIDEO && fields.original_codec == encoder.orig_info.codec &&
	       fields.replacement_codec == info.codec && fields.settings == encoder.context.settings &&
	       fields.create == reroute_callback;
}

struct reroute_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void reroute_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void reroute_capture(int level, const char *format, va_list args, void *param)
{
	struct reroute_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*reroute_get_log)(log_handler_t *, void **);
typedef void (*reroute_set_log)(log_handler_t, void *);

/* Each NULL parameter writes exactly obs_ptr_valid's one debug line. */
bool oracle_encoder_reroute_logs(reroute_fn run, reroute_get_log get_log, reroute_set_log set_log);
bool oracle_encoder_reroute_logs(reroute_fn run, reroute_get_log get_log, reroute_set_log set_log)
{
	struct reroute_log log;
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	memset(&log, 0, sizeof(log));
	set_log(reroute_capture, &log);
	bool ok = run(NULL, NULL) == NULL && log.count == 1 && log.level == LOG_DEBUG &&
		  strcmp(log.message, "obs_encoder_reroute: Null 'encoder' parameter") == 0;
	memset(&log, 0, sizeof(log));
	struct obs_encoder encoder;
	memset(&encoder, 0, sizeof(encoder));
	ok = ok && run(&encoder, NULL) == NULL && log.count == 1 && log.level == LOG_DEBUG &&
	     strcmp(log.message, "obs_encoder_reroute: Null 'reroute_id' parameter") == 0;
	set_log(old_handler, old_param);
	return ok;
}
