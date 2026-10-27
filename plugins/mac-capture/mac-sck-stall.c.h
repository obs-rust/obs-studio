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

enum sck_stall_cause sck_stall_classify(uint64_t now_ns, uint64_t last_delivery_ns,
					uint64_t last_drop_ns)
{
	UNUSED_PARAMETER(now_ns);
	UNUSED_PARAMETER(last_delivery_ns);
	UNUSED_PARAMETER(last_drop_ns);
	return SCK_STALL_FRAMES_ARRIVED;
}

bool sck_stall_should_warn(uint64_t now_ns, uint64_t last_delivery_ns, uint64_t warned_at_ns,
			   unsigned int threshold_ms, unsigned int rewarn_ms)
{
	UNUSED_PARAMETER(now_ns);
	UNUSED_PARAMETER(last_delivery_ns);
	UNUSED_PARAMETER(warned_at_ns);
	UNUSED_PARAMETER(threshold_ms);
	UNUSED_PARAMETER(rewarn_ms);
	return false;
}

const char *sck_stall_cause_name(enum sck_stall_cause cause)
{
	UNUSED_PARAMETER(cause);
	return "no frame";
}
