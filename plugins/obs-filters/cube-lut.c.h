#include "plugins/obs-filters/build/obs-filters.pch.h"

/* Adobe Cube LUT Specification 1.0 */
#define LUT_1D_SIZE_MAX 65536
#define LUT_3D_SIZE_MAX 256

static bool get_cube_entry(FILE *const file, float *const red, float *const green, float *const blue)
{
	bool data_found = false;

	char line[256];
	while (fgets(line, sizeof(line), file)) {
		if (sscanf(line, "%f %f %f", red, green, blue) == 3) {
			data_found = true;
			break;
		}
	}

	return data_found;
}

static void *load_1d_lut(FILE *const file, const uint32_t width, float red, float green, float blue)
{
	const size_t data_size = 4 * (size_t)width * sizeof(struct half);
	struct half *values = bmalloc(data_size);

	size_t offset = 0;
	bool data_found = true;
	for (uint32_t index = 0; index < width; ++index) {
		if (!data_found) {
			bfree(values);
			values = NULL;
			break;
		}

		values[offset++] = half_from_float(gs_srgb_nonlinear_to_linear(red));
		values[offset++] = half_from_float(gs_srgb_nonlinear_to_linear(green));
		values[offset++] = half_from_float(gs_srgb_nonlinear_to_linear(blue));
		values[offset++] = half_from_bits(0x3c00); // 1.0

		data_found = get_cube_entry(file, &red, &green, &blue);
	}

	return values;
}

static void *load_3d_lut(FILE *const file, const uint32_t width, float red, float green, float blue)
{
	const size_t data_size = 4 * (size_t)width * width * width * sizeof(struct half);
	struct half *values = bmalloc(data_size);

	size_t offset = 0;
	bool data_found = true;
	for (uint32_t z = 0; z < width; ++z) {
		for (uint32_t y = 0; y < width; ++y) {
			for (uint32_t x = 0; x < width; ++x) {
				if (!data_found) {
					bfree(values);
					values = NULL;
					break;
				}

				values[offset++] = half_from_float(gs_srgb_nonlinear_to_linear(red));
				values[offset++] = half_from_float(gs_srgb_nonlinear_to_linear(green));
				values[offset++] = half_from_float(gs_srgb_nonlinear_to_linear(blue));
				values[offset++] = half_from_bits(0x3c00); // 1.0

				data_found = get_cube_entry(file, &red, &green, &blue);
			}
		}
	}

	return values;
}

void *load_cube_file(const char *const path, uint32_t *const width, struct vec3 *domain_min, struct vec3 *domain_max,
		     enum clut_dimension *dim)
{
	void *data = NULL;

	FILE *const file = os_fopen(path, "rb");
	if (file) {
		float red = 0.0f, green = 0.0f, blue = 0.0f;
		unsigned width_1d = 0;
		unsigned width_3d = 0;

		bool data_found = false;

		char line[256];
		unsigned u;
		float f[3];
		while (fgets(line, sizeof(line), file)) {
			if (sscanf(line, "%f %f %f", &red, &green, &blue) == 3) {
				/* no more metadata */
				data_found = true;
				break;
			} else if (sscanf(line, "DOMAIN_MIN %f %f %f", &f[0], &f[1], &f[2]) == 3) {
				vec3_set(domain_min, f[0], f[1], f[2]);
			} else if (sscanf(line, "DOMAIN_MAX %f %f %f", &f[0], &f[1], &f[2]) == 3) {
				vec3_set(domain_max, f[0], f[1], f[2]);
			} else if (sscanf(line, "LUT_1D_SIZE %u", &u) == 1) {
				width_1d = u;
			} else if (sscanf(line, "LUT_3D_SIZE %u", &u) == 1) {
				width_3d = u;
			}
		}

		if (domain_min->x >= domain_max->x || domain_min->y >= domain_max->y ||
		    domain_min->z >= domain_max->z) {
			blog(LOG_WARNING, "Invalid CUBE LUT domain: [%f, %f], [%f, %f], [%f, %f]",
			     (double)domain_min->x, (double)domain_max->x, (double)domain_min->y, (double)domain_max->y,
			     (double)domain_min->z, (double)domain_max->z);
		} else if (width_1d > LUT_1D_SIZE_MAX || width_3d > LUT_3D_SIZE_MAX) {
			blog(LOG_WARNING, "CUBE LUT too large: LUT_1D_SIZE %u, LUT_3D_SIZE %u", width_1d, width_3d);
		} else if (data_found) {
			if (width_1d > 0) {
				data = load_1d_lut(file, width_1d, red, green, blue);
				if (data) {
					*width = width_1d;
					*dim = CLUT_1D;
				}
			} else if (width_3d > 0) {
				data = load_3d_lut(file, width_3d, red, green, blue);
				if (data) {
					*width = width_3d;
					*dim = CLUT_3D;
				}
			}
		}

		fclose(file);
	}

	return data;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef LUT_1D_SIZE_MAX
#undef LUT_3D_SIZE_MAX
