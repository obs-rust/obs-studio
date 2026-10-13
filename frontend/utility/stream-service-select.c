#include "stream-service-select.h"

void stream_service_selected(const struct stream_service_select_ops *ops)
{
	if (ops->is_show_all(ops->param))
		ops->show_all(ops->param);
	else
		ops->service_changed(ops->param);

	ops->mark_changed(ops->param);
}
