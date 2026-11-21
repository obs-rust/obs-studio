/* Original obs-service-info-accessors.c.h, renamed to oracle_*, and the real
 * field adapter of obs-service-info-accessors-fields.c.h over the real
 * struct obs_service (#3840). Under cargo test the adapter is the only
 * definition of its real name, so the Rust shims (and the service lifecycle
 * parity in oracle/service_getters.c) reach real service layouts through it.
 * The fixture builds services and tables from byte patterns, so the parity
 * test compares the C and the Rust on the whole service, byte for byte. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#define obs_service_set_info oracle_obs_service_set_info
#define obs_service_set_plugin_data oracle_obs_service_set_plugin_data
#define obs_service_get_info_table oracle_obs_service_get_info_table
#define obs_service_get_plugin_data oracle_obs_service_get_plugin_data
#include "obs-service-info-accessors.c.h"
#undef obs_service_get_plugin_data
#undef obs_service_get_info_table
#undef obs_service_set_plugin_data
#undef obs_service_set_info
#include "service_info_accessors_fields.inc"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

static void service_info_fill(void *dst, unsigned char seed, size_t size)
{
	unsigned char *bytes = dst;
	for (size_t i = 0; i < size; i++)
		bytes[i] = (unsigned char)(seed + i * 7u);
}

/* A service whose every byte is a pattern of the seed. */
obs_service_t *oracle_service_info_service_new(unsigned char seed);
obs_service_t *oracle_service_info_service_new(unsigned char seed)
{
	struct obs_service *service = malloc(sizeof(*service));
	if (service)
		service_info_fill(service, seed, sizeof(*service));
	return service;
}

void oracle_service_info_service_free(obs_service_t *service);
void oracle_service_info_service_free(obs_service_t *service)
{
	free(service);
}

/* A table whose every byte is a pattern of the seed: callbacks that are
 * never called. The table holds only pointers, so it has no padding. */
void oracle_service_info_table_fill(struct obs_service_info *info, unsigned char seed);
void oracle_service_info_table_fill(struct obs_service_info *info, unsigned char seed)
{
	service_info_fill(info, seed, sizeof(*info));
}

bool oracle_service_info_service_equal(const obs_service_t *a, const obs_service_t *b);
bool oracle_service_info_service_equal(const obs_service_t *a, const obs_service_t *b)
{
	return memcmp(a, b, sizeof(struct obs_service)) == 0;
}

bool oracle_service_info_table_equal(const struct obs_service_info *a, const struct obs_service_info *b);
bool oracle_service_info_table_equal(const struct obs_service_info *a, const struct obs_service_info *b)
{
	return memcmp(a, b, sizeof(struct obs_service_info)) == 0;
}

/* The adapter hands back exactly the two member addresses. */
bool oracle_service_info_addresses(obs_service_t *service);
bool oracle_service_info_addresses(obs_service_t *service)
{
	struct obs_service_info_accessors_fields fields = obs_service_info_accessors_fields(service);
	return fields.info == &service->info && fields.data == &service->context.data;
}

/* The adapter struct, the table and where the two members sit. */
void oracle_service_info_layout(size_t *layout);
void oracle_service_info_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_service_info_accessors_fields);
	layout[1] = _Alignof(struct obs_service_info_accessors_fields);
	layout[2] = offsetof(struct obs_service_info_accessors_fields, info);
	layout[3] = offsetof(struct obs_service_info_accessors_fields, data);
	layout[4] = sizeof(struct obs_service_info);
	layout[5] = _Alignof(struct obs_service_info);
	layout[6] = offsetof(struct obs_service, info);
	layout[7] = offsetof(struct obs_service, context.data);
	layout[8] = sizeof(struct obs_service);
}
