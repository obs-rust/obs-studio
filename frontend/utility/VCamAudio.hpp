#pragma once

/* OBS Virtual Microphone settings of the Virtual Camera (#3182): whether the
 * output also sends audio, and which track. Kept free of libobs so it can be
 * unit tested. */

constexpr int VCAM_AUDIO_TRACKS = 6;

struct VCamAudioConfig {
	bool enabled = true;
	int track = 1;
};

/* What the virtualcam output receives as settings. */
struct VCamAudioOutputSettings {
	bool audio_enabled;
	long long audio_track;
};

/* A saved track outside 1..6 (or a collection saved before #3182) reads as
 * track 1. */
inline int VCamAudioTrackFromSaved(long long saved)
{
	(void)saved;
	return 0;
}

inline VCamAudioOutputSettings VCamAudioToOutput(const VCamAudioConfig &config)
{
	(void)config;
	return {false, 0};
}
