#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <mutex>

struct VolumeMeterChannelSnapshot {
	float peak;
	float peakHold;
	float magnitude;
	float inputPeakHold;
};

template<typename Mutex, std::size_t N>
std::array<VolumeMeterChannelSnapshot, N>
snapshotVolumeMeterChannels(Mutex &mutex, const float (&peak)[N], const float (&peakHold)[N],
			    const float (&magnitude)[N], const float (&inputPeakHold)[N])
{
	std::lock_guard<Mutex> locker(mutex);
	std::array<VolumeMeterChannelSnapshot, N> result;
	for (std::size_t i = 0; i < N; i++) {
		result[i] = {peak[i], peakHold[i], magnitude[i], inputPeakHold[i]};
	}
	return result;
}

template<typename Mutex, typename Reset>
bool resetVolumeMeterIfIdle(Mutex &mutex, uint64_t &lastUpdate, uint64_t now, Reset reset)
{
	std::lock_guard<Mutex> locker(mutex);
	// AppleClang states uint64 -> double precision loss by default (#3096); the
	// nanosecond delta is meant as seconds, so state the conversion here.
	double secondsSinceLastUpdate = static_cast<double>(now - lastUpdate) * 1e-9;
	if (secondsSinceLastUpdate > 0.5) {
		reset();
		return true;
	}
	return false;
}
