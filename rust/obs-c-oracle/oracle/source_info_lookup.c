/* The production registry adapter over a per-thread stand-in core, shared by
 * every oracle whose code looks source types up: the Rust get_source_info
 * reads it through the adapter, so each test thread drives its own registry.
 * With no core installed it is the icon oracle's read-only registry. */
#include "libobs/build/libobs.pch.h"

const struct obs_core *oracle_source_icon_core(void);

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *source_info_lookup_installed;
static __declspec(thread) struct obs_core source_info_lookup_storage;
#else
static _Thread_local struct obs_core *source_info_lookup_installed;
static _Thread_local struct obs_core source_info_lookup_storage;
#endif

static const struct obs_core *source_info_lookup_current_core(void)
{
	return source_info_lookup_installed ? source_info_lookup_installed : oracle_source_icon_core();
}

size_t source_info_lookup_real_count(void);
struct obs_source_info *source_info_lookup_real_entry(size_t index);
const char *source_info_lookup_real_id(const struct obs_source_info *info);
#define obs_core_texture_activity_core source_info_lookup_current_core
#define obs_source_info_lookup_count source_info_lookup_real_count
#define obs_source_info_lookup_entry source_info_lookup_real_entry
#define obs_source_info_lookup_id source_info_lookup_real_id
#include "source_info_lookup_fields.inc"
#undef obs_source_info_lookup_id
#undef obs_source_info_lookup_entry
#undef obs_source_info_lookup_count
#undef obs_core_texture_activity_core

/* Install `core` (NULL: the default registry) on this thread; returns the
 * previous one. */
struct obs_core *oracle_source_registry_exchange_core(struct obs_core *core);
struct obs_core *oracle_source_registry_exchange_core(struct obs_core *core)
{
	struct obs_core *previous = source_info_lookup_installed;
	source_info_lookup_installed = core;
	return previous;
}

/* Lookups are seen as they start, before they read the registry. A lookup
 * reads the count first, and again after each entry's ID, so a count read
 * that does not follow an ID read starts one. A NULL asked ID reads nothing
 * and is not seen; nor is the asked ID itself, which only Rust reads. */
typedef void (*source_info_lookup_observer)(void *ctx);
enum source_info_lookup_read { LOOKUP_READ_NONE, LOOKUP_READ_COUNT, LOOKUP_READ_ENTRY, LOOKUP_READ_ID };
#if defined(_MSC_VER)
static __declspec(thread) source_info_lookup_observer source_info_lookup_on_start;
static __declspec(thread) void *source_info_lookup_on_start_ctx;
static __declspec(thread) enum source_info_lookup_read source_info_lookup_last;
static __declspec(thread) size_t source_info_lookup_started;
#else
static _Thread_local source_info_lookup_observer source_info_lookup_on_start;
static _Thread_local void *source_info_lookup_on_start_ctx;
static _Thread_local enum source_info_lookup_read source_info_lookup_last;
static _Thread_local size_t source_info_lookup_started;
#endif

/* Call `observer` as each lookup on this thread starts (NULL: none). */
void oracle_source_info_lookup_observe(source_info_lookup_observer observer, void *ctx);
void oracle_source_info_lookup_observe(source_info_lookup_observer observer, void *ctx)
{
	source_info_lookup_on_start = observer;
	source_info_lookup_on_start_ctx = ctx;
	source_info_lookup_last = LOOKUP_READ_NONE;
}

/* How many lookups have started on this thread. */
size_t oracle_source_registry_lookups(void);
size_t oracle_source_registry_lookups(void)
{
	return source_info_lookup_started;
}

size_t obs_source_info_lookup_count(void)
{
	if (source_info_lookup_last != LOOKUP_READ_ID) {
		++source_info_lookup_started;
		if (source_info_lookup_on_start)
			source_info_lookup_on_start(source_info_lookup_on_start_ctx);
	}
	source_info_lookup_last = LOOKUP_READ_COUNT;
	return source_info_lookup_real_count();
}

struct obs_source_info *obs_source_info_lookup_entry(size_t index)
{
	source_info_lookup_last = LOOKUP_READ_ENTRY;
	return source_info_lookup_real_entry(index);
}

const char *obs_source_info_lookup_id(const struct obs_source_info *info)
{
	source_info_lookup_last = LOOKUP_READ_ID;
	return source_info_lookup_real_id(info);
}

/* Make `types` (borrowed, not copied) this thread's registered source types. */
void oracle_source_info_lookup_use(struct obs_source_info *types, size_t num);
void oracle_source_info_lookup_use(struct obs_source_info *types, size_t num)
{
	source_info_lookup_storage.source_types.array = num ? types : NULL;
	source_info_lookup_storage.source_types.num = num;
	source_info_lookup_storage.source_types.capacity = num;
	source_info_lookup_installed = &source_info_lookup_storage;
}

/* The original lookup, renamed, over the same thread-local core. */
struct obs_source_info *oracle_get_source_info(const char *id);
#define obs_core_texture_activity_core source_info_lookup_current_core
#define get_source_info oracle_get_source_info
#include "libobs/obs-source-info-lookup.c.h"
#undef get_source_info
#undef obs_core_texture_activity_core

#define SOURCE_INFO_LOOKUP_MAX 32

/* Register `ids` (none NULL) on this thread, then look `id` up with the
 * production get_source_info (`lookup`) and the original C. Writes each
 * found index (or -1) and returns whether the registry is unchanged and both
 * found the same entry address. `id` may be NULL only when `num` is 0: the
 * original crashes on it otherwise. */
typedef struct obs_source_info *(*source_info_lookup_fn)(const char *);
bool oracle_source_info_lookup_check(const char *const *ids, size_t num, const char *id, source_info_lookup_fn lookup,
				     ptrdiff_t *rust_index, ptrdiff_t *c_index);
bool oracle_source_info_lookup_check(const char *const *ids, size_t num, const char *id, source_info_lookup_fn lookup,
				     ptrdiff_t *rust_index, ptrdiff_t *c_index)
{
	struct obs_source_info types[SOURCE_INFO_LOOKUP_MAX] = {0};
	struct obs_source_info before[SOURCE_INFO_LOOKUP_MAX];
	if (num > SOURCE_INFO_LOOKUP_MAX || (id == NULL && num != 0))
		return false;
	for (size_t i = 0; i < num; i++) {
		if (!ids[i])
			return false;
		types[i].id = ids[i];
		types[i].version = (uint32_t)i;
	}
	memcpy(before, types, sizeof(types));
	oracle_source_info_lookup_use(types, num);

	struct obs_source_info *rust = lookup(id);
	struct obs_source_info *c = oracle_get_source_info(id);
	*rust_index = rust ? rust - types : -1;
	*c_index = c ? c - types : -1;
	bool same = rust == c && memcmp(before, types, sizeof(types)) == 0;
	oracle_source_info_lookup_use(NULL, 0);
	return same;
}

/* With nothing registered the original never reads the asked ID (strcmp is
 * inside the loop): both sides take an unreadable pointer and find nothing. */
bool oracle_source_info_lookup_unread_id(source_info_lookup_fn lookup);
bool oracle_source_info_lookup_unread_id(source_info_lookup_fn lookup)
{
	const char *unreadable = (const char *)(uintptr_t)1;
	oracle_source_info_lookup_use(NULL, 0);
	return lookup(unreadable) == NULL && oracle_get_source_info(unreadable) == NULL;
}

/* The intentional differences, production side only: with a registered NULL
 * ID first, a NULL asked ID and the empty ID find nothing, and "a" finds the
 * second entry. The original is not called: it would crash in strcmp. */
bool oracle_source_info_lookup_null_ids(source_info_lookup_fn lookup);
bool oracle_source_info_lookup_null_ids(source_info_lookup_fn lookup)
{
	struct obs_source_info types[2] = {{.id = NULL}, {.id = "a"}};
	oracle_source_info_lookup_use(types, 2);
	bool ok = lookup(NULL) == NULL && lookup("") == NULL && lookup("a") == &types[1];
	oracle_source_info_lookup_use(NULL, 0);
	return ok;
}
