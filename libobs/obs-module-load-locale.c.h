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

lookup_t *obs_module_load_locale(obs_module_t *module, const char *default_locale, const char *locale)
{
	char *name = NULL;
	lookup_t *lookup = NULL;

	if (!module || !default_locale || !locale) {
		blog(LOG_WARNING, "obs_module_load_locale: Invalid parameters");
		return NULL;
	}

	name = obs_module_locale_default_file(default_locale);

	char *file = obs_find_module_file(module, name);
	if (file)
		lookup = text_lookup_create(file);

	bfree(file);

	if (!lookup) {
		blog(LOG_WARNING, "Failed to load '%s' text for module: '%s'", default_locale, module->file);
		goto cleanup;
	}

	if (obs_module_locale_is_default(locale, default_locale))
		goto cleanup;

	bfree(name);
	name = obs_module_locale_file(locale);

	file = obs_find_module_file(module, name);

	if (!text_lookup_add(lookup, file))
		blog(LOG_WARNING, "Failed to load '%s' text for module: '%s'", locale, module->file);

	bfree(file);
cleanup:
	bfree(name);
	return lookup;
}
