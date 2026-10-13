#include "stream-service-select.h"

void stream_service_selected(const struct stream_service_select_ops *ops)
{
	/* "Show all..." only expands the list and keeps the service: there is
	 * nothing to apply until a service is picked from the full list. */
	if (ops->is_show_all(ops->param)) {
		ops->show_all(ops->param);
		return;
	}

	ops->service_changed(ops->param);
	ops->mark_changed(ops->param);
}
