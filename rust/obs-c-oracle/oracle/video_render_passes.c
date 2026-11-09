/* Actual C helpers and actual adapter over the unchanged characterization's
 * recording graphics double. Rust and C execute under the Rust facade's lock. */
#include "video-render-passes-mocks.inc"

#define obs_video_draw_mix_texture oracle_c_video_draw_mix_texture
#define obs_video_render_convert_plane oracle_c_video_render_convert_plane
#include "obs-video-render-passes.inc"
#undef obs_video_draw_mix_texture
#undef obs_video_render_convert_plane

#include "obs-video-render-passes-adapter.inc"

size_t oracle_video_render_passes(bool rust, bool convert, size_t index, size_t count, uint32_t w, uint32_t h,
				  size_t plane, char *output);

size_t oracle_video_render_passes(bool rust, bool convert, size_t index, size_t count, uint32_t w, uint32_t h,
				  size_t plane, char *output)
{
	struct mix mix0 = {&textures[0]}, mix1 = {&textures[1]};
	struct mix *mixes[] = {&mix0, &mix1};
	static const char *names[] = {"Planar_Y", "Planar_U", "Planar_V"};
	obs->video.mixes.array = mixes;
	expected_texture = &textures[index];
	expected_technique = names[plane];
	record_width = w;
	record_height = h;
	record_passes = count;
	loop_index = pass_index = call_count = 0;
	if (convert) {
		if (rust)
			render_convert_plane(&effect_token, expected_texture, expected_technique);
		else
			oracle_c_video_render_convert_plane(&effect_token, expected_texture, expected_technique,
							    video_render_passes_call);
		assert_int_equal(pass_index, count);
	} else {
		if (rust)
			draw_mix_texture(index);
		else
			oracle_c_video_draw_mix_texture(index, video_render_passes_call);
		assert_int_equal(loop_index, count + 1);
	}
	memcpy(output, calls, call_count);
	obs->video.mixes.array = NULL;
	return call_count;
}
