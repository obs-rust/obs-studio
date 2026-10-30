/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs-ported-output-caption.pch.h"

static const uint8_t caption_nal_start[4] = {0, 0, 0, 1};

bool obs_output_caption_begin(const char *codec, bool hevc, int priority, uint32_t *carrier)
{
	/* Instead of exiting early for unsupported codecs, we will continue
	 * processing to allow the freeing of caption data even if the captions
	 * will not be included in the bitstream due to being unimplemented in
	 * the given codec. */
	if (strcmp(codec, "h264") == 0) {
		*carrier = OBS_OUTPUT_CAPTION_AVC;
	} else if (strcmp(codec, "av1") == 0) {
		*carrier = OBS_OUTPUT_CAPTION_AV1;
	} else if (hevc && strcmp(codec, "hevc") == 0) {
		*carrier = OBS_OUTPUT_CAPTION_HEVC;
	} else {
		*carrier = OBS_OUTPUT_CAPTION_NONE;
	}

	return priority <= 1;
}

bool obs_output_caption_hevc_sei_header(const uint8_t *data, uint8_t header[2])
{
	size_t nal_header_index_start;
	// Skip past the annex-b start code
	if (memcmp(data, caption_nal_start + 1, 3) == 0) {
		nal_header_index_start = 3;
	} else if (memcmp(data, caption_nal_start, 4) == 0) {
		nal_header_index_start = 4;
	} else {
		return false;
	}

	/* nal_unit_header( ) {
	 * forbidden_zero_bit       f(1)
	 * nal_unit_type            u(6)
	 * nuh_layer_id             u(6)
	 * nuh_temporal_id_plus1    u(3)
	 * }
	 */
	const uint8_t suffix_sei_nal_type = 40;
	/* We will use the same 2 byte NAL unit header for the CC SEI, but swap
	 * the NAL types out. The first bit is always 0, so we just need to save
	 * the last bit off the original header and add the SEI NAL type. */
	header[0] = (uint8_t)((suffix_sei_nal_type << 1) | (0x01 & data[nal_header_index_start]));
	header[1] = data[nal_header_index_start + 1];
	return true;
}

bool obs_output_caption_cc_keep(const uint8_t triplet[3], uint16_t *cc_data)
{
	if ((triplet[0] & 0x3) != 0) {
		// only send cea 608
		return false;
	}

	uint16_t captionData = triplet[1];
	captionData = (uint16_t)(captionData << 8);
	captionData += triplet[2];

	// padding
	if (captionData == 0x8080) {
		return false;
	}

	if (captionData == 0) {
		return false;
	}

	if (!eia608_parity_varify(captionData)) {
		return false;
	}

	*cc_data = captionData;
	return true;
}
