/* Verbatim queries and production field adapter over a thread-local real core. */
#include "libobs/build/libobs.pch.h"
#include <stdlib.h>

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *output_protocol_registry_core;
#else
static _Thread_local struct obs_core *output_protocol_registry_core;
#endif

static const struct obs_core *output_protocol_registry_current_core(void)
{
	return output_protocol_registry_core;
}

bool oracle_obs_is_output_protocol_registered(const char *protocol);
bool oracle_obs_enum_output_protocols(size_t idx, char **protocol);
#define obs output_protocol_registry_core
#define obs_is_output_protocol_registered oracle_obs_is_output_protocol_registered
#define obs_enum_output_protocols oracle_obs_enum_output_protocols
#include "libobs/obs-core-output-protocol-registry.c.h"
#undef obs_enum_output_protocols
#undef obs_is_output_protocol_registered
#undef obs
#define obs_core_texture_activity_core output_protocol_registry_current_core
#include "core_output_protocol_registry_fields.inc"
#undef obs_core_texture_activity_core

typedef bool (*output_protocol_registered_query)(const char *);
typedef bool (*output_protocol_enum_query)(size_t, char **);
bool oracle_output_protocol_registry_check(char **names, size_t count, const char *needle, size_t index,
					   output_protocol_registered_query registered,
					   output_protocol_enum_query enumerate);
bool oracle_output_protocol_registry_check(char **names, size_t count, const char *needle, size_t index,
					   output_protocol_registered_query registered,
					   output_protocol_enum_query enumerate)
{
	struct obs_core core = {0};
	struct obs_core before;
	char *pointers_before[16];
	if (count > 16)
		abort();
	if (count)
		memcpy(pointers_before, names, count * sizeof(char *));
	core.data.protocols.array = count ? names : NULL;
	core.data.protocols.num = count;
	core.data.protocols.capacity = count;
	memcpy(&before, &core, sizeof(core));
	output_protocol_registry_core = &core;
	size_t observed_count = SIZE_MAX;
	char *const *array = obs_core_output_protocol_registry_fields(&observed_count);
	bool valid = observed_count == count && array == core.data.protocols.array;
	char sentinel[] = "unchanged";
	char *actual = sentinel, *expected = sentinel;
	bool actual_result = enumerate(index, &actual);
	bool expected_result = oracle_obs_enum_output_protocols(index, &expected);
	valid = valid && actual_result == expected_result && actual == expected;
	valid = valid && registered(needle) == oracle_obs_is_output_protocol_registered(needle);
	valid = valid && !enumerate(index, NULL) && !oracle_obs_enum_output_protocols(index, NULL);
	valid = valid && memcmp(&core, &before, sizeof(core)) == 0;
	valid = valid && (!count || memcmp(pointers_before, names, count * sizeof(char *)) == 0);
	output_protocol_registry_core = NULL;
	/* The original accepts these NULL arguments even without an initialized core. */
	valid = valid && !registered(NULL) && !oracle_obs_is_output_protocol_registered(NULL);
	valid = valid && !enumerate(index, NULL) && !oracle_obs_enum_output_protocols(index, NULL);
	return valid;
}

void oracle_output_protocol_registry_layout(size_t *layout);
void oracle_output_protocol_registry_layout(size_t *layout)
{
	layout[0] = sizeof(((struct obs_core *)0)->data.protocols.num);
	layout[1] = _Alignof(size_t);
	layout[2] = sizeof(*((struct obs_core *)0)->data.protocols.array);
	layout[3] = _Alignof(char *);
}
