/* Tier 3: the unchanged extracted obs_source_copy_filters(),
 * obs_source_copy_single_filter() and duplicate_filters()
 * (libobs/obs-source-copy-filters.c.h) over the real private layout, and
 * the production field adapters the Rust shim calls
 * (obs-source-copy-filters-fields.c.h, defined once, here).
 *
 * Both sides reach the same recording stand-ins for the libobs calls the
 * copies make (the reference, the naming, the compatibility check, the
 * enabled state, the duplicate, the add, the release and bfree()), so the
 * trace of calls, with the lock state at each, compares exactly. The real
 * calls run through the real core in test_source_copy_filters. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_copy_filters oracle_obs_source_copy_filters
#define obs_source_copy_single_filter oracle_obs_source_copy_single_filter
#define obs_source_duplicate_filters oracle_obs_source_duplicate_filters
#include "libobs/build/libobs.pch.h"

#define SCF_FILTERS 7
#define SCF_EVENTS 160
#define SCF_NAMES 16

/* Source ids in the trace. */
enum {
	SCF_NULL = -1,
	SCF_OTHER = -2,
	SCF_DST = 0,
	SCF_SRC = 1,
	SCF_FILTER0 = 2,
	SCF_COPY0 = SCF_FILTER0 + SCF_FILTERS,
};

/* Event kinds. */
enum {
	SCF_EV_GET_REF = 1,
	SCF_EV_COMPATIBLE,
	SCF_EV_NEW_NAME,
	SCF_EV_ENABLED,
	SCF_EV_DUPLICATE,
	SCF_EV_SET_ENABLED,
	SCF_EV_FREE,
	SCF_EV_ADD,
	SCF_EV_RELEASE,
};

/* One event: kind, two ids or values, a third value, and whether the
 * source's filter_mutex was held. */
#define SCF_EVENT_WIDTH 5

struct scf_fixture {
	struct obs_source dst;
	struct obs_source src;
	struct obs_source filters[SCF_FILTERS];
	struct obs_source copies[SCF_FILTERS];
	obs_source_t *list[SCF_FILTERS];
	struct obs_source dst_before;
	struct obs_source src_before;
	struct obs_source filters_before[SCF_FILTERS];
	unsigned dying;
	unsigned disabled;
	unsigned incompatible;
	unsigned no_duplicate;
	char *names[SCF_NAMES];
	size_t name_count;
	long events[SCF_EVENTS][SCF_EVENT_WIDTH];
	size_t event_count;
	bool overflow;
};

/* Per thread: the Rust test harness runs cases on several threads. */
static _Thread_local struct scf_fixture *scf_current;

static long scf_id(const obs_source_t *source)
{
	struct scf_fixture *f = scf_current;
	if (!source)
		return SCF_NULL;
	if (source == &f->dst)
		return SCF_DST;
	if (source == &f->src)
		return SCF_SRC;
	for (long i = 0; i < SCF_FILTERS; i++) {
		if (source == &f->filters[i])
			return SCF_FILTER0 + i;
		if (source == &f->copies[i])
			return SCF_COPY0 + i;
	}
	return SCF_OTHER;
}

/* Newest first: the heap may hand a freed name's address to the next name
 * (the Windows CRT does), and the live name is the latest one there. */
static long scf_name_id(const char *name)
{
	struct scf_fixture *f = scf_current;
	for (size_t i = f->name_count; i-- > 0;) {
		if (f->names[i] == name)
			return (long)i;
	}
	return -1;
}

static bool scf_locked(void)
{
	int busy = pthread_mutex_trylock(&scf_current->src.filter_mutex);
	if (busy == 0)
		pthread_mutex_unlock(&scf_current->src.filter_mutex);
	return busy != 0;
}

static void scf_event(long kind, long a, long b, long c)
{
	struct scf_fixture *f = scf_current;
	if (f->event_count >= SCF_EVENTS) {
		f->overflow = true;
		return;
	}
	long *e = f->events[f->event_count++];
	e[0] = kind;
	e[1] = a;
	e[2] = b;
	e[3] = c;
	e[4] = scf_locked();
}

static long scf_filter_index(const obs_source_t *source)
{
	long id = scf_id(source);
	return id >= SCF_FILTER0 && id < SCF_COPY0 ? id - SCF_FILTER0 : -1;
}

static obs_source_t *scf_get_ref(obs_source_t *source)
{
	long i = scf_filter_index(source);
	scf_event(SCF_EV_GET_REF, scf_id(source), 0, 0);
	if (i >= 0 && (scf_current->dying & (1u << i)))
		return NULL;
	return source;
}

static bool scf_compatible(obs_source_t *dst, obs_source_t *filter)
{
	long i = scf_filter_index(filter);
	scf_event(SCF_EV_COMPATIBLE, scf_id(dst), scf_id(filter), 0);
	return !(i >= 0 && (scf_current->incompatible & (1u << i)));
}

/* get_new_filter_name(): a fresh allocation per call, the name read from
 * the filter's context, as the original reads it. */
static char *scf_new_name(obs_source_t *dst, const char *name)
{
	struct scf_fixture *f = scf_current;
	char *copy = bstrdup(name);
	if (f->name_count < SCF_NAMES)
		f->names[f->name_count++] = copy;
	scf_event(SCF_EV_NEW_NAME, scf_id(dst), (long)f->name_count - 1, name ? (long)strlen(name) : -1);
	return copy;
}

static bool scf_enabled(const obs_source_t *source)
{
	long i = scf_filter_index(source);
	scf_event(SCF_EV_ENABLED, scf_id(source), 0, 0);
	return !(i >= 0 && (scf_current->disabled & (1u << i)));
}

static obs_source_t *scf_duplicate(obs_source_t *source, const char *name, bool private)
{
	long i = scf_filter_index(source);
	scf_event(SCF_EV_DUPLICATE, scf_id(source), scf_name_id(name), private);
	if (i < 0 || (scf_current->no_duplicate & (1u << i)))
		return NULL;
	return &scf_current->copies[i];
}

static void scf_set_enabled(obs_source_t *source, bool enabled)
{
	scf_event(SCF_EV_SET_ENABLED, scf_id(source), enabled, 0);
}

static void scf_bfree(void *ptr)
{
	scf_event(SCF_EV_FREE, scf_name_id(ptr), 0, 0);
	bfree(ptr);
}

static void scf_filter_add(obs_source_t *source, obs_source_t *filter)
{
	scf_event(SCF_EV_ADD, scf_id(source), scf_id(filter), 0);
}

static void scf_release(obs_source_t *source)
{
	scf_event(SCF_EV_RELEASE, scf_id(source), 0, 0);
}

#define obs_source_get_ref scf_get_ref
#define filter_compatible scf_compatible
#define get_new_filter_name scf_new_name
#define obs_source_enabled scf_enabled
#define obs_source_duplicate scf_duplicate
#define obs_source_set_enabled scf_set_enabled
#define bfree scf_bfree
#define obs_source_filter_add scf_filter_add
#define obs_source_release scf_release
#include "source-copy-filters.inc"
#undef obs_source_copy_filters
#undef obs_source_copy_single_filter
#undef obs_source_duplicate_filters
#include "source_copy_filters_fields.inc"
#undef obs_source_get_ref
#undef filter_compatible
#undef get_new_filter_name
#undef obs_source_enabled
#undef obs_source_duplicate
#undef obs_source_set_enabled
#undef bfree
#undef obs_source_filter_add
#undef obs_source_release

static struct scf_fixture *scf_new(uint8_t noise, size_t count, bool private)
{
	static char names[SCF_FILTERS][8] = {"a", "bb", "", "dddd", "e", "ffffff", "g"};
	struct scf_fixture *f = malloc(sizeof(*f));
	if (!f)
		return NULL;
	memset(f, noise, sizeof(*f));
	if (pthread_mutex_init(&f->src.filter_mutex, NULL) != 0) {
		free(f);
		return NULL;
	}
	for (size_t i = 0; i < SCF_FILTERS; i++) {
		f->filters[i].context.name = names[i];
		f->list[i] = &f->filters[i];
	}
	f->dst.context.private = private;
	f->src.context.private = !private;
	f->src.filters.array = f->list;
	f->src.filters.num = count;
	f->src.filters.capacity = SCF_FILTERS;
	f->dying = 0;
	f->disabled = 0;
	f->incompatible = 0;
	f->no_duplicate = 0;
	f->name_count = 0;
	f->event_count = 0;
	f->overflow = false;
	memcpy(&f->dst_before, &f->dst, sizeof(f->dst));
	memcpy(&f->src_before, &f->src, sizeof(f->src));
	memcpy(f->filters_before, f->filters, sizeof(f->filters));
	return f;
}

static bool scf_free(struct scf_fixture *f)
{
	bool ok = pthread_mutex_destroy(&f->src.filter_mutex) == 0;
	free(f);
	return ok;
}

static bool scf_equal_except(const void *before, const void *after, size_t size, size_t at, size_t len)
{
	const unsigned char *b = before;
	const unsigned char *a = after;
	return memcmp(b, a, at) == 0 && memcmp(b + at + len, a + at + len, size - at - len) == 0;
}

struct scf_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void scf_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void scf_capture(int level, const char *format, va_list args, void *param)
{
	struct scf_log *captured = param;
	captured->count++;
	captured->level = level;
	vsnprintf(captured->message, sizeof(captured->message), format, args);
}

static void scf_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		len = size - 1;
	memcpy(dst, src, len);
	dst[len] = '\0';
}

typedef void (*scf_pair)(obs_source_t *, obs_source_t *);
typedef void (*scf_triple)(obs_source_t *, obs_source_t *, bool);
typedef void (*scf_get_log)(log_handler_t *, void **);
typedef void (*scf_set_log)(log_handler_t, void *);

/* What oracle_scf_run() calls. */
enum {
	SCF_CALL_COPY_FILTERS = 0,
	SCF_CALL_COPY_SINGLE = 1,
	SCF_CALL_DUPLICATE_FILTERS = 2,
};

/* The out array before the events: the event count, overflow, the log
 * count and level, whether every fixture byte but the lock is unchanged,
 * the lock free after, and how many names are still allocated. */
enum {
	SCF_OUT_EVENTS = 0,
	SCF_OUT_OVERFLOW,
	SCF_OUT_LOGS,
	SCF_OUT_LOG_LEVEL,
	SCF_OUT_UNCHANGED,
	SCF_OUT_UNLOCKED,
	SCF_OUT_HEAD,
	SCF_OUT_COUNT = SCF_OUT_HEAD + SCF_EVENTS * SCF_EVENT_WIDTH,
};

/* Runs one call (`which`) on a fresh fixture of count filters, through
 * `pair`/`triple` (the Rust export or a mutant) or, when both are NULL, the
 * original C. dst_arg and second_arg pick the arguments: 0 NULL, 1 the
 * destination, 2 the source, 3 + i filter i. The masks pick the filters
 * whose reference fails, that are disabled, incompatible, or whose
 * duplicate fails. Writes SCF_OUT_COUNT values to out and the log line to
 * message; false when the fixture could not be built. */
bool oracle_scf_run(scf_pair pair, scf_triple triple, scf_get_log get_log, scf_set_log set_log, int which,
		    uint8_t noise, size_t count, bool private, bool private_arg, int dst_arg, int second_arg,
		    unsigned dying, unsigned disabled, unsigned incompatible, unsigned no_duplicate, long *out,
		    char *message, size_t message_size);
bool oracle_scf_run(scf_pair pair, scf_triple triple, scf_get_log get_log, scf_set_log set_log, int which,
		    uint8_t noise, size_t count, bool private, bool private_arg, int dst_arg, int second_arg,
		    unsigned dying, unsigned disabled, unsigned incompatible, unsigned no_duplicate, long *out,
		    char *message, size_t message_size)
{
	if (count > SCF_FILTERS || message_size == 0)
		return false;
	struct scf_fixture *f = scf_new(noise, count, private);
	if (!f)
		return false;
	f->dying = dying;
	f->disabled = disabled;
	f->incompatible = incompatible;
	f->no_duplicate = no_duplicate;
	obs_source_t *args[3 + SCF_FILTERS] = {NULL, &f->dst, &f->src};
	for (size_t i = 0; i < SCF_FILTERS; i++)
		args[3 + i] = &f->filters[i];
	if (dst_arg < 0 || dst_arg >= 3 + SCF_FILTERS || second_arg < 0 || second_arg >= 3 + SCF_FILTERS) {
		scf_free(f);
		return false;
	}
	obs_source_t *first = args[dst_arg];
	obs_source_t *second = args[second_arg];
	struct scf_log captured = {.level = -1};
	log_handler_t previous;
	void *param;

	scf_current = f;
	get_log(&previous, &param);
	set_log(scf_capture, &captured);
	if (which == SCF_CALL_COPY_FILTERS) {
		if (pair)
			pair(first, second);
		else
			oracle_obs_source_copy_filters(first, second);
	} else if (which == SCF_CALL_COPY_SINGLE) {
		if (pair)
			pair(first, second);
		else
			oracle_obs_source_copy_single_filter(first, second);
	} else {
		if (triple)
			triple(first, second, private_arg);
		else
			oracle_obs_source_duplicate_filters(first, second, private_arg);
	}
	set_log(previous, param);

	memset(out, 0, sizeof(long) * SCF_OUT_COUNT);
	out[SCF_OUT_EVENTS] = (long)f->event_count;
	out[SCF_OUT_OVERFLOW] = f->overflow;
	out[SCF_OUT_LOGS] = (long)captured.count;
	out[SCF_OUT_LOG_LEVEL] = captured.level;
	bool unchanged = memcmp(&f->dst_before, &f->dst, sizeof(f->dst)) == 0 &&
			 scf_equal_except(&f->src_before, &f->src, sizeof(f->src),
					  offsetof(struct obs_source, filter_mutex), sizeof(f->src.filter_mutex)) &&
			 memcmp(f->filters_before, f->filters, sizeof(f->filters)) == 0;
	out[SCF_OUT_UNCHANGED] = unchanged;
	int locked = pthread_mutex_trylock(&f->src.filter_mutex);
	if (locked == 0)
		pthread_mutex_unlock(&f->src.filter_mutex);
	out[SCF_OUT_UNLOCKED] = locked == 0;
	for (size_t i = 0; i < f->event_count; i++)
		memcpy(&out[SCF_OUT_HEAD + i * SCF_EVENT_WIDTH], f->events[i], sizeof(f->events[i]));
	scf_copy(message, message_size, captured.count ? captured.message : "");
	scf_current = NULL;
	return scf_free(f);
}

/* The sizes the Rust side mirrors: 0 the out array, 1 the most filters,
 * 2 the event width. */
size_t oracle_scf_layout(size_t which);
size_t oracle_scf_layout(size_t which)
{
	return which == 0 ? SCF_OUT_COUNT : which == 1 ? SCF_FILTERS : which == 2 ? SCF_EVENT_WIDTH : 0;
}
