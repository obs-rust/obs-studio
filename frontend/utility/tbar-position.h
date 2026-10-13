#pragma once

#ifdef __cplusplus
extern "C" {
#endif

/* The Studio Mode T-Bar, kept free of Qt so it can be unit tested. The slider
 * holds the T-Bar position (0 to its maximum). Moving it starts a manual
 * transition and sets the transition's progress; releasing it finishes the
 * transition near the end, cancels it near the start, and otherwise leaves it
 * where it is. Plugins move and release the T-Bar through
 * obs_frontend_set_tbar_position() and obs_frontend_release_tbar()
 * (obs-websocket's SetTBarPosition, Advanced Scene Switcher, ...). */

enum tbar_release_action {
	TBAR_RELEASE_KEEP,
	TBAR_RELEASE_FINISH,
	TBAR_RELEASE_CANCEL,
};

/* What releasing the T-Bar at value does (OBSBasic::TBarReleased). */
enum tbar_release_action tbar_release_action(int value, int maximum, int clamp);

struct tbar_position_ops {
	void *param;

	/* Sets the slider's value, like dragging it. The slider clamps the value
	 * to its range and, when the value changes, calls position_changed. */
	void (*set_slider_value)(void *param, int value);

	/* The slider's value changed (OBSBasic::TBarChanged): starts the manual
	 * transition if needed and sets its progress. */
	void (*position_changed)(void *param, int value);
};

/* obs_frontend_set_tbar_position(): moves the slider to position. */
void tbar_api_set_position(const struct tbar_position_ops *ops, int position);

#ifdef __cplusplus
}
#endif
