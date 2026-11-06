//
//  CMSampleBufferUtils.m
//  dal-plugin
//
//  Created by John Boiles  on 5/8/20.
//

#include "plugins/mac-virtualcam/src/dal-plugin/build/dal-plugin.pch.hpp"

CMSampleTimingInfo CMSampleTimingInfoForTimestamp(uint64_t timestampNanos, uint32_t fpsNumerator,
						  uint32_t fpsDenominator)
{
	// The timing here is quite important. For frames to be delivered correctly and successfully be recorded by apps
	// like QuickTime Player, we need to be accurate in both our timestamps _and_ have a sensible scale. Using large
	// timestamps and scales like clock_gettime_nsec_np() and NSEC_PER_SEC will work for display, but will error out
	// when trying to record.
	//
	// 600 is a common default in Apple's docs https://developer.apple.com/documentation/avfoundation/avmutablemovie/1390622-timescale
	CMTimeScale scale = 600;
	CMSampleTimingInfo timing;
	timing.duration = CMTimeMake(fpsDenominator * (uint32_t)scale, (int32_t)(fpsNumerator * (uint32_t)scale));
	CMTime timestamp = CMTimeMake((int64_t)timestampNanos, NSEC_PER_SEC);
	timing.presentationTimeStamp = CMTimeConvertScale(timestamp, scale, kCMTimeRoundingMethod_QuickTime);
	timing.decodeTimeStamp = kCMTimeInvalid;
	return timing;
}
