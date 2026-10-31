#include "../utility/VCamAudio.hpp"

#include <cassert>

int main()
{
	// A collection saved before #3182 has no track: track 1, audio on.
	VCamAudioConfig defaults;
	assert(defaults.enabled);
	assert(defaults.track == 1);
	assert(VCamAudioTrackFromSaved(0) == 1);

	for (int track = 1; track <= VCAM_AUDIO_TRACKS; track++) {
		assert(VCamAudioTrackFromSaved(track) == track);
	}
	assert(VCamAudioTrackFromSaved(7) == 1);
	assert(VCamAudioTrackFromSaved(-3) == 1);

	VCamAudioOutputSettings on = VCamAudioToOutput(defaults);
	assert(on.audio_enabled);
	assert(on.audio_track == 1);

	VCamAudioOutputSettings off = VCamAudioToOutput({false, 4});
	assert(!off.audio_enabled);
	assert(off.audio_track == 4);

	// The video-only notice: only when audio was wanted and did not start.
	assert(VCamAudioFellBack(defaults, true, true, false));
	assert(!VCamAudioFellBack(defaults, true, true, true));
	assert(!VCamAudioFellBack({false, 1}, true, true, false));
	// Windows/macOS outputs send no audio and have no proc.
	assert(!VCamAudioFellBack(defaults, false, false, false));
	assert(!VCamAudioFellBack(defaults, true, false, false));
}
