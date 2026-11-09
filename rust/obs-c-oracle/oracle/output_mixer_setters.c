/* Exact original setters, real private layout and captured observable state. */
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "obs-internal.h"
/* Avoid the interleave suite's synthetic-layout obs_output_get_name double:
 * this is the unchanged getter body over this oracle's real OBS layout. */
static const char *setter_output_get_name(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_get_name") ? output->context.name : NULL;
}
void oracle_obs_output_set_mixer(obs_output_t *output, size_t mixer_idx);
void oracle_obs_output_set_mixers(obs_output_t *output, size_t mixers);
#define obs_output_get_name setter_output_get_name
#define obs_output_set_mixer oracle_obs_output_set_mixer
#define obs_output_set_mixers oracle_obs_output_set_mixers
#define obs_output_slots_mixer_in_range oracle_obs_output_slots_mixer_in_range
#define obs_output_slots_mixer_mask oracle_obs_output_slots_mixer_mask
#include "obs-output-mixer-setters.c.h"
#undef obs_output_set_mixer
#undef obs_output_get_name
#undef obs_output_set_mixers
#undef obs_output_slots_mixer_in_range
#undef obs_output_slots_mixer_mask
#include "output_mixer_setters_fields.inc"

typedef void (*setter_fn)(obs_output_t *, size_t);
typedef void (*get_log_fn)(log_handler_t *, void **);
typedef void (*set_log_fn)(log_handler_t, void *);
struct setter_observation {
	size_t mask;
	unsigned count;
	int level;
	char text[256];
	bool unchanged;
};
#if defined(__GNUC__) || defined(__clang__)
static void setter_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void setter_capture(int level, const char *format, va_list args, void *param)
{
	struct setter_observation *result = param;
	result->count++;
	result->level = level;
	vsnprintf(result->text, sizeof(result->text), format, args);
	char *prefix = strstr(result->text, "oracle_obs_output_");
	if (prefix)
		memmove(prefix, prefix + 7, strlen(prefix + 7) + 1);
}
static void observe(setter_fn set, size_t initial, uint32_t flags, bool active_value, bool present, size_t value,
		    get_log_fn get_log, set_log_fn set_log, struct setter_observation *result)
{
	struct obs_output output = {0};
	output.context.name = "setter fixture";
	output.mixer_mask = initial;
	output.info.flags = flags;
	os_atomic_set_bool(&output.active, active_value);
	unsigned char before[sizeof(output)];
	memcpy(before, &output, sizeof(output));
	log_handler_t old;
	void *param;
	get_log(&old, &param);
	set_log(setter_capture, result);
	set(present ? &output : NULL, value);
	set_log(old, param);
	result->mask = output.mixer_mask;
	output.mixer_mask = initial;
	result->unchanged = memcmp(before, &output, sizeof(output)) == 0;
}
bool oracle_output_mixer_setters_compare(setter_fn rust, bool single, size_t initial, uint32_t flags, bool active_value,
					 bool present, size_t value, get_log_fn get_log, set_log_fn set_log);
bool oracle_output_mixer_setters_compare(setter_fn rust, bool single, size_t initial, uint32_t flags, bool active_value,
					 bool present, size_t value, get_log_fn get_log, set_log_fn set_log)
{
	struct setter_observation r = {0}, c = {0};
	observe(rust, initial, flags, active_value, present, value, get_log, set_log, &r);
	observe(single ? oracle_obs_output_set_mixer : oracle_obs_output_set_mixers, initial, flags, active_value,
		present, value, get_log, set_log, &c);
	return r.mask == c.mask && r.count == c.count && r.level == c.level && strcmp(r.text, c.text) == 0 &&
	       r.unchanged && c.unchanged;
}
void oracle_output_mixer_setters_layout(size_t *layout);
void oracle_output_mixer_setters_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_mixer_setters_fields);
	layout[1] = _Alignof(struct obs_output_mixer_setters_fields);
	layout[2] = offsetof(struct obs_output_mixer_setters_fields, mask);
	layout[3] = offsetof(struct obs_output_mixer_setters_fields, active);
	layout[4] = offsetof(struct obs_output_mixer_setters_fields, name);
	layout[5] = offsetof(struct obs_output_mixer_setters_fields, flags);
}
