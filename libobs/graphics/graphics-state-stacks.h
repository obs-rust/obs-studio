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

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "../util/darray.h"
#include "axisang.h"
#include "graphics.h"
#include "matrix4.h"
#include "quat.h"
#include "vec3.h"

/* The state stacks of the graphics context (graphics-state-stacks.c), on
 * the graphics_subsystem fields by pointer: the matrix stack (a darray of
 * struct matrix4, with the index of its top), the viewport stack (a darray
 * of struct gs_rect) and the blend state stack (a darray of struct
 * blend_state). graphics.c keeps gs_valid, the device calls and the
 * logging, and calls these steps; nothing here takes the graphics context.
 *
 * Hidden in libobs: not EXPORT. */

struct blend_state {
	bool enabled;
	enum gs_blend_type src_c;
	enum gs_blend_type dest_c;
	enum gs_blend_type src_a;
	enum gs_blend_type dest_a;
	enum gs_blend_op_type op;
};

/* The matrix stack: `stack` is a darray of struct matrix4, `*cur` the
 * index of its top. */
void gs_matrix_stack_push(struct darray *stack, size_t *cur);
/* false when the top is the last matrix: nothing is popped. */
bool gs_matrix_stack_pop(struct darray *stack, size_t *cur);
void gs_matrix_stack_identity(struct darray *stack, size_t cur);
void gs_matrix_stack_transpose(struct darray *stack, size_t cur);
void gs_matrix_stack_set(struct darray *stack, size_t cur, const struct matrix4 *matrix);
void gs_matrix_stack_get(const struct darray *stack, size_t cur, struct matrix4 *dst);
void gs_matrix_stack_mul(struct darray *stack, size_t cur, const struct matrix4 *matrix);
void gs_matrix_stack_rotquat(struct darray *stack, size_t cur, const struct quat *rot);
void gs_matrix_stack_rotaa(struct darray *stack, size_t cur, const struct axisang *rot);
void gs_matrix_stack_translate(struct darray *stack, size_t cur, const struct vec3 *pos);
void gs_matrix_stack_scale(struct darray *stack, size_t cur, const struct vec3 *scale);

/* The viewport stack: push a rectangle; pop gives the rectangle to restore,
 * false when the stack is empty. */
void gs_viewport_stack_push(struct darray *stack, const struct gs_rect *rect);
bool gs_viewport_stack_pop(struct darray *stack, struct gs_rect *rect);

/* The blend state stack: push a state; pop gives the state to restore,
 * false when the stack is empty. */
void gs_blend_state_stack_push(struct darray *stack, const struct blend_state *state);
bool gs_blend_state_stack_pop(struct darray *stack, struct blend_state *state);

/* The device steps of gs_reset_blend_state for the current state: enable
 * blending; set the default blend function; set the default blend op. */
struct gs_blend_reset {
	bool enable;
	bool function;
	bool op;
};

struct gs_blend_reset gs_blend_state_reset_steps(const struct blend_state *cur);
