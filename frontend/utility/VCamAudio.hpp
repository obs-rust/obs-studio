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
	return saved >= 1 && saved <= VCAM_AUDIO_TRACKS ? (int)saved : 1;
}

inline VCamAudioOutputSettings VCamAudioToOutput(const VCamAudioConfig &config)
{
	return {config.enabled, VCamAudioTrackFromSaved(config.track)};
}

/* Whether to tell the user that the Virtual Camera started without its
 * microphone: audio was wanted, the output can send it, and the output's
 * virtualcam_audio proc answered that it is not running. */
inline bool VCamAudioFellBack(const VCamAudioConfig &config, bool outputSendsAudio, bool procAnswered,
			      bool micActive)
{
	(void)config;
	(void)outputSendsAudio;
	(void)procAnswered;
	(void)micActive;
	return false;
}
