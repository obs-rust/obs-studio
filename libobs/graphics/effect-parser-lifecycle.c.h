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

#include "libobs/build/libobs-graphics.pch.h"

void ep_free(struct effect_parser *ep)
{
	size_t i;
	for (i = 0; i < ep->params.num; i++)
		ep_param_free(ep->params.array + i);
	for (i = 0; i < ep->structs.num; i++)
		ep_struct_free(ep->structs.array + i);
	for (i = 0; i < ep->funcs.num; i++)
		ep_func_free(ep->funcs.array + i);
	for (i = 0; i < ep->samplers.num; i++)
		ep_sampler_free(ep->samplers.array + i);
	for (i = 0; i < ep->techniques.num; i++)
		ep_technique_free(ep->techniques.array + i);

	ep->cur_pass = NULL;
	cf_parser_free(&ep->cfp);
	da_free(ep->params);
	da_free(ep->structs);
	da_free(ep->funcs);
	da_free(ep->samplers);
	da_free(ep->techniques);
}
