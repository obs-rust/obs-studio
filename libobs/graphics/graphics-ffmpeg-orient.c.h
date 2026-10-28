#include "libobs/build/libobs-ported-graphics-ffmpeg-orient.pch.h"

static inline int matrix_sign(int32_t v)
{
	return (v > 0) - (v < 0);
}

int gs_ffmpeg_image_matrix_orientation(int32_t a, int32_t b, int32_t c, int32_t d)
{
	/* The display matrix FFmpeg derives from each EXIF orientation, as
	 * the signs of its 2x2 part: {a, b, c, d} = {m[0], m[1], m[3], m[4]}. */
	static const int8_t orientations[8][4] = {
		{1, 0, 0, 1}, {-1, 0, 0, 1}, {-1, 0, 0, -1}, {1, 0, 0, -1},
		{0, 1, 1, 0}, {0, 1, -1, 0}, {0, -1, -1, 0}, {0, -1, 1, 0},
	};
	const int sign[4] = {matrix_sign(a), matrix_sign(b), matrix_sign(c), matrix_sign(d)};

	for (int i = 0; i < 8; i++) {
		if (sign[0] == orientations[i][0] && sign[1] == orientations[i][1] && sign[2] == orientations[i][2] &&
		    sign[3] == orientations[i][3])
			return i + 1;
	}

	return 0;
}

bool gs_ffmpeg_image_orient_needed(int orient)
{
	return !(orient == 0 || orient == 1);
}

bool gs_ffmpeg_image_orient_swaps(int orient)
{
	return orient >= 5 && orient < 9;
}

static inline size_t get_dst_position(const size_t w, const size_t h, const size_t x, const size_t y, int orient)
{
	size_t res_x = 0;
	size_t res_y = 0;

	if (orient == 2) {
		/*
		 * Orientation 2: Flip X
		 *
		 * 888888        888888
		 *     88   ->   88
		 *   8888   ->   8888
		 *     88   ->   88
		 *     88        88
		 *
		 * (0, 0) -> (w, 0)
		 * (0, h) -> (w, h)
		 * (w, 0) -> (0, 0)
		 * (w, h) -> (0, h)
		 *
		 * (w - x, y)
		 */

		res_x = w - 1 - x;
		res_y = y;

	} else if (orient == 3) {
		/*
		 * Orientation 3: 180 degree
		 *
		 *     88        888888
		 *     88   ->   88
		 *   8888   ->   8888
		 *     88   ->   88
		 * 888888        88
		 *
		 * (0, 0) -> (w, h)
		 * (0, h) -> (w, 0)
		 * (w, 0) -> (0, h)
		 * (w, h) -> (0, 0)
		 *
		 * (w - x, h - y)
		 */

		res_x = w - 1 - x;
		res_y = h - 1 - y;

	} else if (orient == 4) {
		/*
		 * Orientation 4: Flip Y
		 *
		 * 88            888888
		 * 88       ->   88
		 * 8888     ->   8888
		 * 88       ->   88
		 * 888888        88
		 *
		 * (0, 0) -> (0, h)
		 * (0, h) -> (0, 0)
		 * (w, 0) -> (w, h)
		 * (w, h) -> (w, 0)
		 *
		 * (x, h - y)
		 */

		res_x = x;
		res_y = h - 1 - y;

	} else if (orient == 5) {
		/*
		 * Orientation 5: Flip Y + 90 degree CW
		 *
		 * 8888888888        888888
		 * 88  88       ->   88
		 * 88           ->   8888
		 *              ->   88
		 *                   88
		 *
		 * (0, 0) -> (0, 0)
		 * (0, h) -> (w, 0)
		 * (w, 0) -> (0, h)
		 * (w, h) -> (w, h)
		 *
		 * (y, x)
		 */

		res_x = y;
		res_y = x;

	} else if (orient == 6) {
		/*
		 * Orientation 6: 90 degree CW
		 *
		 * 88                888888
		 * 88  88       ->   88
		 * 8888888888   ->   8888
		 *              ->   88
		 *                   88
		 *
		 * (0, 0) -> (w, 0)
		 * (0, h) -> (0, 0)
		 * (w, 0) -> (w, h)
		 * (w, h) -> (0, h)
		 *
		 * (w - y, x)
		 */

		res_x = w - 1 - y;
		res_y = x;

	} else if (orient == 7) {
		/*
		 * Orientation 7: Flip Y + 90 degree CCW
		 *
		 *         88        888888
		 *     88  88   ->   88
		 * 8888888888   ->   8888
		 *              ->   88
		 *                   88
		 *
		 * (0, 0) -> (w, h)
		 * (0, h) -> (0, h)
		 * (w, 0) -> (w, 0)
		 * (w, h) -> (0, 0)
		 *
		 * (w - y, h - x)
		 */

		res_x = w - 1 - y;
		res_y = h - 1 - x;

	} else if (orient == 8) {
		/*
		 * Orientation 8: 90 degree CCW
		 *
		 * 8888888888        888888
		 *     88  88   ->   88
		 *         88   ->   8888
		 *              ->   88
		 *                   88
		 *
		 * (0, 0) -> (0, h)
		 * (0, h) -> (w, h)
		 * (w, 0) -> (0, 0)
		 * (w, h) -> (w, 0)
		 *
		 * (y, h - x)
		 */

		res_x = y;
		res_y = h - 1 - x;
	}

	return (res_x + res_y * w) * 4;
}

#define TILE_SIZE 16
#define MIN(a, b) (((a) < (b)) ? (a) : (b))

void gs_ffmpeg_image_orient_copy(uint8_t *dst, const uint8_t *src, size_t cx, size_t cy, int orient)
{
	const size_t sx = cx;
	const size_t sy = cy;
	/* The turned image's size, as ffmpeg_image_orient() set it before. */
	const size_t w = gs_ffmpeg_image_orient_swaps(orient) ? sy : sx;
	const size_t h = gs_ffmpeg_image_orient_swaps(orient) ? sx : sy;

	size_t off_dst;
	size_t off_src = 0;
	for (size_t y0 = 0; y0 < sy; y0 += TILE_SIZE) {
		for (size_t x0 = 0; x0 < sx; x0 += TILE_SIZE) {
			size_t lim_x = MIN((size_t)sx, x0 + TILE_SIZE);
			size_t lim_y = MIN((size_t)sy, y0 + TILE_SIZE);

			for (size_t y = y0; y < lim_y; y++) {
				for (size_t x = x0; x < lim_x; x++) {
					off_src = (x + y * sx) * 4;

					off_dst = get_dst_position(w, h, x, y, orient);

					memcpy(dst + off_dst, src + off_src, 4);
				}
			}
		}
	}
}

#undef MIN
#undef TILE_SIZE

bool gs_ffmpeg_image_too_large(uint32_t max_size, int cx, int cy)
{
	return !(!max_size || cx <= 0 || cy <= 0 || ((uint32_t)cx <= max_size && (uint32_t)cy <= max_size));
}
