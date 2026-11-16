/* Module manifest loading, extracted verbatim before its Rust port. */

#include "libobs/build/libobs.pch.h"

int obs_module_load_metadata(struct obs_module *mod);
int obs_module_load_metadata(struct obs_module *mod)
{
	struct obs_module_metadata *md = NULL;

	/* Check if the metadata file exists */
	char *path = obs_module_names_manifest_path(mod->data_path);

	if (os_file_exists(path)) {
		/* If we find a metadata file, allocate a new metadata. */
		md = bmalloc(sizeof(obs_module_metadata_t));
		obs_data_t *metadata = obs_data_create_from_json_file(path);

		md->display_name = bstrdup(obs_data_get_string(metadata, "display_name"));
		md->id = bstrdup(obs_data_get_string(metadata, "id"));
		md->version = bstrdup(obs_data_get_string(metadata, "version"));
		md->os_arch = bstrdup(obs_data_get_string(metadata, "os_arch"));
		md->name = bstrdup(obs_data_get_string(metadata, "name"));
		md->description = bstrdup(obs_data_get_string(metadata, "description"));
		md->long_description = bstrdup(obs_data_get_string(metadata, "long_description"));

		obs_data_t *urls = obs_data_get_obj(metadata, "urls");
		md->repository_url = bstrdup(obs_data_get_string(urls, "repository"));
		md->website_url = bstrdup(obs_data_get_string(urls, "website"));
		md->support_url = bstrdup(obs_data_get_string(urls, "support"));
		obs_data_release(urls);

		md->has_banner = obs_data_get_bool(metadata, "has_banner");
		md->has_icon = obs_data_get_bool(metadata, "has_icon");
		obs_data_release(metadata);
	}
	bfree(path);
	mod->metadata = md;
	return MODULE_SUCCESS;
}
