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

#pragma once

#include <stdbool.h>
#include <stdint.h>

/* The caption plan of add_caption() in obs-output.c, on plain values
 * (obs-output-caption.c). One function per decision, each asked where the C
 * made it:
 * - obs_output_caption_begin(): the carrier the packet's codec gets, and
 *   whether a packet of that priority takes a caption at all;
 * - obs_output_caption_hevc_sei_header(): for HEVC, the Annex B start code
 *   of the packet and the NAL unit header of the caption's suffix SEI;
 * - obs_output_caption_cc_keep(): which CEA-608 triplets from the caption
 *   queue go into the CEA-708 frame.
 *
 * obs-output.c keeps the caption track lookup, every deque pop, bzalloc()
 * and bfree(), every libcaption call (sei_*, cea708_*, caption_frame_*,
 * extract_buffer_from_sei(), metadata_obu()), the new packet's DARRAY,
 * obs_encoder_packet_release() and the logs, in their order. Nothing here
 * allocates, calls into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* The carrier of a caption: obs_output_caption_begin()'s *carrier. */
#define OBS_OUTPUT_CAPTION_NONE 0u
#define OBS_OUTPUT_CAPTION_AVC 1u
#define OBS_OUTPUT_CAPTION_HEVC 2u
#define OBS_OUTPUT_CAPTION_AV1 3u

/* add_caption(): *carrier for a packet whose encoder's codec is codec
 * (info.codec): "h264" gives AVC, "av1" AV1, and "hevc" HEVC when hevc is
 * true (ENABLE_HEVC); any other codec gives NONE, and the caption is still
 * taken off the queue. Returns false, the caption staying queued, for a
 * packet of priority above 1; *carrier is set either way. */
bool obs_output_caption_begin(const char *codec, bool hevc, int priority, uint32_t *carrier);

/* add_caption(), for HEVC: the 2-byte NAL unit header of the caption's
 * suffix SEI, from the header after the packet's Annex B start code (00 00
 * 01, else 00 00 00 01): its NAL type becomes 40 (SUFFIX_SEI_NUT), its other
 * bits stay. Reads data[0..2], data[3] when the 3-byte start code does not
 * match, and the 2 header bytes after the start code. Returns false, with
 * header untouched, when data starts with neither start code. */
bool obs_output_caption_hevc_sei_header(const uint8_t *data, uint8_t header[2]);

/* add_caption(): a CEA-608 triplet (cc_valid and cc_type, then cc_data_1
 * and cc_data_2) from the caption queue goes into the CEA-708 frame: its
 * cc_type is 0 (CEA-608 field 1), and its cc_data is not padding (0x8080),
 * not 0 and has odd parity in both bytes. *cc_data, set only when it
 * returns true, is cc_data_1 << 8 | cc_data_2. */
bool obs_output_caption_cc_keep(const uint8_t triplet[3], uint16_t *cc_data);
