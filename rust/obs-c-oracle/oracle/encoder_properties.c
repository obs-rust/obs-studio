/* Real extracted C and production adapter, with a one-type registry fixture. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_get_encoder_properties oracle_obs_get_encoder_properties
#define obs_encoder_properties oracle_obs_encoder_properties
#include "obs-encoder-properties.c.h"
#undef obs_get_encoder_properties
#undef obs_encoder_properties
#include "encoder_properties_fields.inc"

struct ep_fixture {
	struct obs_encoder encoder;
	struct obs_encoder_info info;
	uint64_t trace;
	uint32_t seed;
	bool null_props;
};
/* Tests re-exec before using this private registry, just like service fixtures. */
static struct ep_fixture *ep_active;
struct obs_encoder_info *find_encoder(const char *id)
{
	return ep_active && strcmp(id, "ep") == 0 ? &ep_active->info : NULL;
}
static void ep_event(uint64_t event)
{
	ep_active->trace = ep_active->trace * 131 + event;
}
static void ep_defaults(obs_data_t *settings)
{
	ep_event(1);
	obs_data_set_default_int(settings, "value", ep_active->seed);
}
static void ep_defaults2(obs_data_t *settings, void *type_data)
{
	ep_event(type_data == ep_active ? 2 : 99);
	obs_data_set_default_int(settings, "value", ep_active->seed ^ 0x1234u);
}
static bool ep_modified(void *data, obs_properties_t *props, obs_property_t *property, obs_data_t *settings)
{
	(void)props;
	(void)property;
	ep_event(data == ep_active ? 3 : 98);
	ep_event((uint64_t)obs_data_get_int(settings, "value"));
	return false;
}
static obs_properties_t *ep_properties(void *data)
{
	ep_event(data == NULL ? 4 : data == &ep_active->encoder ? 5 : 97);
	if (ep_active->null_props)
		return NULL;
	obs_properties_t *props = obs_properties_create();
	obs_property_t *p = obs_properties_add_int(props, "value", "Value", 0, 2147483647, 1);
	obs_property_set_modified_callback2(p, ep_modified, ep_active);
	return props;
}
static obs_properties_t *ep_properties2(void *data, void *type_data)
{
	ep_event(type_data == ep_active ? 6 : 96);
	return ep_properties(data);
}

typedef obs_properties_t *(*ep_type_getter)(const char *id);
typedef obs_properties_t *(*ep_instance_getter)(const obs_encoder_t *encoder);
uint64_t oracle_encoder_properties_run(ep_type_getter type_getter, ep_instance_getter instance_getter, unsigned flags,
				       uint32_t seed, uint32_t value, bool instance);
uint64_t oracle_encoder_properties_run(ep_type_getter type_getter, ep_instance_getter instance_getter, unsigned flags,
				       uint32_t seed, uint32_t value, bool instance)
{
	struct ep_fixture fixture = {0};
	ep_active = &fixture;
	fixture.seed = seed;
	fixture.null_props = (flags & 16) != 0;
	fixture.info.get_properties = flags & 1 ? ep_properties : NULL;
	fixture.info.get_properties2 = flags & 2 ? ep_properties2 : NULL;
	fixture.info.get_defaults = flags & 4 ? ep_defaults : NULL;
	fixture.info.get_defaults2 = flags & 8 ? ep_defaults2 : NULL;
	fixture.info.type_data = &fixture;
	fixture.encoder.orig_info = fixture.info;
	/* Deliberately distinct current info: the instance must use orig_info. */
	fixture.encoder.info.type_data = NULL;
	fixture.encoder.context.data = &fixture.encoder;
	fixture.encoder.context.settings = obs_data_create();
	obs_data_set_int(fixture.encoder.context.settings, "value", value);
	obs_properties_t *props;
	if (instance)
		props = instance_getter(flags & 64 ? NULL : &fixture.encoder);
	else
		props = type_getter(flags & 32 ? "missing" : "ep");
	ep_event(props ? 7 : 8);
	obs_properties_destroy(props);
	obs_data_release(fixture.encoder.context.settings);
	ep_active = NULL;
	return fixture.trace;
}

void oracle_encoder_properties_layout(size_t *layout);
void oracle_encoder_properties_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_properties_fields);
	layout[1] = _Alignof(struct obs_encoder_properties_fields);
	layout[2] = offsetof(struct obs_encoder_properties_fields, properties);
	layout[3] = offsetof(struct obs_encoder_properties_fields, properties2);
	layout[4] = offsetof(struct obs_encoder_properties_fields, defaults);
	layout[5] = offsetof(struct obs_encoder_properties_fields, defaults2);
	layout[6] = offsetof(struct obs_encoder_properties_fields, type_data);
	layout[7] = offsetof(struct obs_encoder_properties_fields, data);
	layout[8] = offsetof(struct obs_encoder_properties_fields, settings);
}
