/* Original obs_enum_sources(), obs_enum_all_sources(), obs_enum_canvases()
 * and obs_canvas_enum_scenes() (#5495) and the real-header adapters of
 * obs-enum-sources-fields.c.h on one recording fixture core. Under cargo
 * test these adapters are the only definitions of their real names, so the
 * Rust shims reach this fixture.
 *
 * The fixture core holds a pool of real struct obs_source entries, each
 * with a type, an id (the group's or an input's), a private flag and a
 * reference count (-1: none left), linked in the core's uuid table
 * (hh_uuid) in the case's order and in canvas 1's own table (hh) in
 * reverse order, the main canvas's table left empty; and a pool of real struct obs_canvas entries linked as the named canvases (hh).
 * The mutex calls only count holds, per mutex. Traced: every lock and
 * unlock with its mutex, every get_ref with its answer, every release and
 * every callback with the entry's count at the call, plus the callback's
 * own actions (unlinking the current entry from the table walked, taking
 * the last reference of the next entry). A get_ref, release, next read or
 * callback outside the walk's lock, a canvases head read under the
 * canvases' lock (the original reads it before locking), a callback with
 * another param and an unknown pointer are marked. The probe runs the
 * original and a candidate on the same fixture and reports, instead of
 * aborting, any difference in the trace or the final entries, links and
 * counts, or a mutex left held, so mutants are observable.
 *
 * Excluded: an uninitialized core, a dead canvas or listed entry, a NULL
 * callback (the Rust shims walk nothing for one; the original asserts it or
 * calls it) and a table change from another thread, outside the original's
 * contract; the real tables stay unchanged Tier 2 (test_enum_sources,
 * test_enum_scenes). No intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define ES_POOL 8
#define ES_CANVASES 8
#define ES_TRACE 1024
#define ES_NEVER 0xff

/* Entry flags, as the Rust side sends them. */
#define ES_PRIVATE 0x01
#define ES_TYPE_SHIFT 1
#define ES_TYPE_MASK 0x06
#define ES_GROUP_ID 0x08
#define ES_DEAD 0x10

/* The walks. */
#define ES_SOURCES 0
#define ES_ALL 1
#define ES_CANVAS_LIST 2
#define ES_SCENES 3

/* One case: `count` entries (sources, or named canvases for ES_CANVAS_LIST)
 * with `flags[i]`; the callback answers false at visit `stop_after`,
 * unlinks the current entry at visit `unlink_at` and takes the last
 * reference of the walk's next entry at visit `kill_next_at` (ES_NEVER:
 * never). */
struct oracle_es_case {
	const uint8_t *flags;
	size_t count;
	uint8_t walk;
	uint8_t stop_after;
	uint8_t unlink_at;
	uint8_t kill_next_at;
};

typedef bool (*es_proc)(void *, void *);
typedef void (*es_enum_fn)(es_proc, void *);
typedef void (*es_canvas_fn)(obs_canvas_t *, es_proc, void *);

/* The candidates, in walk order; `scenes` is obs_canvas_enum_scenes(). */
struct oracle_es_candidates {
	es_enum_fn sources;
	es_enum_fn all;
	es_enum_fn canvases;
	es_canvas_fn scenes;
};

/* What the probe compares besides the trace. */
struct es_state {
	struct obs_core core;
	struct obs_source sources[ES_POOL];
	struct obs_canvas canvases[ES_CANVASES];
	long refs[ES_POOL];
	unsigned depth_sources, depth_canvases, depth_canvas[ES_CANVASES];
};

struct es_fixture {
	struct es_state state;
	const struct oracle_es_case *c;
	char params[2];
	size_t visits;
	char trace[ES_TRACE];
	size_t trace_len;
};

#if defined(_MSC_VER)
static __declspec(thread) struct es_fixture *es_current;
#else
static _Thread_local struct es_fixture *es_current;
#endif

extern const struct obs_source_info oracle_enum_sources_group_info;
const struct obs_source_info oracle_enum_sources_group_info = {.id = "group"};

static void es_event(char value)
{
	struct es_fixture *f = es_current;
	if (f->trace_len + 1 < sizeof(f->trace)) {
		f->trace[f->trace_len++] = value;
		f->trace[f->trace_len] = '\0';
	}
}

static struct obs_core *es_read_core(void)
{
	return &es_current->state.core;
}

static size_t es_source_index(const void *source)
{
	const struct es_state *s = &es_current->state;
	for (size_t i = 0; i < ES_POOL; i++) {
		if (source == &s->sources[i])
			return i;
	}
	return ES_POOL;
}

static size_t es_canvas_index(const void *canvas)
{
	const struct es_state *s = &es_current->state;
	for (size_t i = 0; i < ES_CANVASES; i++) {
		if (canvas == &s->canvases[i])
			return i;
	}
	return ES_CANVASES;
}

/* The hold count of a fixture mutex; NULL for another one. */
static unsigned *es_depth(const pthread_mutex_t *mutex, char *name)
{
	struct es_state *s = &es_current->state;
	if (mutex == &s->core.data.sources_mutex) {
		*name = 's';
		return &s->depth_sources;
	}
	if (mutex == &s->core.data.canvases_mutex) {
		*name = 'n';
		return &s->depth_canvases;
	}
	for (size_t i = 0; i < ES_CANVASES; i++) {
		if (mutex == &s->canvases[i].sources_mutex) {
			*name = (char)('0' + (int)i);
			return &s->depth_canvas[i];
		}
	}
	return NULL;
}

/* 'L' and the mutex; 'R' marks another mutex. */
static int es_lock(pthread_mutex_t *mutex)
{
	char name;
	unsigned *depth = es_depth(mutex, &name);
	if (!depth) {
		es_event('R');
		return 0;
	}
	es_event('L');
	es_event(name);
	(*depth)++;
	return 0;
}

/* 'U' and the mutex; 'R' marks an unlock without a hold or of another
 * mutex. */
static int es_unlock(pthread_mutex_t *mutex)
{
	char name;
	unsigned *depth = es_depth(mutex, &name);
	if (!depth || !*depth) {
		es_event('R');
		return 0;
	}
	es_event('U');
	es_event(name);
	(*depth)--;
	return 0;
}

/* The hold count of the mutex the case's walk runs under. */
static unsigned es_walk_depth(void)
{
	const struct es_state *s = &es_current->state;
	switch (es_current->c->walk) {
	case ES_SOURCES:
	case ES_ALL:
		return s->depth_sources;
	case ES_CANVAS_LIST:
		return s->depth_canvases;
	default:
		return s->depth_canvas[1];
	}
}

/* 'X' marks a step outside the walk's lock. */
static void es_check_locked(void)
{
	if (!es_walk_depth())
		es_event('X');
}

static char es_source_name(size_t index)
{
	return (char)('a' + (int)index);
}

/* 'g', the source, then '+' for a reference or '-' for none left. */
static obs_source_t *es_get_ref(obs_source_t *source)
{
	size_t i = es_source_index(source);
	es_check_locked();
	if (i == ES_POOL) {
		es_event('?');
		return NULL;
	}
	es_event('g');
	es_event(es_source_name(i));
	long *refs = &es_current->state.refs[i];
	if (*refs < 0) {
		es_event('-');
		return NULL;
	}
	(*refs)++;
	es_event('+');
	return source;
}

/* 'r' and the source. */
static void es_release(obs_source_t *source)
{
	size_t i = es_source_index(source);
	es_check_locked();
	if (i == ES_POOL) {
		es_event('?');
		return;
	}
	es_event('r');
	es_event(es_source_name(i));
	if (es_current->state.refs[i] >= 0)
		es_current->state.refs[i]--;
}

void oracle_obs_enum_sources(bool (*enum_proc)(void *, obs_source_t *), void *param);
void oracle_obs_enum_all_sources(bool (*enum_proc)(void *, obs_source_t *), void *param);
void oracle_obs_enum_canvases(bool (*enum_proc)(void *, obs_canvas_t *), void *param);
void oracle_obs_canvas_enum_scenes(obs_canvas_t *canvas, bool (*enum_proc)(void *, obs_source_t *), void *param);

/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name for the rest of the file and use the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock es_lock
#define pthread_mutex_unlock es_unlock
#define obs es_read_core()
#define obs_source_get_ref es_get_ref
#define obs_source_release es_release
#define group_info oracle_enum_sources_group_info
#define obs_enum_sources oracle_obs_enum_sources
#define obs_enum_all_sources oracle_obs_enum_all_sources
#define obs_enum_canvases oracle_obs_enum_canvases
#define obs_canvas_enum_scenes oracle_obs_canvas_enum_scenes
#include "libobs/obs-enum-sources.c.h"
#undef obs_canvas_enum_scenes
#undef obs_enum_canvases
#undef obs_enum_all_sources
#undef obs_enum_sources
/* The table read adapters are wrapped below to check the lock. */
#define obs_enum_sources_first es_inner_first
#define obs_enum_sources_next es_inner_next
#define obs_enum_sources_canvas_first es_inner_canvas_first
#define obs_enum_sources_canvas_next es_inner_canvas_next
#define obs_enum_sources_canvases_first es_inner_canvases_first
#define obs_enum_sources_canvases_next es_inner_canvases_next
#include "enum-sources-fields.inc"
#undef obs_enum_sources_canvases_next
#undef obs_enum_sources_canvases_first
#undef obs_enum_sources_canvas_next
#undef obs_enum_sources_canvas_first
#undef obs_enum_sources_next
#undef obs_enum_sources_first
#undef group_info
#undef obs_source_release
#undef obs_source_get_ref
#undef obs
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

obs_source_t *obs_enum_sources_first(void);
obs_source_t *obs_enum_sources_first(void)
{
	es_check_locked();
	return es_inner_first();
}

obs_source_t *obs_enum_sources_next(const obs_source_t *source);
obs_source_t *obs_enum_sources_next(const obs_source_t *source)
{
	es_check_locked();
	return es_inner_next(source);
}

obs_source_t *obs_enum_sources_canvas_first(const obs_canvas_t *canvas);
obs_source_t *obs_enum_sources_canvas_first(const obs_canvas_t *canvas)
{
	es_check_locked();
	return es_inner_canvas_first(canvas);
}

obs_source_t *obs_enum_sources_canvas_next(const obs_source_t *source);
obs_source_t *obs_enum_sources_canvas_next(const obs_source_t *source)
{
	es_check_locked();
	return es_inner_canvas_next(source);
}

/* 'H' marks the canvases head read under their lock: the original reads it
 * before locking. */
obs_canvas_t *obs_enum_sources_canvases_first(void);
obs_canvas_t *obs_enum_sources_canvases_first(void)
{
	if (es_current->state.depth_canvases)
		es_event('H');
	return es_inner_canvases_first();
}

obs_canvas_t *obs_enum_sources_canvases_next(const obs_canvas_t *canvas);
obs_canvas_t *obs_enum_sources_canvases_next(const obs_canvas_t *canvas)
{
	es_check_locked();
	return es_inner_canvases_next(canvas);
}

/* Unlinks `index` from a singly walked chain whose links live at
 * `link_offset` in each element of `pool` (stride `size`), then clears the
 * element's own link, as a callback that frees and reuses it would leave
 * it: a walk that reads the link after the callback stops there. */
static void es_unlink(void **head, char *pool, size_t size, size_t count, size_t link_offset, size_t index)
{
	char *target = pool + size * index;
	void **target_link = (void **)(target + link_offset);
	void *target_next = *target_link;
	if (*head == target) {
		*head = target_next;
	} else {
		for (size_t i = 0; i < count; i++) {
			void **link = (void **)(pool + size * i + link_offset);
			if (*link == target) {
				*link = target_next;
				break;
			}
		}
	}
	*target_link = NULL;
}

/* 'c', the entry and its count at the call ('0' for -1, '1' for 0 ...);
 * 'P' marks another param. Then the case's actions: 'K' unlinks the current
 * entry from the table walked, 'D' takes the last reference of the next
 * entry in it. */
static bool es_visit(void *param, void *object)
{
	struct es_fixture *f = es_current;
	struct es_state *s = &f->state;
	const struct oracle_es_case *c = f->c;
	es_check_locked();
	if (param != &f->params[0])
		es_event('P');

	size_t index;
	if (c->walk == ES_CANVAS_LIST) {
		index = es_canvas_index(object);
		if (index == ES_CANVASES) {
			es_event('?');
			return true;
		}
		es_event('c');
		es_event((char)('A' + (int)index));
	} else {
		index = es_source_index(object);
		if (index == ES_POOL) {
			es_event('?');
			return true;
		}
		es_event('c');
		es_event(es_source_name(index));
		es_event((char)('1' + (int)s->refs[index]));
	}

	size_t visit = f->visits++;
	if (visit == c->unlink_at) {
		es_event('K');
		switch (c->walk) {
		case ES_SOURCES:
		case ES_ALL:
			es_unlink((void **)&s->core.data.sources, (char *)s->sources, sizeof(s->sources[0]), ES_POOL,
				  offsetof(struct obs_source, context.hh_uuid.next), index);
			break;
		case ES_CANVAS_LIST:
			es_unlink((void **)&s->core.data.named_canvases, (char *)s->canvases, sizeof(s->canvases[0]),
				  ES_CANVASES, offsetof(struct obs_canvas, context.hh.next), index);
			break;
		default:
			es_unlink((void **)&s->canvases[1].sources, (char *)s->sources, sizeof(s->sources[0]), ES_POOL,
				  offsetof(struct obs_source, context.hh.next), index);
			break;
		}
	}
	if (visit == c->kill_next_at && c->walk != ES_CANVAS_LIST) {
		void *next = c->walk == ES_SCENES ? s->sources[index].context.hh.next
						  : s->sources[index].context.hh_uuid.next;
		size_t n = es_source_index(next);
		if (n < ES_POOL) {
			es_event('D');
			s->refs[n] = -1;
		}
	}
	return visit != c->stop_after;
}

static bool es_valid(const struct oracle_es_case *c)
{
	return c->walk <= ES_SCENES && c->count <= ES_POOL && c->count <= ES_CANVASES && (c->count == 0 || c->flags);
}

static const char *const es_ids[] = {"es_input", "es_scene", "es_filter", "es_transition"};
static const enum obs_source_type es_types[] = {
	OBS_SOURCE_TYPE_INPUT,
	OBS_SOURCE_TYPE_SCENE,
	OBS_SOURCE_TYPE_FILTER,
	OBS_SOURCE_TYPE_TRANSITION,
};

static void es_arm(struct es_fixture *f, const struct oracle_es_case *c)
{
	struct es_state *s = &f->state;
	memset(f, 0, sizeof(*f));
	/* Every byte around the fields read is filled, so a write anywhere
	 * shows. */
	memset(s->sources, 0x5a, sizeof(s->sources));
	memset(s->canvases, 0x5a, sizeof(s->canvases));
	f->c = c;
	es_current = f;

	size_t sources = c->walk == ES_CANVAS_LIST ? 0 : c->count;
	for (size_t i = 0; i < ES_POOL; i++) {
		struct obs_source *source = &s->sources[i];
		uint8_t flags = i < sources ? c->flags[i] : 0;
		size_t type = (size_t)((flags & ES_TYPE_MASK) >> ES_TYPE_SHIFT);
		source->info.type = es_types[type];
		source->info.id = (flags & ES_GROUP_ID) ? "group" : es_ids[type];
		source->context.private = (flags & ES_PRIVATE) != 0;
		s->refs[i] = (flags & ES_DEAD) ? -1 : 0;
		/* The uuid table in the case's order; canvas 1's table reversed. */
		source->context.hh_uuid.next = i + 1 < sources ? &s->sources[i + 1] : NULL;
		source->context.hh.next = i > 0 && i < sources ? &s->sources[i - 1] : NULL;
	}
	s->core.data.sources = sources ? &s->sources[0] : NULL;

	size_t canvases = c->walk == ES_CANVAS_LIST ? c->count : 2;
	for (size_t i = 0; i < ES_CANVASES; i++) {
		struct obs_canvas *canvas = &s->canvases[i];
		canvas->context.hh.next = i + 1 < canvases ? &s->canvases[i + 1] : NULL;
		canvas->sources = NULL;
	}
	s->core.data.named_canvases = canvases ? &s->canvases[0] : NULL;
	/* The main canvas (0) holds nothing, so a walk of the wrong canvas
	 * shows. */
	if (c->walk != ES_CANVAS_LIST && sources)
		s->canvases[1].sources = &s->sources[sources - 1];
}

static void es_run(const struct oracle_es_candidates *fns, const struct oracle_es_case *c, struct es_fixture *f)
{
	es_arm(f, c);
	switch (c->walk) {
	case ES_SOURCES:
		fns->sources(es_visit, &f->params[0]);
		break;
	case ES_ALL:
		fns->all(es_visit, &f->params[0]);
		break;
	case ES_CANVAS_LIST:
		fns->canvases(es_visit, &f->params[0]);
		break;
	default:
		fns->scenes(&f->state.canvases[1], es_visit, &f->params[0]);
		break;
	}
	es_current = NULL;
}

static bool es_released(const struct es_state *s)
{
	bool released = !s->depth_sources && !s->depth_canvases;
	for (size_t i = 0; i < ES_CANVASES; i++)
		released = released && !s->depth_canvas[i];
	return released;
}

static const struct oracle_es_candidates es_originals = {
	(es_enum_fn)oracle_obs_enum_sources,
	(es_enum_fn)oracle_obs_enum_all_sources,
	(es_enum_fn)oracle_obs_enum_canvases,
	(es_canvas_fn)oracle_obs_canvas_enum_scenes,
};

/* 0: match. 1: the traces differ, 2: the final entries, links or counts
 * differ, 4: the candidate (or the original) left a mutex held. 0x100: the
 * case does not fit the fixture, 0x200: out of memory. */
unsigned oracle_enum_sources_probe(const struct oracle_es_candidates *candidates, const struct oracle_es_case *c);
unsigned oracle_enum_sources_probe(const struct oracle_es_candidates *candidates, const struct oracle_es_case *c)
{
	if (!es_valid(c))
		return 0x100;
	/* Both runs use the same fixture memory, so the links compare. */
	struct es_fixture *f = malloc(sizeof(*f));
	struct es_fixture *original = malloc(sizeof(*original));
	if (!f || !original) {
		free(original);
		free(f);
		return 0x200;
	}
	es_run(&es_originals, c, f);
	memcpy(original, f, sizeof(*f));
	es_run(candidates, c, f);
	unsigned diff = 0;
	if (strcmp(original->trace, f->trace) != 0)
		diff |= 1;
	if (memcmp(&original->state, &f->state, sizeof(original->state)) != 0)
		diff |= 2;
	if (!es_released(&original->state) || !es_released(&f->state))
		diff |= 4;
	free(original);
	free(f);
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void es_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* The original's trace, for the characterization checks; false when it left
 * a mutex held or the case does not fit. */
bool oracle_enum_sources_original(const struct oracle_es_case *c, char *trace, size_t trace_size);
bool oracle_enum_sources_original(const struct oracle_es_case *c, char *trace, size_t trace_size)
{
	if (!es_valid(c))
		return false;
	struct es_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	es_run(&es_originals, c, f);
	es_copy(trace, trace_size, f->trace);
	bool released = es_released(&f->state);
	free(f);
	return released;
}

/* The adapters against the real fields. */
bool oracle_enum_sources_adapters(void);
bool oracle_enum_sources_adapters(void)
{
	static const uint8_t flags[] = {
		0,                                  /* public input */
		ES_PRIVATE | (1 << ES_TYPE_SHIFT),  /* private scene */
		ES_GROUP_ID | (1 << ES_TYPE_SHIFT), /* group */
		ES_DEAD | (3 << ES_TYPE_SHIFT),     /* transition, no reference */
	};
	struct oracle_es_case c = {flags, 4, ES_SOURCES, ES_NEVER, ES_NEVER, ES_NEVER};
	struct es_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	es_arm(f, &c);
	struct es_state *s = &f->state;
	obs_source_t *src = s->sources;
	bool ok = true;

	obs_enum_sources_lock();
	ok = ok && s->depth_sources == 1 && obs_enum_sources_first() == &src[0] &&
	     obs_enum_sources_next(&src[0]) == &src[1] && obs_enum_sources_next(&src[3]) == NULL &&
	     obs_enum_sources_get_ref(&src[0]) == &src[0] && obs_enum_sources_get_ref(&src[3]) == NULL &&
	     s->refs[0] == 1;
	obs_enum_sources_release(&src[0]);
	ok = ok && s->refs[0] == 0;
	ok = ok && !obs_enum_sources_private(&src[0]) && obs_enum_sources_private(&src[1]);
	ok = ok && obs_enum_sources_input(&src[0]) && !obs_enum_sources_input(&src[1]) &&
	     !obs_enum_sources_input(&src[2]);
	ok = ok && !obs_enum_sources_group(&src[0]) && !obs_enum_sources_group(&src[1]) &&
	     obs_enum_sources_group(&src[2]);
	ok = ok && !obs_enum_sources_scene(&src[0]) && obs_enum_sources_scene(&src[1]) &&
	     obs_enum_sources_scene(&src[2]) && !obs_enum_sources_scene(&src[3]);
	obs_enum_sources_unlock();
	ok = ok && s->depth_sources == 0;

	/* Each table's steps under the lock its walk takes. */
	c.walk = ES_SCENES;
	obs_canvas_t *canvas = &s->canvases[1];
	obs_enum_sources_canvas_lock(canvas);
	ok = ok && s->depth_canvas[1] == 1 && obs_enum_sources_canvas_first(canvas) == &src[3] &&
	     obs_enum_sources_canvas_next(&src[3]) == &src[2] && obs_enum_sources_canvas_next(&src[0]) == NULL;
	obs_enum_sources_canvas_unlock(canvas);
	ok = ok && s->depth_canvas[1] == 0;

	c.walk = ES_CANVAS_LIST;
	ok = ok && obs_enum_sources_canvases_first() == &s->canvases[0];
	obs_enum_sources_canvases_lock();
	ok = ok && s->depth_canvases == 1 && obs_enum_sources_canvases_next(&s->canvases[0]) == &s->canvases[1] &&
	     obs_enum_sources_canvases_next(&s->canvases[1]) == NULL;
	obs_enum_sources_canvases_unlock();
	ok = ok && s->depth_canvases == 0;

	ok = ok && strcmp(f->trace, "Lsga+gd-raUsL1U1LnUn") == 0;
	es_current = NULL;
	free(f);
	return ok;
}
