#include "plugins/win-dshow/virtualcam-module/build/virtualcam-module-c.pch.h"

static bool have_clockfreq = false;
static LARGE_INTEGER clock_freq;

static inline uint64_t get_clockfreq(void)
{
	if (!have_clockfreq) {
		QueryPerformanceFrequency(&clock_freq);
		have_clockfreq = true;
	}

	return clock_freq.QuadPart;
}

uint64_t gettime_100ns(void)
{
	LARGE_INTEGER current_time;
	double time_val;

	QueryPerformanceCounter(&current_time);
	time_val = (double)current_time.QuadPart;
	time_val *= 10000000.0;
	time_val /= (double)get_clockfreq();

	return (uint64_t)time_val;
}

bool sleepto_100ns(uint64_t time_target)
{
	uint64_t t = gettime_100ns();
	uint32_t milliseconds;

	if (t >= time_target) {
		return false;
	}

	milliseconds = (uint32_t)((time_target - t) / 10000);
	if (milliseconds > 1) {
		Sleep(milliseconds - 1);
	}

	/* No QPC spin after the sleep. This runs once per frame in the process of
	 * whatever app uses the OBS Virtual Camera, where the `Sleep(0)` loop spun
	 * for the remainder of the frame. `Sleep(milliseconds - 1)` stops within a
	 * tick of the target: up to ~2 ms early when the timer is fine, and late
	 * when it is not (`milliseconds` is a truncated count and `Sleep` rounds up
	 * to the host's granularity). The caller waits on an absolute deadline
	 * (`cur_time += obs_interval`), so an early return does not drift the frame
	 * schedule (obs-rust/obs-studio#1279). */
	return true;
}
