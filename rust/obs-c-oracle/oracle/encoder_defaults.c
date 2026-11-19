/* Original C defaults and real production adapter over real-header fixtures. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "obs-internal.h"

obs_data_t *oracle_obs_encoder_defaults(const char *id);
obs_data_t *oracle_obs_encoder_get_defaults(const obs_encoder_t *encoder);
#define obs_encoder_defaults oracle_obs_encoder_defaults
#define obs_encoder_get_defaults oracle_obs_encoder_get_defaults
#include "encoder-defaults.inc"
#undef obs_encoder_defaults
#undef obs_encoder_get_defaults
#include "encoder-defaults-fields.inc"

struct ed_fixture {
	unsigned flags;
	uint32_t seed;
	uint64_t trace;
	unsigned wrong_calls;
	bool identity_ok;
	bool before_ok;
	obs_data_t *settings;
	unsigned logs;
	int level;
	char message[128];
};
static struct ed_fixture *ed_active;
struct obs_core *oracle_encoder_registry_exchange_core(struct obs_core *core);
static void ed_legacy(obs_data_t *settings)
{
	ed_active->trace = ed_active->trace * 10 + 1;
	ed_active->identity_ok &= settings != NULL;
	ed_active->settings = settings;
	obs_data_set_default_int(settings, "winner", ed_active->seed);
	obs_data_set_default_bool(settings, "legacy", true);
}
static void ed_modern(obs_data_t *settings, void *type_data)
{
	struct ed_fixture *f = ed_active;
	f->trace = f->trace * 10 + 2;
	f->identity_ok &= settings != NULL && type_data == ((f->flags & 4) ? NULL : f);
	if (f->flags & 1) {
		f->identity_ok &= settings == f->settings;
		f->before_ok &= obs_data_get_int(settings, "winner") == f->seed &&
				obs_data_get_bool(settings, "legacy");
	} else {
		f->before_ok &= !obs_data_has_default_value(settings, "winner");
	}
	f->settings = settings;
	obs_data_set_default_int(settings, "winner", f->seed ^ 0x1234u);
}
static void ed_wrong(obs_data_t *settings)
{
	(void)settings;
	ed_active->wrong_calls++;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void ed_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	ed_active->logs++;
	ed_active->level = level;
	vsnprintf(ed_active->message, sizeof(ed_active->message), format, args);
}

typedef obs_data_t *(*ed_type_getter)(const char *id);
typedef obs_data_t *(*ed_instance_getter)(const obs_encoder_t *encoder);
void oracle_encoder_defaults_run(ed_type_getter by_id, ed_instance_getter by_instance, unsigned flags, uint32_t seed,
				 uint64_t *report);
void oracle_encoder_defaults_run(ed_type_getter by_id, ed_instance_getter by_instance, unsigned flags, uint32_t seed,
				 uint64_t *report)
{
	struct ed_fixture f = {.flags = flags, .seed = seed, .identity_ok = true, .before_ok = true};
	ed_active = &f;
	struct obs_encoder encoder = {0};
	encoder.info.id = "ed";
	encoder.info.get_defaults = (flags & 1) ? ed_legacy : NULL;
	encoder.info.get_defaults2 = (flags & 2) ? ed_modern : NULL;
	encoder.info.type_data = (flags & 4) ? NULL : &f;
	encoder.orig_info.get_defaults = ed_wrong;
	encoder.context.settings = obs_data_create();
	obs_data_set_int(encoder.context.settings, "existing", 123);
	struct obs_encoder_info registered = encoder.info;
	if (flags & 32) {
		/* The registered ID and orig_info disagree with active instance callbacks. */
		registered.get_defaults = ed_wrong;
		registered.get_defaults2 = NULL;
	}
	struct obs_core registry = {0};
	registry.encoder_types.array = &registered;
	registry.encoder_types.num = 1;
	registry.encoder_types.capacity = 1;
	struct obs_core *previous = oracle_encoder_registry_exchange_core(&registry);
	struct obs_encoder before;
	memcpy(&before, &encoder, sizeof(before));
	log_handler_t saved;
	void *saved_param;
	base_get_log_handler(&saved, &saved_param);
	base_set_log_handler(ed_log, NULL);
	memset(report, 0, 20 * sizeof(*report));
	obs_data_t *first = (flags & 32) ? by_instance((flags & 16) ? NULL : &encoder)
					 : by_id((flags & 8) ? "missing-ed" : "ed");
	report[0] = first != NULL;
	if (first) {
		report[1] = f.settings == first;
		report[2] = (uint64_t)obs_data_get_int(first, "winner");
		report[3] = obs_data_has_default_value(first, "winner");
		report[4] = obs_data_has_user_value(first, "winner");
		report[5] = obs_data_get_bool(first, "legacy");
		obs_data_item_t *item = obs_data_first(first);
		report[19] = (flags & 3) != 0 || item == NULL;
		obs_data_item_release(&item);
		report[6] = !obs_data_has_user_value(first, "existing") &&
			    !obs_data_has_user_value(first, "caller-only");
		obs_data_set_int(first, "caller-only", 99);
		obs_data_set_default_int(first, "winner", 77);
		obs_data_t *second = (flags & 32) ? by_instance(&encoder) : by_id("ed");
		report[7] = first != second && first != encoder.context.settings && second != encoder.context.settings;
		report[8] = !obs_data_has_user_value(second, "caller-only") &&
			    !obs_data_has_user_value(second, "existing") &&
			    obs_data_get_int(second, "winner") ==
				    ((flags & 2) ? (seed ^ 0x1234u) : ((flags & 1) ? seed : 0));
		obs_data_release(first);
		obs_data_set_int(second, "caller-only", 42);
		obs_data_addref(second);
		obs_data_release(second);
		report[9] = obs_data_get_int(second, "caller-only") == 42;
		obs_data_release(second);
		obs_data_t *third = (flags & 32) ? by_instance(&encoder) : by_id("ed");
		report[10] = third != NULL && !obs_data_has_user_value(third, "caller-only") &&
			     obs_data_get_int(third, "winner") ==
				     ((flags & 2) ? (seed ^ 0x1234u) : ((flags & 1) ? seed : 0));
		obs_data_release(third);
	}
	report[11] = f.trace;
	report[12] = f.identity_ok && f.before_ok;
	report[13] = f.wrong_calls;
	report[14] = obs_data_get_int(encoder.context.settings, "existing") == 123 &&
		     !obs_data_has_default_value(encoder.context.settings, "winner");
	report[15] = memcmp(&before, &encoder, sizeof(encoder)) == 0;
	report[16] = f.logs;
	report[17] = (uint64_t)f.level;
	report[18] = strcmp(f.message, "obs_encoder_defaults: Null 'encoder' parameter") == 0;
	if (!report[0])
		report[19] = 1;
	base_set_log_handler(saved, saved_param);
	obs_data_release(encoder.context.settings);
	oracle_encoder_registry_exchange_core(previous);
	ed_active = NULL;
}
void oracle_encoder_defaults_layout(size_t *layout);
void oracle_encoder_defaults_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_defaults_fields);
	layout[1] = _Alignof(struct obs_encoder_defaults_fields);
	layout[2] = offsetof(struct obs_encoder_defaults_fields, legacy);
	layout[3] = offsetof(struct obs_encoder_defaults_fields, modern);
	layout[4] = offsetof(struct obs_encoder_defaults_fields, type_data);
}
