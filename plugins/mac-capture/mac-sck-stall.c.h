/*
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */

#include "plugins/mac-capture/build/mac-capture-sck-stall.pch.h"

#define NS_PER_MS 1000000ULL

enum sck_stall_cause sck_stall_classify(uint64_t now_ns, uint64_t last_delivery_ns, uint64_t last_drop_ns)
{
	UNUSED_PARAMETER(now_ns);

	/* No frame has been published since capture started. */
	if (last_delivery_ns == 0) {
		return SCK_STALL_NO_FRAMES;
	}

	/* The delegate refused a frame after the last one it published: the
	 * stream is producing, and the refusal is what the user cannot see. */
	if (last_drop_ns > last_delivery_ns) {
		return SCK_STALL_FRAMES_DROPPED;
	}

	/* A frame did arrive after the newest refusal, so the refusals are
	 * stale and the current gap is frames that stopped arriving. */
	if (last_drop_ns != 0) {
		return SCK_STALL_NO_FRAMES;
	}

	return SCK_STALL_FRAMES_ARRIVED;
}

bool sck_stall_should_warn(uint64_t now_ns, uint64_t last_delivery_ns, uint64_t warned_at_ns, unsigned int threshold_ms,
			   unsigned int rewarn_ms)
{
	/* Nothing to measure from until capture starts, and a clock that
	 * went backwards is not a stall. */
	if (last_delivery_ns == 0 || now_ns < last_delivery_ns) {
		return false;
	}

	if (now_ns - last_delivery_ns < (uint64_t)threshold_ms * NS_PER_MS) {
		return false;
	}

	if (warned_at_ns != 0 && now_ns >= warned_at_ns && now_ns - warned_at_ns < (uint64_t)rewarn_ms * NS_PER_MS) {
		return false;
	}

	return true;
}

const char *sck_stall_cause_name(enum sck_stall_cause cause)
{
	switch (cause) {
	case SCK_STALL_NO_FRAMES:
		return "no frame has arrived since capture started";
	case SCK_STALL_FRAMES_DROPPED:
		return "frames arrive but cannot be published";
	case SCK_STALL_FRAMES_ARRIVED:
		return "no newer frame has arrived";
	}

	return "unknown";
}

#undef NS_PER_MS
