/* Original reroute implementation and real private-layout adapter. Registry
 * and create-only permission are test-owned, per-thread inputs. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>
#include "encoder_reroute_fields.inc"

struct reroute_fixture {
	struct obs_encoder encoder;
	struct obs_encoder_info replacement;
	struct obs_encoder_info original;
	unsigned flags;
	unsigned calls;
	bool callback_valid;
};
static THREAD_LOCAL struct reroute_fixture *reroute_current;

bool obs_encoder_reroute_allowed(void)
{
	return (reroute_current->flags & 1) != 0;
}
struct obs_encoder_info *find_encoder(const char *id)
{
	return strcmp(id, "replacement") == 0 ? &reroute_current->replacement : NULL;
}

void *oracle_obs_encoder_create_rerouted(obs_encoder_t *encoder, const char *reroute_id);
#define obs_encoder_create_rerouted oracle_obs_encoder_create_rerouted
#include "obs-encoder-reroute.c.h"
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

uint64_t oracle_encoder_reroute_run(reroute_fn run, unsigned flags, int original_type, int replacement_type,
				    const char *original_codec, const char *replacement_codec, unsigned caps);
uint64_t oracle_encoder_reroute_run(reroute_fn run, unsigned flags, int original_type, int replacement_type,
				    const char *original_codec, const char *replacement_codec, unsigned caps)
{
	struct reroute_fixture f = {0};
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
	reroute_current = &f;
	const char *ids[] = {"replacement", "", "missing", NULL};
	void *result = run((flags & 4) ? NULL : &f.encoder, ids[(flags >> 3) & 3]);
	uint64_t trace = (result == &f.calls) | ((uint64_t)f.calls << 1) | ((uint64_t)f.callback_valid << 3) |
			 ((uint64_t)(memcmp(&f.encoder.info, &f.replacement, sizeof(f.replacement)) == 0) << 4) |
			 ((uint64_t)(memcmp(&f.encoder.orig_info, &f.original, sizeof(f.original)) == 0) << 5) |
			 ((uint64_t)(memcmp(&f.encoder.info, &before, sizeof(before)) == 0) << 6);
	reroute_current = NULL;
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
bool oracle_encoder_reroute_logs(reroute_fn run);
bool oracle_encoder_reroute_logs(reroute_fn run)
{
	struct reroute_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(reroute_capture, &log);
	bool ok = run(NULL, NULL) == NULL && log.count == 1 && log.level == LOG_DEBUG &&
		  strcmp(log.message, "obs_encoder_reroute: Null 'encoder' parameter") == 0;
	log.count = 0;
	struct obs_encoder encoder = {0};
	ok &= run(&encoder, NULL) == NULL && log.count == 1 && log.level == LOG_DEBUG &&
	      strcmp(log.message, "obs_encoder_reroute: Null 'reroute_id' parameter") == 0;
	base_set_log_handler(old_handler, old_param);
	return ok;
}
