#include "libobs/build/libobs.pch.h"

extern obs_module_t *loadingModule;

/* The shared registrar's whole_fields_size(size, NULL, 0), kept local so
 * this unit can be compiled independently and later swapped as a whole. */
static size_t copy_size_obs_service_info(size_t size)
{
	return obs_module_register_copy_whole_size(size, sizeof(void *), NULL, 0);
}

#define REGISTER_OBS_DEF(size_var, structure, dest, info)                                               \
	do {                                                                                            \
		struct structure data = {0};                                                            \
		switch (obs_module_register_copy_gate(size_var, sizeof(data))) {                        \
		case OBS_MODULE_REGISTER_COPY_OUTSIDE_LOAD:                                             \
			blog(LOG_ERROR, "Tried to register " #structure " outside of obs_module_load"); \
			return;                                                                         \
		case OBS_MODULE_REGISTER_COPY_TOO_LARGE:                                                \
			blog(LOG_ERROR,                                                                 \
			     "Tried to register " #structure " with size %llu which is more "           \
			     "than libobs currently supports "                                          \
			     "(%llu)",                                                                  \
			     (long long unsigned)size_var, (long long unsigned)sizeof(data));           \
			goto error;                                                                     \
		case OBS_MODULE_REGISTER_COPY_COPY:                                                     \
		default:                                                                                \
			break;                                                                          \
		}                                                                                       \
                                                                                                        \
		memcpy(&data, info, copy_size_##structure(size_var));                                   \
		da_push_back(dest, &data);                                                              \
	} while (false)

#define HAS_VAL(type, info, val) \
	(obs_module_register_copy_within(offsetof(type, val), sizeof(info->val), size) && info->val)

#define HANDLE_ERROR(size_var, structure, info)                                         \
	do {                                                                            \
		/* Free the caller's type data only when type_data and          \
		 * free_type_data each lie wholly within the size, as in        \
		 * HAS_VAL(): a size that ends inside a pointer would leave it  \
		 * partly copied, and calling that jumps to a wrong address.    \
		 */                                                             \
		if (obs_module_register_copy_within(offsetof(struct structure, type_data),             \
						    sizeof(info->type_data), (size_var)) &&            \
		    obs_module_register_copy_within(offsetof(struct structure, free_type_data),        \
						    sizeof(info->free_type_data), (size_var)) &&       \
		    info->type_data && info->free_type_data)                                           \
			info->free_type_data(info->type_data);                                     \
	} while (false)

#define type_required(val, func)                             \
	blog(LOG_ERROR,                                      \
	     "Required value '" #val "' for "                \
	     "'%s' not found.  " #func " failed.",           \
	     info->id)
#define has_val(type, val, bit)                   \
	do {                                      \
		if (HAS_VAL(type, info, val))     \
			callbacks |= OBS_MODULE_TYPE_RULES_HAS_##bit; \
	} while (false)

#define service_warn(format, ...) blog(LOG_WARNING, "obs_register_service: " format, ##__VA_ARGS__)

void obs_register_service_s(const struct obs_service_info *info, size_t size)
{
	enum obs_module_type_rules_error err;
	uint32_t callbacks = 0;

	if (find_service(info->id)) {
		service_warn("Service id '%s' already exists!  "
			     "Duplicate library?",
			     info->id);
		goto error;
	}

	has_val(struct obs_service_info, get_name, GET_NAME);
	has_val(struct obs_service_info, create, CREATE);
	has_val(struct obs_service_info, destroy, DESTROY);
	has_val(struct obs_service_info, get_protocol, GET_PROTOCOL);

	err = obs_module_type_rules_service_check(callbacks);
	switch (err) {
	case OBS_MODULE_TYPE_RULES_OK:
		break;
	case OBS_MODULE_TYPE_RULES_NO_GET_NAME:
		type_required(get_name, obs_register_service);
		break;
	case OBS_MODULE_TYPE_RULES_NO_CREATE:
		type_required(create, obs_register_service);
		break;
	case OBS_MODULE_TYPE_RULES_NO_DESTROY:
		type_required(destroy, obs_register_service);
		break;
	case OBS_MODULE_TYPE_RULES_NO_GET_PROTOCOL:
		type_required(get_protocol, obs_register_service);
		break;
	default:
		break;
	}
	if (err != OBS_MODULE_TYPE_RULES_OK)
		goto error;

	REGISTER_OBS_DEF(size, obs_service_info, obs->service_types, info);

	if (loadingModule) {
		char *service_id = bstrdup(info->id);
		da_push_back(loadingModule->services, &service_id);
	}

	return;

error:
	HANDLE_ERROR(size, obs_service_info, info);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef type_required
#undef has_val
#undef REGISTER_OBS_DEF
#undef HAS_VAL
#undef HANDLE_ERROR
#undef service_warn
