#pragma once

#include <stdbool.h>
#include <stddef.h>

#include <util/darray.h>
#include <util/platform.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The order an Image Slide Show plays the images of a folder in its file
 * list. Files added one by one keep their place in the list. */
enum slideshow_folder_order {
	/* by name, digit runs compared as numbers (slide2 before slide10),
	 * otherwise case-insensitive */
	SLIDESHOW_FOLDER_ORDER_NATURAL,
	/* by name, case-insensitive ordinal (the order NTFS lists a folder in) */
	SLIDESHOW_FOLDER_ORDER_NAME,
	/* unsorted, as the filesystem lists the folder */
	SLIDESHOW_FOLDER_ORDER_FILESYSTEM,
};

/* setting key and values */
#define SLIDESHOW_S_FOLDER_ORDER "folder_order"
#define SLIDESHOW_S_FOLDER_ORDER_NATURAL "natural"
#define SLIDESHOW_S_FOLDER_ORDER_NAME "name"
#define SLIDESHOW_S_FOLDER_ORDER_FILESYSTEM "filesystem"

/* An unknown or empty value means filesystem order. */
enum slideshow_folder_order slideshow_folder_order_from_string(const char *value);

/* strcmp-style comparisons (<0, 0, >0) of two file names */
int slideshow_name_cmp_natural(const char *a, const char *b);
int slideshow_name_cmp(const char *a, const char *b);

typedef DARRAY(char *) slideshow_paths_t;

/* Appends "<dir_path>/<name>" for every image file (valid_extension(), no
 * subfolders) of the open folder dir to paths, in the given order. */
void slideshow_folder_files(os_dir_t *dir, const char *dir_path, enum slideshow_folder_order order,
			    slideshow_paths_t *paths);
void slideshow_paths_free(slideshow_paths_t *paths);

/* obs-slideshow.c.h: whether a file extension is an image the slideshows
 * play */
bool valid_extension(const char *ext);

#ifdef __cplusplus
}
#endif
