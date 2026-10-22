/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

/* Context names, moved from obs.c: the name a context gets for a NULL or
 * empty name, and the name that makes a duplicate unique. Pure: no libobs
 * state. obs.c keeps the unnamed counter (obs->data.unnamed_index) and the
 * name tables, which it looks up for obs_context_name_deduplicate() through
 * exists(). Both return a bmalloc'd string (or NULL) that the caller owns.
 * Declared in obs-internal.h. */

/* ensures that names are never blank */
char *obs_context_name_dup(const char *name, bool private, long long *unnamed_index)
{
	if (private && !name)
		return NULL;

	if (!name || !*name) {
		struct dstr unnamed = {0};
		dstr_printf(&unnamed, "__unnamed%04lld", (*unnamed_index)++);

		return unnamed.array;
	} else {
		return bstrdup(name);
	}
}

char *obs_context_name_deduplicate(const char *name, obs_context_name_exists_t exists, void *param)
{
	bool taken = exists(param, name);
	if (!taken)
		return NULL;

	struct dstr new_name = {0};
	int suffix = 2;

	while (taken) {
		dstr_printf(&new_name, "%s %d", name, suffix++);
		taken = exists(param, new_name.array);
	}

	return new_name.array;
}
