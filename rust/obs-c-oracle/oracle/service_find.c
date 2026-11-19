/* Original lookup and real field projections, with a controlled C-owned core. */
#include "obs-internal.h"
#include <string.h>

static struct obs_core service_find_oracle_storage;
static struct obs_core *service_find_oracle_core = &service_find_oracle_storage;
#define obs service_find_oracle_core
#define find_service oracle_find_service
const struct obs_service_info *oracle_find_service(const char *id);
#include "service-find.inc"
#undef find_service
#include "service_find_fields.inc"
#undef obs

void oracle_service_set_types(const struct obs_service_info *types, size_t num);
void oracle_service_set_types(const struct obs_service_info *types, size_t num)
{
	/* The lookup only borrows this table; it never writes through the C field. */
	memcpy(&service_find_oracle_storage.service_types.array, &types, sizeof(types));
	service_find_oracle_storage.service_types.num = num;
	service_find_oracle_storage.service_types.capacity = num;
}

static unsigned service_find_calls;
static const char *service_find_name(void *data)
{
	(void)data;
	service_find_calls++;
	return "unused";
}
static void *service_find_create(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	(void)service;
	service_find_calls++;
	return NULL;
}
static void service_find_destroy(void *data)
{
	(void)data;
	service_find_calls++;
}

typedef const struct obs_service_info *(*service_find_fn)(const char *);
bool oracle_service_find_check(const char *const *ids, size_t num, size_t capacity, const char *query,
			       service_find_fn rust_find);
bool oracle_service_find_check(const char *const *ids, size_t num, size_t capacity, const char *query,
			       service_find_fn rust_find)
{
	if (num > capacity || capacity > 8)
		return false;
	struct obs_service_info entries[8] = {0};
	struct obs_service_info before[8];
	int owners[8] = {0};
	unsigned char saved[sizeof(service_find_oracle_storage)];
	memcpy(saved, &service_find_oracle_storage, sizeof(saved));
	for (size_t i = 0; i < capacity; i++) {
		entries[i].id = ids[i];
		entries[i].get_name = service_find_name;
		entries[i].create = service_find_create;
		entries[i].destroy = service_find_destroy;
		entries[i].free_type_data = service_find_destroy;
		entries[i].type_data = &owners[i];
	}
	oracle_service_set_types(entries, num);
	service_find_oracle_storage.service_types.capacity = capacity;
	bool ok = true;
	/* Repeat after replacing and moving entries: no stale cached result. */
	for (unsigned step = 0; step < 3; step++) {
		memcpy(before, entries, sizeof(entries));
		unsigned char core_before[sizeof(service_find_oracle_storage)];
		memcpy(core_before, &service_find_oracle_storage, sizeof(core_before));
		service_find_calls = 0;
		const struct obs_service_info *expected = oracle_find_service(query);
		const struct obs_service_info *actual = rust_find(query);
		ok = ok && actual == expected && service_find_calls == 0 &&
		     memcmp(entries, before, sizeof(entries)) == 0 &&
		     memcmp(core_before, &service_find_oracle_storage, sizeof(core_before)) == 0 &&
		     memcmp(owners, (int[8]){0}, sizeof(owners)) == 0;
		if (actual && expected)
			ok = ok && actual->id == expected->id && actual->type_data == expected->type_data;
		if (num) {
			if (step == 0) {
				struct obs_service_info swap = entries[0];
				entries[0] = entries[num - 1];
				entries[num - 1] = swap;
			} else {
				entries[0].id = "replaced";
			}
		}
	}
	memcpy(&service_find_oracle_storage, saved, sizeof(saved));
	return ok;
}
