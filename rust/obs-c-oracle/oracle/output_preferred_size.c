/* The original legacy preferred-size wrapper (obs-output-preferred-size.c.h),
 * over the real private output layout, with its indexed setter replaced by a
 * recorder both sides share. Refs obs-rust/obs-studio#4816. */
#include <stdio.h>
#include <string.h>
#include "obs-internal.h"

struct preferred_size_call {
	void *output;
	uint32_t width;
	uint32_t height;
	size_t idx;
};
struct preferred_size_record {
	unsigned count;
	struct preferred_size_call last;
};
static _Thread_local struct preferred_size_record preferred_size_record;

void oracle_preferred_size_record(obs_output_t *output, uint32_t width, uint32_t height, size_t idx);
void oracle_preferred_size_record(obs_output_t *output, uint32_t width, uint32_t height, size_t idx)
{
	preferred_size_record.count++;
	preferred_size_record.last = (struct preferred_size_call){output, width, height, idx};
}

/* The original helper from obs-output.c.h, which the extracted file declares. */
static inline bool log_flag_video(const struct obs_output *output, const char *func_name)
{
	bool ret = (output->info.flags & OBS_OUTPUT_VIDEO) != 0;
	if (!ret)
		blog(LOG_WARNING, "Output '%s': Tried to use %s on a non-video output", output->context.name,
		     func_name);
	return ret;
}

void oracle_obs_output_set_preferred_size(obs_output_t *output, uint32_t width, uint32_t height);
#define obs_output_set_preferred_size oracle_obs_output_set_preferred_size
#define obs_output_set_preferred_size2 oracle_preferred_size_record
#include "obs-output-preferred-size.c.h"
#undef obs_output_set_preferred_size
#undef obs_output_set_preferred_size2

struct preferred_size_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void preferred_size_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void preferred_size_capture(int level, const char *format, va_list args, void *param)
{
	struct preferred_size_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	char *prefix = strstr(log->message, "oracle_");
	if (prefix)
		memmove(prefix, prefix + 7, strlen(prefix + 7) + 1);
}

typedef void (*preferred_size_fn)(obs_output_t *, uint32_t, uint32_t);
typedef void (*preferred_size_get_log)(log_handler_t *, void **);
typedef void (*preferred_size_set_log)(log_handler_t, void *);

struct preferred_size_observation {
	struct preferred_size_log log;
	struct preferred_size_record record;
	bool unchanged;
};

static struct preferred_size_observation preferred_size_observe(preferred_size_fn set, bool present, uint32_t flags,
								bool active, uint32_t width, uint32_t height,
								preferred_size_set_log set_log)
{
	struct preferred_size_observation o = {0};
	struct obs_output output = {0};
	struct obs_output before;
	output.context.name = "preferred-%s";
	output.info.flags = flags;
	output.active = active;
	output.scaled_width = 37;
	output.scaled_height = 53;
	memcpy(&before, &output, sizeof(output));
	memset(&preferred_size_record, 0, sizeof(preferred_size_record));
	set_log(preferred_size_capture, &o.log);
	set(present ? &output : NULL, width, height);
	o.record = preferred_size_record;
	if (o.record.last.output == &output)
		o.record.last.output = (void *)(uintptr_t)1;
	o.unchanged = memcmp(&output, &before, sizeof(output)) == 0;
	return o;
}

/* Runs `set` (the Rust wrapper over the same recorder) and the original on
 * identical fixtures; true when logs, delegation and state all match. */
bool oracle_output_preferred_size_compare(preferred_size_fn set, bool present, uint32_t flags, bool active,
					  uint32_t width, uint32_t height, preferred_size_get_log get_log,
					  preferred_size_set_log set_log);
bool oracle_output_preferred_size_compare(preferred_size_fn set, bool present, uint32_t flags, bool active,
					  uint32_t width, uint32_t height, preferred_size_get_log get_log,
					  preferred_size_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct preferred_size_observation r =
		preferred_size_observe(set, present, flags, active, width, height, set_log);
	struct preferred_size_observation c = preferred_size_observe(oracle_obs_output_set_preferred_size, present,
								     flags, active, width, height, set_log);
	set_log(old_handler, old_param);
	return r.log.count == c.log.count && r.log.level == c.log.level && strcmp(r.log.message, c.log.message) == 0 &&
	       r.record.count == c.record.count && r.record.last.output == c.record.last.output &&
	       r.record.last.width == c.record.last.width && r.record.last.height == c.record.last.height &&
	       r.record.last.idx == c.record.last.idx && r.unchanged && c.unchanged;
}

/* Test-only link stand-in: the Rust export names libobs's indexed setter,
 * which the MSVC, MinGW and macOS linkers want resolved in the test binaries
 * that link obs-output without libobs. The tests never reach it: they run
 * the wrapper with the recorder above as its delegate. */
void obs_output_set_preferred_size2(obs_output_t *output, uint32_t width, uint32_t height, size_t idx)
{
	UNUSED_PARAMETER(output);
	UNUSED_PARAMETER(width);
	UNUSED_PARAMETER(height);
	UNUSED_PARAMETER(idx);
}
