#include "frontend/build/frontend-utility-missing-source-types.pch.h"

void missing_source_types_add(struct missing_source_types *list, const char *id, bool registered)
{
	(void)list;
	(void)id;
	(void)registered;
}

enum missing_source_hint missing_source_types_hint(const char *id)
{
	(void)id;
	return MISSING_SOURCE_HINT_PLUGIN;
}
