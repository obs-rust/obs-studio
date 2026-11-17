#include "libobs/build/libobs-graphics.pch.h"

/* Private graphics layout and log lines for the Rust matrix calls
 * (obs-graphics::matrix_entrypoints): one context read per call, giving the
 * addresses of the context's matrix stack and of the index of its top,
 * gs_valid's debug line, and gs_matrix_pop's error line. */
bool obs_graphics_matrix_stack(struct darray **stack, size_t **cur);
bool obs_graphics_matrix_stack(struct darray **stack, size_t **cur)
{
	graphics_t *graphics = gs_get_context();

	if (!graphics)
		return false;

	*stack = &graphics->matrix_stack.da;
	*cur = &graphics->cur_matrix;
	return true;
}

void obs_graphics_matrix_debug(const char *function);
void obs_graphics_matrix_debug(const char *function)
{
	blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
}

void obs_graphics_matrix_pop_error(void);
void obs_graphics_matrix_pop_error(void)
{
	blog(LOG_ERROR, "Tried to pop last matrix on stack");
}
