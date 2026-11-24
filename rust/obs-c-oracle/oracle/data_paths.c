/* Original obs_add_data_path() and obs_remove_data_path() (#5397) and the
 * production list and adapters the Rust shims go through
 * (obs-data-paths-fields.c.h, defined once, here). Both sides change the
 * same list, as in the reference library, so each run starts from an empty
 * list and leaves one. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_add_data_path oracle_obs_add_data_path
#define obs_remove_data_path oracle_obs_remove_data_path
#include "libobs/build/libobs.pch.h"
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif
#include "data-paths.inc"
#undef obs_remove_data_path
#undef obs_add_data_path
#include "data_paths_fields.inc"

typedef void (*data_paths_add)(const char *);
typedef bool (*data_paths_remove)(const char *);

static void data_paths_clear(void)
{
	while (obs_data_paths_count())
		obs_data_paths_erase(obs_data_paths_count() - 1);
}

/* Runs ops[i] (0 add, 1 remove) with paths[i] (NULL allowed) through add and
 * remove, or through the originals when they are NULL, from an empty list.
 * Each add gets a fresh copy of its path that is scribbled over and freed
 * right after, so an entry that kept the caller's pointer shows. results[i]
 * is a remove's answer (2 for an add). The list left is written to list as
 * one line per entry, "<null>" for a NULL entry, then emptied. False when
 * list is too small. */
bool oracle_data_paths_run(data_paths_add add, data_paths_remove remove, const uint8_t *ops, const char *const *paths,
			   size_t count, uint8_t *results, char *list, size_t size);
bool oracle_data_paths_run(data_paths_add add, data_paths_remove remove, const uint8_t *ops, const char *const *paths,
			   size_t count, uint8_t *results, char *list, size_t size)
{
	if (!add)
		add = oracle_obs_add_data_path;
	if (!remove)
		remove = oracle_obs_remove_data_path;
	data_paths_clear();
	for (size_t i = 0; i < count; i++) {
		if (ops[i] == 0) {
			char *copy = paths[i] ? bstrdup(paths[i]) : NULL;
			add(copy);
			if (copy) {
				memset(copy, 'X', strlen(copy));
				bfree(copy);
			}
			results[i] = 2;
		} else {
			results[i] = remove(paths[i]) ? 1 : 0;
		}
	}
	size_t used = 0;
	bool fits = size > 0;
	if (fits)
		list[0] = 0;
	for (size_t i = 0; fits && i < obs_data_paths_count(); i++) {
		const char *entry = obs_data_paths_entry(i);
		int n = snprintf(list + used, size - used, "%s\n", entry ? entry : "<null>");
		fits = n >= 0 && (size_t)n < size - used;
		if (fits)
			used += (size_t)n;
	}
	data_paths_clear();
	return fits;
}
