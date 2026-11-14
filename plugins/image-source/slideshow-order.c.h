#include "plugins/image-source/build/image-source-slideshow.pch.h"

/* RED stubs: every folder plays in filesystem order. */

enum slideshow_folder_order slideshow_folder_order_from_string(const char *value)
{
	UNUSED_PARAMETER(value);
	return SLIDESHOW_FOLDER_ORDER_FILESYSTEM;
}

int slideshow_name_cmp(const char *a, const char *b)
{
	return strcmp(a, b);
}

int slideshow_name_cmp_natural(const char *a, const char *b)
{
	return strcmp(a, b);
}

void slideshow_folder_files(os_dir_t *dir, const char *dir_path, enum slideshow_folder_order order,
			    slideshow_paths_t *paths)
{
	struct os_dirent *ent;

	UNUSED_PARAMETER(order);

	while ((ent = os_readdir(dir)) != NULL) {
		if (ent->directory || !valid_extension(os_get_path_extension(ent->d_name)))
			continue;

		struct dstr path = {0};
		dstr_printf(&path, "%s/%s", dir_path, ent->d_name);
		da_push_back(*paths, &path.array);
	}
}

void slideshow_paths_free(slideshow_paths_t *paths)
{
	for (size_t i = 0; i < paths->num; i++)
		bfree(paths->array[i]);
	da_free(*paths);
}
