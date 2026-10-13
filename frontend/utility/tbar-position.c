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
	ops->position_changed(ops->param, position);
}
