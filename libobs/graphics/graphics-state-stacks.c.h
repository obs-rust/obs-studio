/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "build/libobs-ported-graphics-state-stacks.pch.h"

/* The state stacks of the graphics context, declared in
 * graphics-state-stacks.h: the code of graphics.c's gs_matrix_*,
 * gs_viewport_push/pop, gs_blend_state_push/pop and gs_reset_blend_state
 * without gs_valid, the device calls and the logging, which graphics.c
 * keeps. Ported to rust/obs-graphics (state_stacks): the hybrid libobs
 * drops this file. */

static inline struct matrix4 *gs_matrix_stack__top(struct darray *stack, size_t cur)
{
	return (struct matrix4 *)stack->array + cur;
}

void gs_matrix_stack_push(struct darray *stack, size_t *cur)
{
	struct matrix4 mat, *top_mat = gs_matrix_stack__top(stack, *cur);

	memcpy(&mat, top_mat, sizeof(struct matrix4));
	darray_push_back(sizeof(struct matrix4), stack, &mat);
	(*cur)++;
}

bool gs_matrix_stack_pop(struct darray *stack, size_t *cur)
{
	if (*cur == 0)
		return false;

	darray_erase(sizeof(struct matrix4), stack, *cur);
	(*cur)--;
	return true;
}

void gs_matrix_stack_identity(struct darray *stack, size_t cur)
{
	struct matrix4 *top_mat = gs_matrix_stack__top(stack, cur);
	if (top_mat)
		matrix4_identity(top_mat);
}

void gs_matrix_stack_transpose(struct darray *stack, size_t cur)
{
	struct matrix4 *top_mat = gs_matrix_stack__top(stack, cur);
	if (top_mat)
		matrix4_transpose(top_mat, top_mat);
}

void gs_matrix_stack_set(struct darray *stack, size_t cur, const struct matrix4 *matrix)
{
	struct matrix4 *top_mat = gs_matrix_stack__top(stack, cur);
	if (top_mat)
		matrix4_copy(top_mat, matrix);
}

void gs_matrix_stack_get(const struct darray *stack, size_t cur, struct matrix4 *dst)
{
	const struct matrix4 *top_mat = (const struct matrix4 *)stack->array + cur;
	if (top_mat)
		matrix4_copy(dst, top_mat);
}

void gs_matrix_stack_mul(struct darray *stack, size_t cur, const struct matrix4 *matrix)
{
	struct matrix4 *top_mat = gs_matrix_stack__top(stack, cur);
	if (top_mat)
		matrix4_mul(top_mat, matrix, top_mat);
}

void gs_matrix_stack_rotquat(struct darray *stack, size_t cur, const struct quat *rot)
{
	struct matrix4 *top_mat = gs_matrix_stack__top(stack, cur);
	if (top_mat)
		matrix4_rotate_i(top_mat, rot, top_mat);
}

void gs_matrix_stack_rotaa(struct darray *stack, size_t cur, const struct axisang *rot)
{
	struct matrix4 *top_mat = gs_matrix_stack__top(stack, cur);
	if (top_mat)
		matrix4_rotate_aa_i(top_mat, rot, top_mat);
}

void gs_matrix_stack_translate(struct darray *stack, size_t cur, const struct vec3 *pos)
{
	struct matrix4 *top_mat = gs_matrix_stack__top(stack, cur);
	if (top_mat)
		matrix4_translate3v_i(top_mat, pos, top_mat);
}

void gs_matrix_stack_scale(struct darray *stack, size_t cur, const struct vec3 *scale)
{
	struct matrix4 *top_mat = gs_matrix_stack__top(stack, cur);
	if (top_mat)
		matrix4_scale_i(top_mat, scale, top_mat);
}

void gs_viewport_stack_push(struct darray *stack, const struct gs_rect *rect)
{
	darray_push_back(sizeof(struct gs_rect), stack, rect);
}

bool gs_viewport_stack_pop(struct darray *stack, struct gs_rect *rect)
{
	if (!stack->num)
		return false;

	*rect = *((struct gs_rect *)darray_end(sizeof(struct gs_rect), stack));
	darray_pop_back(sizeof(struct gs_rect), stack);
	return true;
}

void gs_blend_state_stack_push(struct darray *stack, const struct blend_state *state)
{
	darray_push_back(sizeof(struct blend_state), stack, state);
}

bool gs_blend_state_stack_pop(struct darray *stack, struct blend_state *state)
{
	struct blend_state *end = darray_end(sizeof(struct blend_state), stack);
	if (!end)
		return false;

	*state = *end;
	darray_pop_back(sizeof(struct blend_state), stack);
	return true;
}

struct gs_blend_reset gs_blend_state_reset_steps(const struct blend_state *cur)
{
	struct gs_blend_reset steps = {
		.enable = !cur->enabled,
		.function = cur->src_c != GS_BLEND_SRCALPHA || cur->dest_c != GS_BLEND_INVSRCALPHA ||
			    cur->src_a != GS_BLEND_ONE || cur->dest_a != GS_BLEND_INVSRCALPHA,
	};

	/* The op is part of the default state too (#2519): with a different
	 * function, as before, and on its own. */
	steps.op = steps.function || cur->op != GS_BLEND_OP_ADD;
	return steps;
}
