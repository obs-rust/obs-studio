/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
    Copyright (C) 2025 by Taylor Giampaolo <warchamp7@obsproject.com>

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

#include "screenshot-render.hpp"
#include "display-helpers.hpp"

void RenderScreenshot(gs_texrender_t *texrender, obs_source_t *source, uint32_t sourceWidth, uint32_t sourceHeight,
		      uint32_t outputWidth, uint32_t outputHeight, enum gs_color_space space)
{
	if (gs_texrender_begin_with_color_space(texrender, outputWidth, outputHeight, space)) {
		vec4 zero;
		vec4_zero(&zero);

		int x{0};
		int y{0};
		int scaledWidth{0};
		int scaledHeight{0};
		float scale{0.0};

		GetScaleAndCenterPos(sourceWidth, sourceHeight, outputWidth, outputHeight, x, y, scale);

		scaledWidth = int(scale * float(sourceWidth));
		scaledHeight = int(scale * float(sourceHeight));

		gs_clear(GS_CLEAR_COLOR, &zero, 0.0f, 0);

		gs_viewport_push();
		gs_projection_push();

		gs_ortho(0.0f, (float)sourceWidth, 0.0f, (float)sourceHeight, -100.0f, 100.0f);
		gs_set_viewport(x, y, scaledWidth, scaledHeight);

		gs_blend_state_push();
		gs_blend_function(GS_BLEND_ONE, GS_BLEND_ZERO);

		if (source) {
			/* libobs keeps high-bit-depth sources (P010, I010, I210, I412:
			 * HEVC Main10, ProRes, 10-bit capture cards) as linear float.
			 * Scenes and the previews draw with linear sRGB on, which
			 * encodes them to sRGB in an 8-bit target; without it the
			 * screenshot is stored linear and comes out too dark. 8-bit
			 * sRGB sources keep the direct path, so their pixels and
			 * straight alpha stay as they were, and float targets (JXR)
			 * are left as they were. */
			const enum gs_color_space preferredSpaces[] = {GS_CS_SRGB, GS_CS_SRGB_16F, GS_CS_709_EXTENDED};
			const enum gs_color_space sourceSpace =
				obs_source_get_color_space(source, OBS_COUNTOF(preferredSpaces), preferredSpaces);
			const bool previousLinearSrgb =
				gs_set_linear_srgb(space == GS_CS_SRGB && sourceSpace != GS_CS_SRGB);

			obs_source_inc_showing(source);
			obs_source_video_render(source);
			obs_source_dec_showing(source);

			gs_set_linear_srgb(previousLinearSrgb);
		} else {
			obs_render_main_texture();
		}

		gs_projection_pop();
		gs_viewport_pop();

		gs_blend_state_pop();
		gs_texrender_end(texrender);
	}
}
