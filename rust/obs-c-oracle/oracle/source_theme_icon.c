/* Original whole-file C theme dispatch; both sides look IDs up with the ported
 * get_source_info over a thread-local registry. The active fixture is local to
 * this thread. */
#include "libobs/build/libobs-ported-service-getters.pch.h"
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef const char *(*theme_getter)(const char *);
typedef const char *(*theme_callback)(void *);
void *obs_source_info_icon_data(const struct obs_source_info *info);
theme_callback obs_source_info_dark_icon(const struct obs_source_info *info);
theme_callback obs_source_info_light_icon(const struct obs_source_info *info);
struct theme_fixture {
	const char *const *ids;
	const uint32_t *icons;
	size_t count;
	unsigned flags;
	void *data;
	const char *dark_result, *light_result;
	size_t lookups, dark_calls, light_calls, data_matches;
};
#ifdef _MSC_VER
static __declspec(thread) struct theme_fixture *theme_active;
#else
static _Thread_local struct theme_fixture *theme_active;
#endif

static const char *theme_dark(void *data)
{
	++theme_active->dark_calls;
	theme_active->data_matches += data == theme_active->data;
	return theme_active->dark_result;
}
static const char *theme_light(void *data)
{
	++theme_active->light_calls;
	theme_active->data_matches += data == theme_active->data;
	return theme_active->light_result;
}

/* Both getters look IDs up with the ported get_source_info over the fixture's
 * registry (source_info_lookup.c). Each lookup is counted as it starts; with
 * flag 4 the second lookup finds an empty registry, as if the type had been
 * unregistered between the icon type check and the callback lookup. */
#define THEME_FIXTURE_MAX 32
typedef void (*theme_lookup_observer)(void *ctx);
void oracle_source_info_lookup_use(struct obs_source_info *types, size_t num);
struct obs_core *oracle_source_registry_exchange_core(struct obs_core *core);
void oracle_source_info_lookup_observe(theme_lookup_observer observer, void *ctx);

static void theme_lookup_started(void *ctx)
{
	struct theme_fixture *f = ctx;
	++f->lookups;
	if ((f->flags & 4) && f->lookups == 2)
		oracle_source_info_lookup_use(NULL, 0);
}

enum obs_icon_type oracle_obs_source_get_icon_type(const char *id);
const char *oracle_obs_source_get_dark_icon(const char *id);
const char *oracle_obs_source_get_light_icon(const char *id);
#define obs_source_get_icon_type oracle_obs_source_get_icon_type
#define obs_source_get_dark_icon oracle_obs_source_get_dark_icon
#define obs_source_get_light_icon oracle_obs_source_get_light_icon
#include "source_theme_icon.inc"
#undef obs_source_get_light_icon
#undef obs_source_get_dark_icon
#undef obs_source_get_icon_type

const char *oracle_source_theme_icon_run(theme_getter getter, const char *query, const char *const *ids,
					 const uint32_t *icons, size_t count, unsigned flags, void *data,
					 const char *dark, const char *light, size_t *observed);
const char *oracle_source_theme_icon_run(theme_getter getter, const char *query, const char *const *ids,
					 const uint32_t *icons, size_t count, unsigned flags, void *data,
					 const char *dark, const char *light, size_t *observed)
{
	struct theme_fixture fixture = {.ids = ids,
					.icons = icons,
					.count = count,
					.flags = flags,
					.data = data,
					.dark_result = dark,
					.light_result = light};
	struct obs_source_info types[THEME_FIXTURE_MAX] = {0};
	if (count > THEME_FIXTURE_MAX)
		abort();
	for (size_t i = 0; i < count; ++i) {
		types[i] = (struct obs_source_info){.id = ids[i],
						    .icon_type = (enum obs_icon_type)icons[i],
						    .type_data = data,
						    .get_dark_icon = (flags & 1) ? theme_dark : NULL,
						    .get_light_icon = (flags & 2) ? theme_light : NULL};
	}
	struct theme_fixture *previous = theme_active;
	theme_active = &fixture;
	struct obs_core *previous_core = oracle_source_registry_exchange_core(NULL);
	oracle_source_info_lookup_use(types, count);
	oracle_source_info_lookup_observe(theme_lookup_started, &fixture);
	const char *result = getter(query);
	oracle_source_info_lookup_observe(NULL, NULL);
	oracle_source_registry_exchange_core(previous_core);
	theme_active = previous;
	observed[0] = fixture.lookups;
	observed[1] = fixture.dark_calls;
	observed[2] = fixture.light_calls;
	observed[3] = fixture.data_matches;
	return result;
}

bool oracle_source_theme_icon_layout(size_t *layout);
bool oracle_source_theme_icon_layout(size_t *layout)
{
	struct obs_source_info info = {.type_data = &info, .get_dark_icon = theme_dark, .get_light_icon = theme_light};
	layout[0] = sizeof(info.type_data);
	layout[1] = _Alignof(void *);
	layout[2] = sizeof(info.get_dark_icon);
	layout[3] = _Alignof(theme_callback);
	return obs_source_info_icon_data(&info) == &info && obs_source_info_dark_icon(&info) == theme_dark &&
	       obs_source_info_light_icon(&info) == theme_light;
}
