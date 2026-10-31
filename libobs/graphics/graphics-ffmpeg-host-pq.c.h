#include "libobs/build/libobs-graphics-ffmpeg-host-pq.pch.h"

_Static_assert(sizeof(struct half) == 2, "struct half is one uint16_t");

static float pq_to_linear(float u)
{
	const float common = powf(u, 1.f / 78.84375f);
	/* windows.h's max(common - 0.8359375f, 0.f), expanded. */
	return powf(fabsf((((common - 0.8359375f) > (0.f)) ? (common - 0.8359375f) : (0.f)) /
			  (18.8515625f - 18.6875f * common)),
		    1.f / 0.1593017578f);
}

void gs_image_pq10_to_scrgb(uint8_t *dst, uint32_t dst_size, const uint8_t *src, uint32_t src_size)
{
	/* The caller sizes dst for every texel of src. */
	(void)dst_size;

	const uint8_t *src_cursor = src;
	const uint8_t *src_cursor_end = src_cursor + src_size;
	uint8_t *dst_cursor = dst;
	uint32_t rgb10;
	struct half rgba16[4];
	rgba16[3].u = 0x3c00;
	while (src_cursor < src_cursor_end) {
		memcpy(&rgb10, src_cursor, sizeof(rgb10));
		const float blue = (float)(rgb10 & 0x3ff) / 1023.f;
		const float green = (float)((rgb10 >> 10) & 0x3ff) / 1023.f;
		const float red = (float)((rgb10 >> 20) & 0x3ff) / 1023.f;
		const float red2020 = pq_to_linear(red);
		const float green2020 = pq_to_linear(green);
		const float blue2020 = pq_to_linear(blue);
		const float red709 = 1.6604910021084345f * red2020 - 0.58764113878854951f * green2020 -
				     0.072849863319884883f * blue2020;
		const float green709 = -0.12455047452159074f * red2020 + 1.1328998971259603f * green2020 -
				       0.0083494226043694768f * blue2020;
		const float blue709 = -0.018150763354905303f * red2020 - 0.10057889800800739f * green2020 +
				      1.1187296613629127f * blue2020;
		rgba16[0] = half_from_float(red709 * 125.f);
		rgba16[1] = half_from_float(green709 * 125.f);
		rgba16[2] = half_from_float(blue709 * 125.f);
		memcpy(dst_cursor, &rgba16, sizeof(rgba16));
		src_cursor += 4;
		dst_cursor += 8;
	}
}
