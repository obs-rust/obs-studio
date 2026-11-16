#include "libobs/build/libobs.pch.h"

void obs_canvas_set_name(obs_canvas_t *canvas, const char *name)
{
	if (obs_canvas_rules_name_empty(name))
		return;
	if (!obs_canvas_rules_renames(canvas->flags)) /* Do not allow renaming main canvases. */
		return;
	if (strcmp(name, canvas->context.name) == 0)
		return;

	char *prev_name = bstrdup(canvas->context.name);

	if (!obs_canvas_rules_public(canvas->context.private))
		obs_context_data_setname(&canvas->context, name);
	else
		obs_context_data_setname_ht(&canvas->context, name, &obs->data.named_canvases);

	struct calldata data;
	calldata_init(&data);
	calldata_set_ptr(&data, "canvas", canvas);
	calldata_set_string(&data, "new_name", canvas->context.name);
	calldata_set_string(&data, "prev_name", prev_name);
	signal_handler_signal(canvas->context.signals, "rename", &data);

	if (obs_canvas_rules_public(canvas->context.private))
		signal_handler_signal(obs->signals, "canvas_rename", &data);

	calldata_free(&data);
	bfree(prev_name);
}
