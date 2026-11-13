/* Original backup loop and real adapter; only obs_save_source is a fixture seam.
 * Tier 2 exercises the unchanged characterization with the real serializer. */
#include <errno.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define obs_source_backup_filters oracle_obs_source_backup_filters
#include "obs-source-filter-backup.c.h"
#undef obs_source_backup_filters
#include "source_filter_backup_fields.inc"

struct backup_fixture;
struct backup_entry {
	struct obs_source source;
	struct backup_fixture *owner;
	unsigned char value;
};

struct backup_fixture {
	struct obs_source parent;
	struct backup_entry entries[16];
	obs_source_t *items[16];
	size_t saves;
	bool locked_saves;
	bool grow;
};

obs_data_t *obs_save_source(obs_source_t *source)
{
	struct backup_entry *entry = source->context.data;
	struct backup_fixture *fixture = entry->owner;
	int result = pthread_mutex_trylock(&fixture->parent.filter_mutex);
	fixture->locked_saves = fixture->locked_saves && result == EBUSY;
	if (result == 0)
		pthread_mutex_unlock(&fixture->parent.filter_mutex);
	fixture->saves++;
	char name[32], uuid[32];
	snprintf(name, sizeof(name), "filter-%u", (unsigned)entry->value);
	snprintf(uuid, sizeof(uuid), "uuid-%u", (unsigned)entry->value);
	obs_data_t *saved = obs_data_create();
	obs_data_set_string(saved, "name", name);
	obs_data_set_string(saved, "id", "backup_filter");
	obs_data_set_string(saved, "versioned_id", "backup_filter");
	obs_data_set_string(saved, "uuid", uuid);
	obs_data_set_bool(saved, "enabled", (entry->value & 1) != 0);
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "marker", entry->value);
	obs_data_set_int(settings, "save_ordinal", (long long)fixture->saves);
	obs_data_set_obj(saved, "settings", settings);
	obs_data_release(settings);
	if (fixture->grow && fixture->saves == 1 && fixture->parent.filters.num < 16)
		fixture->parent.filters.num++;
	return saved;
}

void *oracle_filter_backup_new(const unsigned char *values, size_t count, bool grow);
void *oracle_filter_backup_new(const unsigned char *values, size_t count, bool grow)
{
	if (count > 16)
		return NULL;
	struct backup_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	if (pthread_mutex_init(&fixture->parent.filter_mutex, NULL) != 0) {
		free(fixture);
		return NULL;
	}
	fixture->locked_saves = true;
	fixture->grow = grow;
	for (size_t i = 0; i < 16; i++) {
		fixture->entries[i].owner = fixture;
		fixture->entries[i].value = i < count ? values[i] : 238;
		fixture->entries[i].source.context.data = &fixture->entries[i];
		fixture->items[i] = &fixture->entries[i].source;
	}
	fixture->parent.filters.array = fixture->items;
	fixture->parent.filters.num = count;
	fixture->parent.filters.capacity = 16;
	return fixture;
}

bool oracle_filter_backup_healthy(void *handle, size_t expected_saves);
bool oracle_filter_backup_healthy(void *handle, size_t expected_saves)
{
	struct backup_fixture *fixture = handle;
	int result = pthread_mutex_trylock(&fixture->parent.filter_mutex);
	if (result != 0)
		return false;
	pthread_mutex_unlock(&fixture->parent.filter_mutex);
	bool ok = fixture->locked_saves && fixture->saves == expected_saves &&
		  fixture->parent.filters.array == fixture->items && fixture->parent.filters.capacity == 16 &&
		  fixture->parent.filters.num == expected_saves;
	for (size_t i = 0; i < 16; i++)
		ok = ok && fixture->items[i] == &fixture->entries[i].source;
	return ok;
}

bool oracle_filter_backup_free(void *handle);
bool oracle_filter_backup_free(void *handle)
{
	struct backup_fixture *fixture = handle;
	int result = pthread_mutex_destroy(&fixture->parent.filter_mutex);
	free(fixture);
	return result == 0;
}

struct backup_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void backup_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void backup_capture(int level, const char *format, va_list args, void *param)
{
	struct backup_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef obs_data_array_t *(*backup_fn)(obs_source_t *);
typedef void (*backup_get_log)(log_handler_t *, void **);
typedef void (*backup_set_log)(log_handler_t, void *);
bool oracle_filter_backup_logs(backup_fn backup, backup_get_log get, backup_set_log set);
bool oracle_filter_backup_logs(backup_fn backup, backup_get_log get, backup_set_log set)
{
	log_handler_t old;
	void *param;
	get(&old, &param);
	struct backup_log r = {0}, c = {0};
	set(backup_capture, &r);
	obs_data_array_t *rust_value = backup(NULL);
	set(backup_capture, &c);
	obs_data_array_t *c_value = oracle_obs_source_backup_filters(NULL);
	set(old, param);
	return !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG && strcmp(r.message, "obs_source_backup_filters: Null 'source' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
