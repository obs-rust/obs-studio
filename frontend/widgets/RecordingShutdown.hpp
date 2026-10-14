#pragma once

#include <cstdint>

/* Called by OBSBasic::closeWindow() before the outputs are destroyed.
 *
 * Today nothing stops the recording here: an active recording is still
 * running when outputHandler.reset() destroys its output, which force-stops
 * it and drops the frames the encoder still holds. */
template<typename Handler> bool StopRecordingForShutdown(Handler *handler, uint64_t timeout_ms)
{
	(void)handler;
	(void)timeout_ms;
	return true;
}
