/* Original extracted C over real-header scene items. Items are filled with an
 * arbitrary byte pattern and never enter the destroy path; neighbors are only
 * compared, never followed. No intentional differences. */
#define obs_sceneitem_addref oracle_obs_sceneitem_addref
#include "libobs/build/libobs.pch.h"
#include "sceneitem-addref.inc"
#undef obs_sceneitem_addref
#include <stdlib.h>
#include <string.h>

typedef void (*addref_operation)(obs_sceneitem_t *);
size_t oracle_sceneitem_addref_layout(unsigned field);
unsigned oracle_sceneitem_addref_probe(long initial, unsigned char fill, unsigned times, addref_operation addref);
unsigned oracle_sceneitem_addref_null(addref_operation addref);
obs_sceneitem_t *oracle_sceneitem_addref_new(long initial);
long oracle_sceneitem_addref_count(const obs_sceneitem_t *item);
void oracle_sceneitem_addref_free(obs_sceneitem_t *item);
void oracle_sceneitem_addref_original(obs_sceneitem_t *item);

/* The counter's width and place in the real private header. */
size_t oracle_sceneitem_addref_layout(unsigned field)
{
	switch (field) {
	case 0:
		return sizeof(((struct obs_scene_item *)NULL)->ref);
	case 1:
		return offsetof(struct obs_scene_item, ref);
	case 2:
		return sizeof(struct obs_scene_item);
	default:
		return (size_t)-1;
	}
}

/* Run the original on one item and the candidate on a byte copy, `times`
 * times each, both pointing at one shared neighbor. Bit 0: the two items'
 * bytes differ after a call; bit 1: the original's count is not the initial
 * plus the number of calls (wrapping as the native atomic does); bit 2: the
 * neighbor changed; bit 3: allocation failed. */
unsigned oracle_sceneitem_addref_probe(long initial, unsigned char fill, unsigned times, addref_operation addref)
{
	struct obs_scene_item *items = malloc(3 * sizeof(*items));
	struct obs_scene_item *neighbor_before = malloc(sizeof(*neighbor_before));
	unsigned result = 0;
	if (!items || !neighbor_before) {
		result = 8;
		goto done;
	}
	struct obs_scene_item *original = &items[0], *candidate = &items[1], *neighbor = &items[2];
	memset(items, fill, 3 * sizeof(*items));
	original->ref = initial;
	original->removed = true;
	original->id = INT64_C(123456789);
	original->active_refs = 17;
	original->defer_update = 23;
	original->defer_group_resize = 29;
	original->visible = true;
	original->selected = true;
	original->locked = true;
	original->crop.left = -11;
	original->prev = neighbor;
	original->next = neighbor;
	neighbor->ref = 71;
	neighbor->next = original;
	memcpy(candidate, original, sizeof(*original));
	memcpy(neighbor_before, neighbor, sizeof(*neighbor));
	for (unsigned call = 1; call <= times; call++) {
		oracle_obs_sceneitem_addref(original);
		addref(candidate);
		if (memcmp(original, candidate, sizeof(*original)))
			result |= 1;
		if (original->ref != (long)((unsigned long)initial + call))
			result |= 2;
		if (memcmp(neighbor, neighbor_before, sizeof(*neighbor)))
			result |= 4;
	}
done:
	free(neighbor_before);
	free(items);
	return result;
}

struct addref_log {
	unsigned count;
};
#if defined(__GNUC__) || defined(__clang__)
static void addref_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void addref_capture(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	((struct addref_log *)param)->count++;
}

/* NULL is silent in both: bit 0 if the original logs, bit 1 if the candidate
 * does. Reaching the return also shows neither dereferenced NULL. */
unsigned oracle_sceneitem_addref_null(addref_operation addref)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	struct addref_log original = {0}, candidate = {0};
	base_set_log_handler(addref_capture, &original);
	oracle_obs_sceneitem_addref(NULL);
	base_set_log_handler(addref_capture, &candidate);
	addref(NULL);
	base_set_log_handler(previous, param);
	return (original.count ? 1u : 0u) | (candidate.count ? 2u : 0u);
}

/* A zeroed real-header item shared by concurrent callers. */
obs_sceneitem_t *oracle_sceneitem_addref_new(long initial)
{
	struct obs_scene_item *item = calloc(1, sizeof(*item));
	if (item)
		item->ref = initial;
	return item;
}

long oracle_sceneitem_addref_count(const obs_sceneitem_t *item)
{
	return item->ref;
}

void oracle_sceneitem_addref_free(obs_sceneitem_t *item)
{
	free(item);
}

void oracle_sceneitem_addref_original(obs_sceneitem_t *item)
{
	oracle_obs_sceneitem_addref(item);
}
