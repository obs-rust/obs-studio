#include "tbar-position.h"

enum tbar_release_action tbar_release_action(int value, int maximum, int clamp)
{
	if ((maximum - value) <= clamp)
		return TBAR_RELEASE_FINISH;
	if (value <= clamp)
		return TBAR_RELEASE_CANCEL;
	return TBAR_RELEASE_KEEP;
}

void tbar_api_set_position(const struct tbar_position_ops *ops, int position)
{
	/* Move the slider like a drag, so the slider's valueChanged runs
	 * position_changed and the slider holds the position: the UI shows it,
	 * obs_frontend_get_tbar_position() returns it and releasing the T-Bar
	 * reads it. */
	ops->set_slider_value(ops->param, position);
}
