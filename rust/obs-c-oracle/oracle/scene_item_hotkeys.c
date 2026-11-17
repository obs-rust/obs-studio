/* Unchanged C bodies and real field adapters, with observable operations recorded.
 * LONG_MAX is excluded: owners + 1 in the original C has signed overflow there. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"

#ifdef _MSC_VER
#define SCENE_HOTKEY_TLS __declspec(thread)
#else
#define SCENE_HOTKEY_TLS _Thread_local
#endif
static SCENE_HOTKEY_TLS struct obs_core *scene_hotkey_core;
static SCENE_HOTKEY_TLS uint64_t scene_hotkey_trace;
static SCENE_HOTKEY_TLS const char *scene_hotkey_show_desc;
static SCENE_HOTKEY_TLS const char *scene_hotkey_hide_desc;
static SCENE_HOTKEY_TLS size_t scene_hotkey_pair;
static SCENE_HOTKEY_TLS unsigned scene_hotkey_rename_result;
static SCENE_HOTKEY_TLS bool scene_hotkey_race_pending;
static SCENE_HOTKEY_TLS long scene_hotkey_race_owners;

/* Model one competing owner between load and CAS, over a still-live fixture. */
static bool scene_hotkey_compare(volatile long *value, long *expected, long next)
{
	if (scene_hotkey_race_pending) {
		os_atomic_store_long(value, scene_hotkey_race_owners);
		scene_hotkey_race_pending = false;
	}
	return os_atomic_compare_exchange_long(value, expected, next);
}

static bool scene_hotkey_set_visible(obs_sceneitem_t *item, bool visible)
{
	scene_hotkey_trace = scene_hotkey_trace * 10 + (visible ? 1 : 2);
	os_atomic_store_bool(&item->user_visible, visible);
	return true;
}

static void scene_hotkey_release(obs_sceneitem_t *item)
{
	scene_hotkey_trace = scene_hotkey_trace * 10 + (item ? 3 : 4);
	if (item)
		os_atomic_dec_long(&item->ref);
}

/* The rename handler's reads in the order they happen: n the name, s and h
 * the show and hide templates, p the pair (Rust side only: the C reads the
 * field as the setter's argument), d the setter. */
static SCENE_HOTKEY_TLS char scene_hotkey_reads[16];
static SCENE_HOTKEY_TLS size_t scene_hotkey_read_count;
static SCENE_HOTKEY_TLS unsigned scene_hotkey_copies;

static void scene_hotkey_read(char read)
{
	if (scene_hotkey_read_count + 1 < sizeof(scene_hotkey_reads)) {
		scene_hotkey_reads[scene_hotkey_read_count++] = read;
		scene_hotkey_reads[scene_hotkey_read_count] = 0;
	}
}

static void scene_hotkey_reads_reset(void)
{
	scene_hotkey_read_count = 0;
	scene_hotkey_reads[0] = 0;
	scene_hotkey_copies = 0;
}

static const char *scene_hotkey_calldata_string(const calldata_t *data, const char *name)
{
	scene_hotkey_read('n');
	return calldata_string(data, name);
}

static void scene_hotkey_descriptions(obs_hotkey_pair_id pair, const char *show, const char *hide)
{
	scene_hotkey_read('d');
	scene_hotkey_rename_result = (pair != scene_hotkey_pair);
	if (!!show != !!scene_hotkey_show_desc || (show && strcmp(show, scene_hotkey_show_desc)))
		scene_hotkey_rename_result |= 2;
	if (!!hide != !!scene_hotkey_hide_desc || (hide && strcmp(hide, scene_hotkey_hide_desc)))
		scene_hotkey_rename_result |= 4;
}

void oracle_dstr_copy(struct dstr *str, const char *text);
void oracle_dstr_replace(struct dstr *str, const char *find, const char *replacement);

/* The C copies the show template, then the hide template. */
static void scene_hotkey_dstr_copy(struct dstr *str, const char *text)
{
	scene_hotkey_read(scene_hotkey_copies++ == 0 ? 's' : 'h');
	oracle_dstr_copy(str, text);
}

#define obs scene_hotkey_core
#define obs_sceneitem_set_visible scene_hotkey_set_visible
#define obs_sceneitem_release scene_hotkey_release
#define obs_hotkey_pair_set_descriptions scene_hotkey_descriptions
#define calldata_string scene_hotkey_calldata_string
#define dstr_copy scene_hotkey_dstr_copy
#define dstr_replace oracle_dstr_replace
#define os_atomic_compare_exchange_long scene_hotkey_compare
#include "obs-scene-item-hotkeys.c.h"
#undef dstr_replace
#undef dstr_copy
#include "scene_item_hotkeys_fields.inc"
#undef calldata_string
#undef os_atomic_compare_exchange_long
#undef obs_hotkey_pair_set_descriptions
#undef obs_sceneitem_release
#undef obs_sceneitem_set_visible
#undef obs

typedef bool (*scene_hotkey_callback)(obs_sceneitem_t *, bool, bool, long (*)(obs_sceneitem_t *),
				      bool (*)(obs_sceneitem_t *, long *, long), bool (*)(const obs_sceneitem_t *),
				      void (*)(obs_sceneitem_t *, bool), void (*)(obs_sceneitem_t *));
bool obs_sceneitem_user_visible_load(const obs_sceneitem_t *item);
typedef void (*scene_rename_callback)(obs_sceneitem_t *, calldata_t *, const char *(*)(calldata_t *),
				      const char *(*)(bool), size_t (*)(const obs_sceneitem_t *),
				      void (*)(size_t, const char *, const char *));

/* The adapter's template and pair getters, with their reads recorded. */
static const char *scene_hotkey_template_read(bool show)
{
	scene_hotkey_read(show ? 's' : 'h');
	return obs_sceneitem_hotkey_template(show);
}

static size_t scene_hotkey_pair_read(const obs_sceneitem_t *item)
{
	scene_hotkey_read('p');
	return obs_sceneitem_hotkey_pair(item);
}
unsigned oracle_scene_hotkey_probe(long owners, bool visible, bool show, bool pressed, bool race, long observed,
				   scene_hotkey_callback callback);
unsigned oracle_scene_hotkey_rename_probe(const char *show, const char *hide, const char *name, size_t pair,
					  scene_rename_callback callback);

unsigned oracle_scene_hotkey_probe(long owners, bool visible, bool show, bool pressed, bool race, long observed,
				   scene_hotkey_callback callback)
{
	struct obs_scene_item original = {0}, actual = {0};
	os_atomic_store_long(&original.ref, owners);
	os_atomic_store_bool(&original.user_visible, visible);
	memcpy(&actual, &original, sizeof(actual));
	scene_hotkey_trace = 0;
	scene_hotkey_race_pending = race;
	scene_hotkey_race_owners = observed;
	bool expected = (show ? hotkey_show_sceneitem : hotkey_hide_sceneitem)(&original, 123, NULL, pressed);
	uint64_t trace = scene_hotkey_trace;
	scene_hotkey_trace = 0;
	scene_hotkey_race_pending = race;
	bool result = callback(&actual, show, pressed, obs_sceneitem_hotkey_owners, obs_sceneitem_hotkey_compare,
			       obs_sceneitem_user_visible_load, obs_sceneitem_hotkey_set_visible,
			       obs_sceneitem_hotkey_release);
	return (result != expected ? 1u : 0u) | (scene_hotkey_trace != trace ? 2u : 0u) |
	       (memcmp(&actual, &original, sizeof(actual)) != 0 ? 4u : 0u);
}

unsigned oracle_scene_hotkey_rename_probe(const char *show, const char *hide, const char *name, size_t pair,
					  scene_rename_callback callback)
{
	struct obs_core core = {0};
	struct obs_scene_item item = {0};
	core.hotkeys.sceneitem_show = show ? bstrdup(show) : NULL;
	core.hotkeys.sceneitem_hide = hide ? bstrdup(hide) : NULL;
	item.toggle_visibility = pair;
	scene_hotkey_core = &core;
	scene_hotkey_pair = pair;
	/* Keep independent C descriptions alive while checking both callbacks. */
	struct dstr show_desc = {0}, hide_desc = {0};
	oracle_dstr_copy(&show_desc, show);
	oracle_dstr_replace(&show_desc, "%1", name);
	oracle_dstr_copy(&hide_desc, hide);
	oracle_dstr_replace(&hide_desc, "%1", name);
	scene_hotkey_show_desc = show_desc.array;
	scene_hotkey_hide_desc = hide_desc.array;
	calldata_t data;
	calldata_init(&data);
	if (name)
		calldata_set_string(&data, "new_name", name);
	scene_hotkey_reads_reset();
	sceneitem_renamed(&item, &data);
	unsigned original_result = scene_hotkey_rename_result;
	/* The C's own order, its pair read being the setter's argument. */
	if (strcmp(scene_hotkey_reads, "nshd") != 0)
		original_result |= 32;
	scene_hotkey_rename_result = 8;
	scene_hotkey_reads_reset();
	callback(&item, &data, obs_sceneitem_hotkey_new_name, scene_hotkey_template_read, scene_hotkey_pair_read,
		 obs_sceneitem_hotkey_descriptions);
	unsigned result = scene_hotkey_rename_result | original_result;
	/* The same order, the pair read right before the setter. */
	result |= strcmp(scene_hotkey_reads, "nshpd") != 0 ? 64u : 0u;
	result |= item.toggle_visibility != pair ? 16u : 0u;
	calldata_free(&data);
	dstr_free(&show_desc);
	dstr_free(&hide_desc);
	bfree(core.hotkeys.sceneitem_show);
	bfree(core.hotkeys.sceneitem_hide);
	scene_hotkey_core = NULL;
	return result;
}
#undef SCENE_HOTKEY_TLS
