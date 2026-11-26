/* Tier 3: the unchanged extracted obs_source_get_width() and
 * obs_source_get_height(), renamed oracle_*, and the production field and
 * lock adapters the Rust shims go through (obs-source-get-size-fields.c.h,
 * defined once, here), over the real private layout.
 *
 * Both sides reach the same libobs: obs_source_size_entry() and
 * obs_source_base_width() / obs_source_base_height() (the exports the test
 * binary links), which call each node's get_width / get_height callback.
 * The callbacks record which node answered, the axis and which nodes'
 * filter_mutex is held at that moment (an error-checking mutex, so a
 * trylock from the owning thread reports it held). The real sources are
 * covered by the Tier 2 tests that reach these exports. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_width oracle_obs_source_get_width
#define obs_source_get_height oracle_obs_source_get_height
#include "libobs/build/libobs.pch.h"
#include "source-get-size.inc"
#undef obs_source_get_height
#undef obs_source_get_width
#include "source_get_size_fields.inc"

#define GET_SIZE_NODES 4
#define GET_SIZE_FILTERS 3
#define GET_SIZE_EVENTS 16

/* One call: width (which 0) or height (1) of node `source` (-1 for NULL).
 * Each node has a type, data or not, enabled or not, a filter parent and
 * target (-1 for NULL), a filter list and a width and height. */
struct get_size_case {
	uint8_t which;
	int8_t source;
	uint8_t noise;
	int32_t type[GET_SIZE_NODES];
	bool data[GET_SIZE_NODES];
	bool enabled[GET_SIZE_NODES];
	int8_t filter_parent[GET_SIZE_NODES];
	int8_t filter_target[GET_SIZE_NODES];
	uint8_t filters[GET_SIZE_NODES];
	int8_t filter[GET_SIZE_NODES][GET_SIZE_FILTERS];
	uint32_t size[GET_SIZE_NODES][2];
};

/* What one side did: its callback events in order, three words each (node,
 * axis, mask of nodes whose filter_mutex was held), what it returned, its
 * log lines (count, level of the last, the last), and whether every
 * filter_mutex was free afterwards. */
struct get_size_result {
	uint32_t events;
	uint64_t trace[3 * GET_SIZE_EVENTS];
	uint32_t returned;
	uint32_t log_count;
	int32_t log_level;
	char log[160];
	bool unlocked;
};

struct get_size_data {
	uint32_t index;
	uint32_t size[2];
};

struct get_size_fixture {
	struct obs_source nodes[GET_SIZE_NODES];
	struct get_size_data data[GET_SIZE_NODES];
	obs_source_t *lists[GET_SIZE_NODES][GET_SIZE_FILTERS];
	struct get_size_result *out;
};

static _Thread_local struct get_size_fixture *get_size_current;

static uint64_t get_size_held(void)
{
	struct get_size_fixture *f = get_size_current;
	uint64_t mask = 0;
	for (size_t i = 0; i < GET_SIZE_NODES; i++) {
		if (pthread_mutex_trylock(&f->nodes[i].filter_mutex) == 0)
			pthread_mutex_unlock(&f->nodes[i].filter_mutex);
		else
			mask |= (uint64_t)1 << i;
	}
	return mask;
}

static uint32_t get_size_answer(void *data, uint64_t axis)
{
	struct get_size_data *d = data;
	struct get_size_result *out = get_size_current->out;
	if (out->events < GET_SIZE_EVENTS) {
		out->trace[3 * out->events] = d->index;
		out->trace[3 * out->events + 1] = axis;
		out->trace[3 * out->events + 2] = get_size_held();
	}
	out->events++;
	return d->size[axis];
}

static uint32_t get_size_width(void *data)
{
	return get_size_answer(data, 0);
}

static uint32_t get_size_height(void *data)
{
	return get_size_answer(data, 1);
}

static obs_source_t *get_size_node(struct get_size_fixture *f, int8_t index)
{
	return index < 0 || index >= GET_SIZE_NODES ? NULL : &f->nodes[index];
}

#if defined(__GNUC__) || defined(__clang__)
static void get_size_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void get_size_capture(int level, const char *format, va_list args, void *param)
{
	struct get_size_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef uint32_t (*get_size_fn)(obs_source_t *);
typedef void (*get_size_get_log)(log_handler_t *, void **);
typedef void (*get_size_set_log)(log_handler_t, void *);

/* The original calls, for the parity test's other side. */
void oracle_source_get_size_originals(get_size_fn *width, get_size_fn *height);
void oracle_source_get_size_originals(get_size_fn *width, get_size_fn *height)
{
	*width = oracle_obs_source_get_width;
	*height = oracle_obs_source_get_height;
}

/* Runs one case through width or height on fresh noise-filled nodes. False
 * when the fixture could not be built or an adapter does not reach its
 * field. */
bool oracle_source_get_size_run(get_size_fn width, get_size_fn height, get_size_get_log get, get_size_set_log set,
				const struct get_size_case *c, struct get_size_result *out);
bool oracle_source_get_size_run(get_size_fn width, get_size_fn height, get_size_get_log get, get_size_set_log set,
				const struct get_size_case *c, struct get_size_result *out)
{
	struct get_size_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	memset(f, c->noise, sizeof(*f));
	memset(out, 0, sizeof(*out));
	out->log_level = -1;
	f->out = out;
	get_size_current = f;

	pthread_mutexattr_t attr;
	if (pthread_mutexattr_init(&attr) != 0) {
		get_size_current = NULL;
		free(f);
		return false;
	}
	pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_ERRORCHECK);
	bool valid = true;
	for (size_t i = 0; i < GET_SIZE_NODES; i++) {
		struct obs_source *n = &f->nodes[i];
		f->data[i].index = (uint32_t)i;
		f->data[i].size[0] = c->size[i][0];
		f->data[i].size[1] = c->size[i][1];
		n->context.data = c->data[i] ? &f->data[i] : NULL;
		n->info.type = (enum obs_source_type)c->type[i];
		n->info.get_width = get_size_width;
		n->info.get_height = get_size_height;
		n->enabled = c->enabled[i];
		n->async_active = false;
		n->async_rotation = 0;
		n->transition_actual_cx = (long)c->size[i][0] + 1;
		n->transition_actual_cy = (long)c->size[i][1] + 1;
		n->filter_parent = get_size_node(f, c->filter_parent[i]);
		n->filter_target = get_size_node(f, c->filter_target[i]);
		uint8_t count = c->filters[i] > GET_SIZE_FILTERS ? GET_SIZE_FILTERS : c->filters[i];
		for (uint8_t k = 0; k < count; k++)
			f->lists[i][k] = get_size_node(f, c->filter[i][k]);
		n->filters.array = f->lists[i];
		n->filters.num = count;
		n->filters.capacity = GET_SIZE_FILTERS;
		valid = valid && pthread_mutex_init(&n->filter_mutex, &attr) == 0;
	}
	pthread_mutexattr_destroy(&attr);

	/* The adapters reach the fields they name. */
	for (size_t i = 0; valid && i < GET_SIZE_NODES; i++) {
		obs_source_t *n = &f->nodes[i];
		valid = obs_source_get_size_has_data(n) == c->data[i] &&
			obs_source_get_size_type(n) == (int)n->info.type &&
			obs_source_get_size_has_filters(n) == (n->filters.num != 0) &&
			(n->filters.num == 0 || obs_source_get_size_first_filter(n) == n->filters.array[0]);
		obs_source_get_size_lock(n);
		valid = valid && get_size_held() == (uint64_t)1 << i;
		obs_source_get_size_unlock(n);
		valid = valid && get_size_held() == 0;
	}

	if (valid) {
		log_handler_t old_handler;
		void *old_param;
		get(&old_handler, &old_param);
		set(get_size_capture, out);
		obs_source_t *source = get_size_node(f, c->source);
		out->returned = c->which == 0 ? width(source) : height(source);
		set(old_handler, old_param);
		out->unlocked = get_size_held() == 0;
	}
	get_size_current = NULL;

	for (size_t i = 0; i < GET_SIZE_NODES; i++)
		pthread_mutex_destroy(&f->nodes[i].filter_mutex);
	free(f);
	return valid;
}
