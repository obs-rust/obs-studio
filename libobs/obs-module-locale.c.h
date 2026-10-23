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

#include "libobs/build/libobs-ported-module-locale.pch.h"

/* Module locale files, moved from obs_module_load_locale() in obs-module.c:
 * the names of the default locale's file and of the locale's file in a
 * module's data path, and whether the locale is the default one. Pure: no
 * libobs state. obs-module.c keeps the module, the lookups
 * (obs_find_module_file(), text_lookup_create(), text_lookup_add()) and the
 * logging, and passes the locales in. Declared in obs-internal.h. Ported to
 * rust/obs-core (module_locale): the hybrid libobs drops this file. */

char *obs_module_locale_default_file(const char *default_locale)
{
	struct dstr str = {0};

	dstr_copy(&str, "locale/");
	dstr_cat(&str, default_locale);
	dstr_cat(&str, ".ini");

	return str.array;
}

bool obs_module_locale_is_default(const char *locale, const char *default_locale)
{
	return astrcmpi(locale, default_locale) == 0;
}

char *obs_module_locale_file(const char *locale)
{
	struct dstr str = {0};

	dstr_copy(&str, "/locale/");
	dstr_cat(&str, locale);
	dstr_cat(&str, ".ini");

	return str.array;
}
