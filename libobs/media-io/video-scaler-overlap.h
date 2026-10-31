/* SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Private fast-path predicates. No pointer arithmetic or dereferences. */
struct scaler_plane_span {
	uintptr_t start;
	size_t size;
};

static inline bool scaler_plane_span_init(struct scaler_plane_span *span, uintptr_t start, uint32_t stride,
					  uint32_t rows)
{
	/* The public stride is unsigned, but sws_scale receives an int. Keep
	 * encoded negative strides and unrepresentable spans on the old path. */
	if (!start || !stride || stride > INT_MAX || !rows || stride > SIZE_MAX / rows)
		return false;
	const size_t size = (size_t)stride * rows;
	if (size > UINTPTR_MAX - start)
		return false;
	span->start = start;
	span->size = size;
	return true;
}

static inline bool scaler_plane_spans_overlap(const struct scaler_plane_span *a, const struct scaler_plane_span *b)
{
	return a->start <= b->start ? b->start - a->start < a->size : a->start - b->start < b->size;
}

static inline bool scaler_plane_sets_disjoint(const struct scaler_plane_span src[4], uint32_t src_mask,
					      const struct scaler_plane_span dst[4], uint32_t dst_mask)
{
	for (size_t d = 0; d < 4; ++d) {
		if (!(dst_mask & (1u << d)))
			continue;
		for (size_t s = 0; s < 4; ++s) {
			if ((src_mask & (1u << s)) && scaler_plane_spans_overlap(&dst[d], &src[s]))
				return false;
		}
		/* Scratch-copy also defines the order for overlapping output
		 * planes. Direct swscale writes need not preserve that order. */
		for (size_t other = 0; other < d; ++other) {
			if ((dst_mask & (1u << other)) && scaler_plane_spans_overlap(&dst[d], &dst[other]))
				return false;
		}
	}
	return true;
}
