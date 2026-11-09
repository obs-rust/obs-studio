/* Original setter pair, with identical reference recording for both sides. */
#include "libobs/build/libobs.pch.h"

struct scene_setter_event {
	unsigned kind;
	unsigned source;
	unsigned shown;
	unsigned hidden;
};
struct scene_setter_state {
	struct obs_scene_item item;
	struct obs_source sources[3];
	int refs[3];
	struct scene_setter_event events[32];
	size_t count;
};
static THREAD_LOCAL struct scene_setter_state *scene_setter_active;

static unsigned scene_setter_id(const obs_source_t *source)
{
	if (!source)
		return 0;
	for (unsigned i = 0; i < 3; i++)
		if (source == &scene_setter_active->sources[i])
			return i + 1;
	abort();
}
static void scene_setter_record(unsigned kind, obs_source_t *source)
{
	struct scene_setter_state *s = scene_setter_active;
	if (s->count == 32)
		abort();
	s->events[s->count++] = (struct scene_setter_event){kind, scene_setter_id(source),
							    scene_setter_id(s->item.show_transition),
							    scene_setter_id(s->item.hide_transition)};
}
void oracle_scene_setter_release(obs_source_t *source);
void oracle_scene_setter_release(obs_source_t *source)
{
	scene_setter_record(1, source);
	scene_setter_active->refs[scene_setter_id(source) - 1]--;
}
obs_source_t *oracle_scene_setter_get_ref(obs_source_t *source);
obs_source_t *oracle_scene_setter_get_ref(obs_source_t *source)
{
	scene_setter_record(2, source);
	if (!source)
		return NULL;
	int *refs = &scene_setter_active->refs[scene_setter_id(source) - 1];
	if (*refs <= 0)
		return NULL;
	++*refs;
	return source;
}

void oracle_obs_sceneitem_set_transition(obs_sceneitem_t *item, bool show, obs_source_t *transition);
void oracle_obs_sceneitem_set_transition_duration(obs_sceneitem_t *item, bool show, uint32_t duration_ms);
#define obs_source_release oracle_scene_setter_release
#define obs_source_get_ref oracle_scene_setter_get_ref
#define obs_sceneitem_set_transition oracle_obs_sceneitem_set_transition
#define obs_sceneitem_set_transition_duration oracle_obs_sceneitem_set_transition_duration
#include "obs-scene-item-transition-setters.c.h"
#undef obs_source_release
#undef obs_source_get_ref
#undef obs_sceneitem_set_transition
#undef obs_sceneitem_set_transition_duration

typedef void (*scene_setter_transition_fn)(obs_sceneitem_t *, bool, obs_source_t *);
typedef void (*scene_setter_duration_fn)(obs_sceneitem_t *, bool, uint32_t);
static void scene_setter_run(struct scene_setter_state *s, scene_setter_transition_fn transition,
			     scene_setter_duration_fn duration, bool show, unsigned replacement, uint32_t ms)
{
	obs_source_t *next = replacement ? &s->sources[replacement - 1] : NULL;
	/* NULL must not even call get_ref(NULL). */
	transition(NULL, show, next);
	duration(NULL, show, ms);
	transition(&s->item, show, next);
	duration(&s->item, show, ms);
	transition(&s->item, !show, next);
	duration(&s->item, !show, ~ms);
	transition(&s->item, show, next);
	transition(&s->item, !show, NULL);
}

bool oracle_scene_setter_compare(scene_setter_transition_fn transition, scene_setter_duration_fn duration, bool show,
				 uint8_t flags, uint8_t replacement, uint32_t ms);
bool oracle_scene_setter_compare(scene_setter_transition_fn transition, scene_setter_duration_fn duration, bool show,
				 uint8_t flags, uint8_t replacement, uint32_t ms)
{
	struct scene_setter_state *s = calloc(1, sizeof(*s));
	struct scene_setter_state *initial = malloc(sizeof(*initial));
	struct scene_setter_state *expected = malloc(sizeof(*expected));
	if (!s || !initial || !expected)
		abort();
	s->item.show_transition = (flags & 1) ? &s->sources[0] : NULL;
	s->item.hide_transition = (flags & 2) ? &s->sources[1] : NULL;
	s->item.show_transition_duration = 250;
	s->item.hide_transition_duration = 750;
	s->item.id = 901;
	s->item.visible = true;
	s->refs[0] = (flags & 4) ? 2 : 1;
	s->refs[1] = (flags & 8) ? 2 : 1;
	s->refs[2] = (flags & 16) ? 1 : 0;
	memcpy(initial, s, sizeof(*s));
	struct scene_setter_state *previous = scene_setter_active;
	scene_setter_active = s;
	scene_setter_run(s, oracle_obs_sceneitem_set_transition, oracle_obs_sceneitem_set_transition_duration, show,
			 replacement, ms);
	memcpy(expected, s, sizeof(*s));
	memcpy(s, initial, sizeof(*s));
	scene_setter_run(s, transition, duration, show, replacement, ms);
	bool equal = memcmp(expected, s, sizeof(*s)) == 0;
	scene_setter_active = previous;
	free(expected);
	free(initial);
	free(s);
	return equal;
}
