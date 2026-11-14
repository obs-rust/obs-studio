#include "plugins/image-source/build/image-source-slideshow.pch.h"

enum slideshow_folder_order slideshow_folder_order_from_string(const char *value)
{
	if (value && strcmp(value, SLIDESHOW_S_FOLDER_ORDER_NATURAL) == 0)
		return SLIDESHOW_FOLDER_ORDER_NATURAL;
	if (value && strcmp(value, SLIDESHOW_S_FOLDER_ORDER_NAME) == 0)
		return SLIDESHOW_FOLDER_ORDER_NAME;
	return SLIDESHOW_FOLDER_ORDER_FILESYSTEM;
}

/* ASCII letters compare upper-cased, as NTFS compares names (so '_' sorts
 * after the letters); every other byte, UTF-8 included, by its value. */
static inline int order_fold(unsigned char c)
{
	return (c >= 'a' && c <= 'z') ? c - 'a' + 'A' : c;
}

static inline bool order_is_digit(unsigned char c)
{
	return c >= '0' && c <= '9';
}

static inline int order_sign(int v)
{
	return (v > 0) - (v < 0);
}

int slideshow_name_cmp(const char *a, const char *b)
{
	const unsigned char *pa = (const unsigned char *)a;
	const unsigned char *pb = (const unsigned char *)b;

	for (;; pa++, pb++) {
		int ca = order_fold(*pa);
		int cb = order_fold(*pb);
		if (ca != cb)
			return ca < cb ? -1 : 1;
		if (!ca)
			break;
	}

	/* names that differ only in case: a fixed order all the same */
	return order_sign(strcmp(a, b));
}

/* Runs of digits compare by their value, of any length: leading zeros are
 * skipped, then the longer run is the larger number, then the first digit
 * that differs decides. Same approach as a_compare_number() in
 * obsproject/obs-studio#12577 (Dankirk). */
int slideshow_name_cmp_natural(const char *a, const char *b)
{
	const unsigned char *pa = (const unsigned char *)a;
	const unsigned char *pb = (const unsigned char *)b;
	int zeros = 0;

	while (*pa && *pb) {
		if (order_is_digit(*pa) && order_is_digit(*pb)) {
			const unsigned char *za = pa;
			const unsigned char *zb = pb;
			while (*pa == '0')
				pa++;
			while (*pb == '0')
				pb++;

			const unsigned char *da = pa;
			const unsigned char *db = pb;
			while (order_is_digit(*pa))
				pa++;
			while (order_is_digit(*pb))
				pb++;

			size_t la = (size_t)(pa - da);
			size_t lb = (size_t)(pb - db);
			if (la != lb)
				return la < lb ? -1 : 1;

			int r = memcmp(da, db, la);
			if (r)
				return order_sign(r);

			/* same value: remember which had fewer leading zeros,
			 * in case nothing after it differs */
			if (!zeros && (da - za) != (db - zb))
				zeros = (da - za) < (db - zb) ? -1 : 1;
			continue;
		}

		int ca = order_fold(*pa);
		int cb = order_fold(*pb);
		if (ca != cb)
			return ca < cb ? -1 : 1;
		pa++;
		pb++;
	}

	if (*pa || *pb)
		return *pa ? 1 : -1;
	if (zeros)
		return zeros;
	return slideshow_name_cmp(a, b);
}

static int order_cmp_natural(const void *a, const void *b)
{
	return slideshow_name_cmp_natural(*(char *const *)a, *(char *const *)b);
}

static int order_cmp_name(const void *a, const void *b)
{
	return slideshow_name_cmp(*(char *const *)a, *(char *const *)b);
}

void slideshow_folder_files(os_dir_t *dir, const char *dir_path, enum slideshow_folder_order order,
			    slideshow_paths_t *paths)
{
	size_t first = paths->num;
	struct os_dirent *ent;

	while ((ent = os_readdir(dir)) != NULL) {
		if (ent->directory || !valid_extension(os_get_path_extension(ent->d_name)))
			continue;

		struct dstr path = {0};
		dstr_printf(&path, "%s/%s", dir_path, ent->d_name);
		da_push_back(*paths, &path.array);
	}

	/* every path of this folder has the same "<dir_path>/" prefix, so
	 * comparing the paths compares the file names */
	size_t count = paths->num - first;
	if (count < 2 || order == SLIDESHOW_FOLDER_ORDER_FILESYSTEM)
		return;

	qsort(paths->array + first, count, sizeof(char *),
	      order == SLIDESHOW_FOLDER_ORDER_NATURAL ? order_cmp_natural : order_cmp_name);
}

void slideshow_paths_free(slideshow_paths_t *paths)
{
	for (size_t i = 0; i < paths->num; i++)
		bfree(paths->array[i]);
	da_free(*paths);
}

void slideshow_add_folder_order_property(obs_properties_t *ppts)
{
	obs_property_t *p = obs_properties_add_list(ppts, SLIDESHOW_S_FOLDER_ORDER,
						    obs_module_text("SlideShow.FolderOrder"), OBS_COMBO_TYPE_LIST,
						    OBS_COMBO_FORMAT_STRING);
	obs_property_list_add_string(p, obs_module_text("SlideShow.FolderOrder.Natural"),
				     SLIDESHOW_S_FOLDER_ORDER_NATURAL);
	obs_property_list_add_string(p, obs_module_text("SlideShow.FolderOrder.Name"), SLIDESHOW_S_FOLDER_ORDER_NAME);
	obs_property_list_add_string(p, obs_module_text("SlideShow.FolderOrder.Filesystem"),
				     SLIDESHOW_S_FOLDER_ORDER_FILESYSTEM);
	obs_property_set_long_description(p, obs_module_text("SlideShow.FolderOrder.ToolTip"));
}
