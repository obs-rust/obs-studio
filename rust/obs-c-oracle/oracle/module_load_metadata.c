/* The original loader, with actual private layouts and OBS data APIs. */
#define obs_module_load_metadata oracle_obs_module_load_metadata
#include "libobs/obs-module-load-metadata.c.h"
#undef obs_module_load_metadata
#include "module-load-metadata-fields.inc"
#include <stdlib.h>
#include <stddef.h>

void *oracle_module_load_metadata_new(char *path);
void *oracle_module_load_metadata_new(char *path)
{
	struct obs_module *module = calloc(1, sizeof(*module));
	if (!module)
		abort();
	module->data_path = path;
	return module;
}
void oracle_module_load_metadata_free(void *p);
void oracle_module_load_metadata_free(void *p)
{
	struct obs_module *module = p;
	free_module_metadata(module->metadata);
	bfree(module->metadata);
	free(module);
}
void oracle_module_load_metadata_layout(size_t *values);
void oracle_module_load_metadata_layout(size_t *values)
{
	const size_t layout[] = {sizeof(struct obs_module_metadata),
				 _Alignof(struct obs_module_metadata),
				 offsetof(struct obs_module_metadata, display_name),
				 offsetof(struct obs_module_metadata, version),
				 offsetof(struct obs_module_metadata, id),
				 offsetof(struct obs_module_metadata, os_arch),
				 offsetof(struct obs_module_metadata, description),
				 offsetof(struct obs_module_metadata, long_description),
				 offsetof(struct obs_module_metadata, has_icon),
				 offsetof(struct obs_module_metadata, has_banner),
				 offsetof(struct obs_module_metadata, repository_url),
				 offsetof(struct obs_module_metadata, support_url),
				 offsetof(struct obs_module_metadata, website_url),
				 offsetof(struct obs_module_metadata, name)};
	memcpy(values, layout, sizeof(layout));
}
