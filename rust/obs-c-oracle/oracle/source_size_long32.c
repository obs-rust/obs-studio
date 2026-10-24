/* Test-only oracle: the original libobs/obs-source-size.c.h, unmodified,
 * built with a 32-bit long, as MSVC builds it: the rotation's type is
 * remapped to int (32 bits on every target the oracle builds for) after the
 * standard headers, and the functions are renamed to oracle32_*. On Windows
 * this is the same build as oracle/source_size.c. It shows the route of the
 * values a 32-bit long holds on a host whose long is 64 bits. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

_Static_assert(sizeof(int) == 4, "the oracle's 32-bit long is an int");

/* Remapping a keyword is the point here; clang's -Wpedantic flags it. */
#if defined(__clang__)
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wkeyword-macro"
#endif
#define long int
#if defined(__clang__)
#pragma clang diagnostic pop
#endif
#define obs_source_size_route oracle32_obs_source_size_route
#define obs_source_size_entry oracle32_obs_source_size_entry

#include "obs-source-size.c.h"

#undef obs_source_size_entry
#undef obs_source_size_route

/* sizeof and the rotation's offset of the 32-bit node, then sizeof(long)
 * as this file sees it (4). */
size_t oracle32_source_size_layout(size_t which);

size_t oracle32_source_size_layout(size_t which)
{
	const size_t values[3] = {
		sizeof(struct obs_source_size_node),
		offsetof(struct obs_source_size_node, async_rotation),
		sizeof(long),
	};
	return which < 3 ? values[which] : 0;
}

#undef long
