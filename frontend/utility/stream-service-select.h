#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Settings > Stream: a new entry was selected in the Service list, kept free
 * of Qt so it can be unit tested. The list starts with the common services
 * and ends with "Show all...", which reloads it with every service and keeps
 * the current service selected. */
struct stream_service_select_ops {
	void *param;

	/* Whether the selected entry is "Show all...". */
	bool (*is_show_all)(void *param);

	/* Reloads the list with every service, selects the service that was
	 * selected before and opens the list again. */
	void (*show_all)(void *param);

	/* The service changed: updates the server list, the stream key link,
	 * the recommendations and the other fields that follow the service. */
	void (*service_changed)(void *param);

	/* Marks the stream settings changed, which enables Apply and makes
	 * closing Settings ask about unsaved changes. */
	void (*mark_changed)(void *param);
};

void stream_service_selected(const struct stream_service_select_ops *ops);

#ifdef __cplusplus
}
#endif
