/* Original getters and exact production adapter over the real private layout.
 * No intentional differences or excluded inputs. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_filter_get_parent oracle_obs_filter_get_parent
#define obs_filter_get_target oracle_obs_filter_get_target
#include "obs-source-filter-relationship-getters.c.h"
#undef obs_filter_get_parent
#undef obs_filter_get_target
#include "source_filter_relationship_fields.inc"

typedef obs_source_t *(*relationship_getter)(const obs_source_t *filter);

void oracle_filter_relationship_layout(size_t *layout);
void oracle_filter_relationship_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_filter_relationship_fields);
	layout[1] = _Alignof(struct obs_source_filter_relationship_fields);
	layout[2] = offsetof(struct obs_source_filter_relationship_fields, parent);
	layout[3] = offsetof(struct obs_source_filter_relationship_fields, target);
	layout[4] = sizeof(obs_source_t *);
	layout[5] = _Alignof(obs_source_t *);
}

struct relationship_fixture {
	struct obs_source sources[3];
	unsigned char snapshot[sizeof(struct obs_source) * 3];
};

void *oracle_filter_relationship_new(unsigned parent, unsigned target, unsigned type);
void *oracle_filter_relationship_new(unsigned parent, unsigned target, unsigned type)
{
	struct relationship_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		obs_source_t *values[] = {NULL, &fixture->sources[0], &fixture->sources[1], &fixture->sources[2]};
		fixture->sources[0].info.type = (enum obs_source_type)type;
		fixture->sources[0].filter_parent = values[parent];
		fixture->sources[0].filter_target = values[target];
		fixture->sources[0].async_width = 17;
		fixture->sources[0].async_height = 29;
		memcpy(fixture->snapshot, fixture->sources, sizeof(fixture->sources));
	}
	return fixture;
}

bool oracle_filter_relationship_unchanged(const void *filter);
bool oracle_filter_relationship_unchanged(const void *filter)
{
	const struct relationship_fixture *fixture = filter;
	return memcmp(fixture->sources, fixture->snapshot, sizeof(fixture->sources)) == 0;
}

void oracle_filter_relationship_free(void *filter);
void oracle_filter_relationship_free(void *filter)
{
	free(filter);
}

struct relationship_log {
	unsigned count;
	int level;
	char message[128];
};

/* Retain printf checking when forwarding the callback format to vsnprintf. */
#if defined(__GNUC__) || defined(__clang__)
static void relationship_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif

static void relationship_capture(int level, const char *format, va_list args, void *param)
{
	struct relationship_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_filter_relationship_null_log(relationship_getter get_parent, relationship_getter get_target);
bool oracle_filter_relationship_null_log(relationship_getter get_parent, relationship_getter get_target)
{
	struct relationship_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(relationship_capture, &log);
	obs_source_t *parent = get_parent(NULL);
	bool parent_ok = !parent && log.count == 1 && log.level == LOG_DEBUG &&
			 strcmp(log.message, "obs_filter_get_parent: Null 'filter' parameter") == 0;
	obs_source_t *target = get_target(NULL);
	base_set_log_handler(old_handler, old_param);
	return parent_ok && !target && log.count == 2 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_filter_get_target: Null 'filter' parameter") == 0;
}
