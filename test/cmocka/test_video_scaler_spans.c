/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Standalone: also runs without libobs or FFmpeg, including under ASan/UBSan. */
#include "../../libobs/media-io/video-scaler-overlap.h"
#include <stdio.h>
#include <stdlib.h>

#define CHECK(expr)                                                    \
	do {                                                           \
		if (!(expr)) {                                         \
			fprintf(stderr, "%d: %s\n", __LINE__, #expr);    \
			return EXIT_FAILURE;                           \
		}                                                      \
	} while (0)

int main(void)
{
	struct scaler_plane_span a, b;
	CHECK(scaler_plane_span_init(&a, 1024, 32, 4));
	CHECK(a.size == 128);
	CHECK(scaler_plane_span_init(&b, 1152, 32, 4));
	CHECK(!scaler_plane_spans_overlap(&a, &b));
	CHECK(!scaler_plane_spans_overlap(&b, &a));
	CHECK(scaler_plane_span_init(&b, 1151, 32, 4));
	CHECK(scaler_plane_spans_overlap(&a, &b));
	CHECK(scaler_plane_spans_overlap(&b, &a));
	CHECK(scaler_plane_spans_overlap(&a, &a));
	CHECK(!scaler_plane_span_init(&b, 0, 32, 4));
	CHECK(!scaler_plane_span_init(&b, 1024, 0, 4));
	CHECK(!scaler_plane_span_init(&b, 1024, 32, 0));
	CHECK(!scaler_plane_span_init(&b, 1024, UINT32_MAX, 4));
	CHECK(!scaler_plane_span_init(&b, UINTPTR_MAX - 31, 32, 1));
	CHECK(scaler_plane_span_init(&b, UINTPTR_MAX - 32, 32, 1));
#if SIZE_MAX == UINT32_MAX
	CHECK(!scaler_plane_span_init(&b, 1, INT_MAX, 3));
#endif

	/* Exhaust all small ranges against an independent end-address oracle. */
	for (uintptr_t x = 1; x <= 64; ++x) {
		for (uintptr_t y = 1; y <= 64; ++y) {
			for (uint32_t len = 1; len <= 16; ++len) {
				CHECK(scaler_plane_span_init(&a, x, len, 1));
				CHECK(scaler_plane_span_init(&b, y, 17 - len, 1));
				const bool expected = x < y + b.size && y < x + a.size;
				CHECK(scaler_plane_spans_overlap(&a, &b) == expected);
			}
		}
	}

	struct scaler_plane_span src[4] = {{1024, 64}, {2048, 64}, {3072, 64}, {4096, 64}};
	struct scaler_plane_span dst[4] = {{5120, 64}, {6144, 64}, {7168, 64}, {8192, 64}};
	CHECK(scaler_plane_sets_disjoint(src, 15, dst, 15));
	for (size_t d = 0; d < 4; ++d) {
		const struct scaler_plane_span saved = dst[d];
		for (size_t s = 0; s < 4; ++s) {
			dst[d] = src[s];
			CHECK(!scaler_plane_sets_disjoint(src, 15, dst, 15));
		}
		dst[d] = saved;
	}
	dst[3] = dst[0];
	CHECK(!scaler_plane_sets_disjoint(src, 15, dst, 15));
	/* Unused planes must not participate. */
	CHECK(scaler_plane_sets_disjoint(src, 1, dst, 7));
	puts("scaler spans: 65536 range comparisons and plane/overflow cases passed");
	return EXIT_SUCCESS;
}
