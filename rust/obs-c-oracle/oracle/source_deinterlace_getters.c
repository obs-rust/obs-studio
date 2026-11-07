/* Original getters and adapter over the real private source layout.
 * No intended behavior differences or excluded mode bits. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_deinterlace_mode oracle_obs_source_get_deinterlace_mode
#define obs_source_get_deinterlace_field_order oracle_obs_source_get_deinterlace_field_order
#include "obs-source-deinterlace-getters.c.h"
#undef obs_source_get_deinterlace_mode
#undef obs_source_get_deinterlace_field_order
#include "source_deinterlace_fields.inc"

extern enum obs_deinterlace_mode obs_source_get_deinterlace_mode(const obs_source_t *source);
extern enum obs_deinterlace_field_order obs_source_get_deinterlace_field_order(const obs_source_t *source);

void oracle_source_deinterlace_layout(size_t *layout);
void oracle_source_deinterlace_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_deinterlace_fields);
	layout[1] = _Alignof(struct obs_source_deinterlace_fields);
	layout[2] = offsetof(struct obs_source_deinterlace_fields, mode);
	layout[3] = offsetof(struct obs_source_deinterlace_fields, top_first);
	layout[4] = sizeof(enum obs_deinterlace_mode);
	layout[5] = _Alignof(enum obs_deinterlace_mode);
	layout[6] = sizeof(enum obs_deinterlace_field_order);
	layout[7] = _Alignof(enum obs_deinterlace_field_order);
}

struct deinterlace_fixture {
	struct obs_source source;
	unsigned char snapshot[sizeof(struct obs_source)];
};

void *oracle_source_deinterlace_new(uint32_t mode, bool top);
void *oracle_source_deinterlace_new(uint32_t mode, bool top)
{
	struct deinterlace_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.deinterlace_mode = (enum obs_deinterlace_mode)mode;
		fixture->source.deinterlace_top_first = top;
		fixture->source.async_width = 17;
		fixture->source.async_height = 29;
		memcpy(fixture->snapshot, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

bool oracle_source_deinterlace_unchanged(const void *source);
bool oracle_source_deinterlace_unchanged(const void *source)
{
	const struct deinterlace_fixture *fixture = source;
	return memcmp(&fixture->source, fixture->snapshot, sizeof(fixture->source)) == 0;
}

void oracle_source_deinterlace_free(void *source);
void oracle_source_deinterlace_free(void *source)
{
	free(source);
}

struct deinterlace_log {
	unsigned count;
	int level;
	char message[128];
};

/* Like the other oracle printf forwarders, retain format checking at the call site. */
#if defined(__GNUC__) || defined(__clang__)
static void deinterlace_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif

static void deinterlace_capture(int level, const char *format, va_list args, void *param)
{
	struct deinterlace_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_source_deinterlace_null_log(bool rust);
bool oracle_source_deinterlace_null_log(bool rust)
{
	struct deinterlace_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(deinterlace_capture, &log);
	enum obs_deinterlace_mode mode = rust ? obs_source_get_deinterlace_mode(NULL)
					      : oracle_obs_source_get_deinterlace_mode(NULL);
	bool mode_ok = mode == OBS_DEINTERLACE_MODE_DISABLE && log.count == 1 && log.level == LOG_DEBUG &&
		       strcmp(log.message, "obs_source_set_deinterlace_mode: Null 'source' parameter") == 0;
	enum obs_deinterlace_field_order order = rust ? obs_source_get_deinterlace_field_order(NULL)
						      : oracle_obs_source_get_deinterlace_field_order(NULL);
	base_set_log_handler(old_handler, old_param);
	return mode_ok && order == OBS_DEINTERLACE_FIELD_ORDER_TOP && log.count == 2 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_source_set_deinterlace_field_order: Null 'source' parameter") == 0;
}
