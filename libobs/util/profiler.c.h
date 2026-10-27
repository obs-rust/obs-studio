#include "build/libobs-util.pch.h"

//#define TRACK_OVERHEAD

typedef struct profiler_time_entry profiler_time_entry;

typedef struct profile_call profile_call;
struct profile_call {
	const char *name;
#ifdef TRACK_OVERHEAD
	uint64_t overhead_start;
#endif
	uint64_t start_time;
	uint64_t end_time;
#ifdef TRACK_OVERHEAD
	uint64_t overhead_end;
#endif
	uint64_t expected_time_between_calls;
	DARRAY(profile_call) children;
	profile_call *parent;
};

typedef struct profile_times_table_entry profile_times_table_entry;
struct profile_times_table_entry {
	size_t probes;
	profiler_time_entry entry;
};

typedef struct profile_times_table profile_times_table;
struct profile_times_table {
	size_t size;
	size_t occupied;
	size_t max_probe_count;
	profile_times_table_entry *entries;

	size_t old_start_index;
	size_t old_occupied;
	profile_times_table_entry *old_entries;
};

typedef struct profile_entry profile_entry;
struct profile_entry {
	const char *name;
	profile_times_table times;
#ifdef TRACK_OVERHEAD
	profile_times_table overhead;
#endif
	uint64_t expected_time_between_calls;
	profile_times_table times_between_calls;
	DARRAY(profile_entry) children;
};

typedef struct profile_root_entry profile_root_entry;
struct profile_root_entry {
	pthread_mutex_t *mutex;
	const char *name;
	profile_entry *entry;
	profile_call *prev_call;
};

static inline uint64_t diff_ns_to_usec(uint64_t prev, uint64_t next)
{
	return (next - prev + 500) / 1000;
}

static inline void update_max_probes(profile_times_table *map, size_t val)
{
	map->max_probe_count = map->max_probe_count < val ? val : map->max_probe_count;
}

static void migrate_old_entries(profile_times_table *map, bool limit_items);
static void grow_hashmap(profile_times_table *map, uint64_t usec, uint64_t count);

static void add_hashmap_entry(profile_times_table *map, uint64_t usec, uint64_t count)
{
	size_t probes = 1;

	size_t start = usec % map->size;
	for (;; probes += 1) {
		size_t idx = (start + probes) % map->size;
		profile_times_table_entry *entry = &map->entries[idx];
		if (!entry->probes) {
			entry->probes = probes;
			entry->entry.time_delta = usec;
			entry->entry.count = count;
			map->occupied += 1;
			update_max_probes(map, probes);
			return;
		}

		if (entry->entry.time_delta == usec) {
			entry->entry.count += count;
			return;
		}

		if (entry->probes >= probes)
			continue;

		if ((double)map->occupied / (double)map->size > 0.7) {
			grow_hashmap(map, usec, count);
			return;
		}

		size_t old_probes = entry->probes;
		uint64_t old_count = entry->entry.count;
		uint64_t old_usec = entry->entry.time_delta;

		entry->probes = probes;
		entry->entry.count = count;
		entry->entry.time_delta = usec;

		update_max_probes(map, probes);

		probes = old_probes;
		count = old_count;
		usec = old_usec;

		start = usec % map->size;
	}
}

static void init_hashmap(profile_times_table *map, size_t size)
{
	map->size = size;
	map->occupied = 0;
	map->max_probe_count = 0;
	map->entries = bzalloc(sizeof(profile_times_table_entry) * size);
	map->old_start_index = 0;
	map->old_occupied = 0;
	map->old_entries = NULL;
}

static void migrate_old_entries(profile_times_table *map, bool limit_items)
{
	if (!map->old_entries)
		return;

	if (!map->old_occupied) {
		bfree(map->old_entries);
		map->old_entries = NULL;
		return;
	}

	for (size_t i = 0; !limit_items || i < 8; i++, map->old_start_index++) {
		if (!map->old_occupied)
			return;

		profile_times_table_entry *entry = &map->old_entries[map->old_start_index];
		if (!entry->probes)
			continue;

		add_hashmap_entry(map, entry->entry.time_delta, entry->entry.count);
		map->old_occupied -= 1;
	}
}

static void grow_hashmap(profile_times_table *map, uint64_t usec, uint64_t count)
{
	migrate_old_entries(map, false);

	size_t old_size = map->size;
	size_t old_occupied = map->occupied;
	profile_times_table_entry *entries = map->entries;

	init_hashmap(map, (old_size * 2 < 16) ? 16 : (old_size * 2));

	map->old_occupied = old_occupied;
	map->old_entries = entries;

	add_hashmap_entry(map, usec, count);
}

static profile_entry *init_entry(profile_entry *entry, const char *name)
{
	entry->name = name;
	init_hashmap(&entry->times, 1);
#ifdef TRACK_OVERHEAD
	init_hashmap(&entry->overhead, 1);
#endif
	entry->expected_time_between_calls = 0;
	init_hashmap(&entry->times_between_calls, 1);
	return entry;
}

static profile_entry *get_child(profile_entry *parent, const char *name)
{
	const size_t num = parent->children.num;
	for (size_t i = 0; i < num; i++) {
		profile_entry *child = &parent->children.array[i];
		if (child->name == name)
			return child;
	}

	return init_entry(da_push_back_new(parent->children), name);
}

static void merge_call(profile_entry *entry, profile_call *call, profile_call *prev_call)
{
	const size_t num = call->children.num;
	for (size_t i = 0; i < num; i++) {
		profile_call *child = &call->children.array[i];
		merge_call(get_child(entry, child->name), child, NULL);
	}

	if (entry->expected_time_between_calls != 0 && prev_call) {
		migrate_old_entries(&entry->times_between_calls, true);
		uint64_t usec = diff_ns_to_usec(prev_call->start_time, call->start_time);
		add_hashmap_entry(&entry->times_between_calls, usec, 1);
	}

	migrate_old_entries(&entry->times, true);
	uint64_t usec = diff_ns_to_usec(call->start_time, call->end_time);
	add_hashmap_entry(&entry->times, usec, 1);

#ifdef TRACK_OVERHEAD
	migrate_old_entries(&entry->overhead, true);
	usec = diff_ns_to_usec(call->overhead_start, call->start_time);
	usec += diff_ns_to_usec(call->end_time, call->overhead_end);
	add_hashmap_entry(&entry->overhead, usec, 1);
#endif
}

static bool enabled = false;
static pthread_mutex_t root_mutex = PTHREAD_MUTEX_INITIALIZER;
static DARRAY(profile_root_entry) root_entries;

static THREAD_LOCAL profile_call *thread_context = NULL;
static THREAD_LOCAL bool thread_enabled = true;

void profiler_start(void)
{
	pthread_mutex_lock(&root_mutex);
	enabled = true;
	pthread_mutex_unlock(&root_mutex);
}

void profiler_stop(void)
{
	pthread_mutex_lock(&root_mutex);
	enabled = false;
	pthread_mutex_unlock(&root_mutex);
}

void profile_reenable_thread(void)
{
	if (thread_enabled)
		return;

	pthread_mutex_lock(&root_mutex);
	thread_enabled = enabled;
	pthread_mutex_unlock(&root_mutex);
}

static bool lock_root(void)
{
	pthread_mutex_lock(&root_mutex);
	if (!enabled) {
		pthread_mutex_unlock(&root_mutex);
		thread_enabled = false;
		return false;
	}

	return true;
}

static profile_root_entry *get_root_entry(const char *name)
{
	profile_root_entry *r_entry = NULL;

	for (size_t i = 0; i < root_entries.num; i++) {
		if (root_entries.array[i].name == name) {
			r_entry = &root_entries.array[i];
			break;
		}
	}

	if (!r_entry) {
		r_entry = da_push_back_new(root_entries);
		r_entry->mutex = bmalloc(sizeof(pthread_mutex_t));
		pthread_mutex_init(r_entry->mutex, NULL);

		r_entry->name = name;
		r_entry->entry = bzalloc(sizeof(profile_entry));
		init_entry(r_entry->entry, name);
	}

	return r_entry;
}

void profile_register_root(const char *name, uint64_t expected_time_between_calls)
{
	if (!lock_root())
		return;

	get_root_entry(name)->entry->expected_time_between_calls = (expected_time_between_calls + 500) / 1000;
	pthread_mutex_unlock(&root_mutex);
}

static void free_call_context(profile_call *context);

static void merge_context(profile_call *context)
{
	pthread_mutex_t *mutex = NULL;
	profile_entry *entry = NULL;
	profile_call *prev_call = NULL;

	if (!lock_root()) {
		free_call_context(context);
		return;
	}

	profile_root_entry *r_entry = get_root_entry(context->name);

	mutex = r_entry->mutex;
	entry = r_entry->entry;
	prev_call = r_entry->prev_call;

	r_entry->prev_call = context;

	pthread_mutex_lock(mutex);
	pthread_mutex_unlock(&root_mutex);

	merge_call(entry, context, prev_call);

	pthread_mutex_unlock(mutex);

	free_call_context(prev_call);
}

void profile_start(const char *name)
{
	if (!thread_enabled)
		return;

	profile_call new_call = {
		.name = name,
#ifdef TRACK_OVERHEAD
		.overhead_start = os_gettime_ns(),
#endif
		.parent = thread_context,
	};

	profile_call *call = NULL;

	if (new_call.parent) {
		size_t idx = da_push_back(new_call.parent->children, &new_call);
		call = &new_call.parent->children.array[idx];
	} else {
		call = bmalloc(sizeof(profile_call));
		memcpy(call, &new_call, sizeof(profile_call));
	}

	thread_context = call;
	call->start_time = os_gettime_ns();
}

void profile_end(const char *name)
{
	uint64_t end = os_gettime_ns();
	if (!thread_enabled)
		return;

	profile_call *call = thread_context;
	if (!call) {
		blog(LOG_ERROR, "Called profile end with no active profile");
		return;
	}

	if (!call->name)
		call->name = name;

	if (call->name != name) {
		blog(LOG_ERROR,
		     "Called profile end with mismatching name: "
		     "start(\"%s\"[%p]) <-> end(\"%s\"[%p])",
		     call->name, call->name, name, name);

		profile_call *parent = call->parent;
		while (parent && parent->parent && parent->name != name)
			parent = parent->parent;

		if (!parent || parent->name != name)
			return;

		while (call->name != name) {
			profile_end(call->name);
			call = call->parent;
		}
	}

	thread_context = call->parent;

	call->end_time = end;
#ifdef TRACK_OVERHEAD
	call->overhead_end = os_gettime_ns();
#endif

	if (call->parent)
		return;

	merge_context(call);
}

static int profiler_time_entry_compare(const void *first, const void *second)
{
	int64_t diff = (int64_t)(((const profiler_time_entry *)second)->time_delta -
				 ((const profiler_time_entry *)first)->time_delta);
	return diff < 0 ? -1 : (diff > 0 ? 1 : 0);
}

static uint64_t copy_map_to_array(profile_times_table *map, profiler_time_entries_t *entry_buffer, uint64_t *min_,
				  uint64_t *max_)
{
	migrate_old_entries(map, false);

	da_reserve(*entry_buffer, map->occupied);
	da_resize(*entry_buffer, 0);

	uint64_t min__ = ~(uint64_t)0;
	uint64_t max__ = 0;

	uint64_t calls = 0;
	for (size_t i = 0; i < map->size; i++) {
		if (!map->entries[i].probes)
			continue;

		profiler_time_entry *entry = &map->entries[i].entry;

		da_push_back(*entry_buffer, entry);

		calls += entry->count;
		min__ = (min__ < entry->time_delta) ? min__ : entry->time_delta;
		max__ = (max__ > entry->time_delta) ? max__ : entry->time_delta;
	}

	if (min_)
		*min_ = min__;
	if (max_)
		*max_ = max__;

	return calls;
}

static void free_call_children(profile_call *call)
{
	if (!call)
		return;

	const size_t num = call->children.num;
	for (size_t i = 0; i < num; i++)
		free_call_children(&call->children.array[i]);

	da_free(call->children);
}

static void free_call_context(profile_call *context)
{
	free_call_children(context);
	bfree(context);
}

static void free_hashmap(profile_times_table *map)
{
	map->size = 0;
	bfree(map->entries);
	map->entries = NULL;
	bfree(map->old_entries);
	map->old_entries = NULL;
}

static void free_profile_entry(profile_entry *entry)
{
	for (size_t i = 0; i < entry->children.num; i++)
		free_profile_entry(&entry->children.array[i]);

	free_hashmap(&entry->times);
#ifdef TRACK_OVERHEAD
	free_hashmap(&entry->overhead);
#endif
	free_hashmap(&entry->times_between_calls);
	da_free(entry->children);
}

void profiler_free(void)
{
	DARRAY(profile_root_entry) old_root_entries = {0};

	pthread_mutex_lock(&root_mutex);
	enabled = false;
	da_move(old_root_entries, root_entries);
	pthread_mutex_unlock(&root_mutex);

	for (size_t i = 0; i < old_root_entries.num; i++) {
		profile_root_entry *entry = &old_root_entries.array[i];

		pthread_mutex_lock(entry->mutex);
		pthread_mutex_unlock(entry->mutex);

		pthread_mutex_destroy(entry->mutex);
		bfree(entry->mutex);
		entry->mutex = NULL;

		free_call_context(entry->prev_call);

		free_profile_entry(entry->entry);
		bfree(entry->entry);
	}

	da_free(old_root_entries);

	pthread_mutex_destroy(&root_mutex);
}

/* ------------------------------------------------------------------------- */
/* Profiler name storage */

struct profiler_name_store {
	pthread_mutex_t mutex;
	DARRAY(char *) names;
};

profiler_name_store_t *profiler_name_store_create(void)
{
	profiler_name_store_t *store = bzalloc(sizeof(profiler_name_store_t));

	if (pthread_mutex_init(&store->mutex, NULL))
		goto error;

	return store;

error:
	bfree(store);
	return NULL;
}

void profiler_name_store_free(profiler_name_store_t *store)
{
	if (!store)
		return;

	for (size_t i = 0; i < store->names.num; i++)
		bfree(store->names.array[i]);

	da_free(store->names);

	pthread_mutex_destroy(&store->mutex);

	bfree(store);
}

const char *profile_store_name(profiler_name_store_t *store, const char *format, ...)
{
	va_list args;
	va_start(args, format);

	struct dstr str = {0};
	dstr_vprintf(&str, format, args);

	va_end(args);

	const char *result = NULL;

	pthread_mutex_lock(&store->mutex);
	size_t idx = da_push_back(store->names, &str.array);
	result = store->names.array[idx];
	pthread_mutex_unlock(&store->mutex);

	return result;
}

/* ------------------------------------------------------------------------- */
/* Profiler data access */

static void add_entry_to_snapshot(profile_entry *entry, profiler_snapshot_entry_t *s_entry)
{
	s_entry->name = entry->name;

	s_entry->overall_count =
		copy_map_to_array(&entry->times, &s_entry->times, &s_entry->min_time, &s_entry->max_time);

	if ((s_entry->expected_time_between_calls = entry->expected_time_between_calls))
		s_entry->overall_between_calls_count =
			copy_map_to_array(&entry->times_between_calls, &s_entry->times_between_calls,
					  &s_entry->min_time_between_calls, &s_entry->max_time_between_calls);

	da_reserve(s_entry->children, entry->children.num);
	for (size_t i = 0; i < entry->children.num; i++)
		add_entry_to_snapshot(&entry->children.array[i], da_push_back_new(s_entry->children));
}

static void sort_snapshot_entry(profiler_snapshot_entry_t *entry)
{
	qsort(entry->times.array, entry->times.num, sizeof(profiler_time_entry), profiler_time_entry_compare);

	if (entry->expected_time_between_calls)
		qsort(entry->times_between_calls.array, entry->times_between_calls.num, sizeof(profiler_time_entry),
		      profiler_time_entry_compare);

	for (size_t i = 0; i < entry->children.num; i++)
		sort_snapshot_entry(&entry->children.array[i]);
}

profiler_snapshot_t *profile_snapshot_create(void)
{
	profiler_snapshot_t *snap = bzalloc(sizeof(profiler_snapshot_t));

	pthread_mutex_lock(&root_mutex);
	da_reserve(snap->roots, root_entries.num);
	for (size_t i = 0; i < root_entries.num; i++) {
		pthread_mutex_lock(root_entries.array[i].mutex);
		add_entry_to_snapshot(root_entries.array[i].entry, da_push_back_new(snap->roots));
		pthread_mutex_unlock(root_entries.array[i].mutex);
	}
	pthread_mutex_unlock(&root_mutex);

	for (size_t i = 0; i < snap->roots.num; i++)
		sort_snapshot_entry(&snap->roots.array[i]);

	return snap;
}
