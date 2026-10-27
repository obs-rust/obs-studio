#include "plugins/obs-nvenc/build/obs-nvenc.pch.h"

static void *cuda_lib = NULL;

bool load_cuda_lib(void);
bool load_cuda_lib(void)
{
#ifdef _WIN32
	cuda_lib = os_dlopen("nvcuda.dll");
#else
	cuda_lib = os_dlopen("libcuda.so.1");
#endif
	return cuda_lib != NULL;
}

static void *load_cuda_func(const char *func)
{
	void *func_ptr = os_dlsym(cuda_lib, func);
	if (!func_ptr) {
		blog(LOG_ERROR, "[obs-nvenc] Could not load function: %s", func);
	}
	return func_ptr;
}

typedef struct cuda_function {
	ptrdiff_t offset;
	const char *name;
} cuda_function;

static const cuda_function cuda_functions[] = {
	{offsetof(CudaFunctions, cuInit), "cuInit"},

	{offsetof(CudaFunctions, cuDeviceGetCount), "cuDeviceGetCount"},
	{offsetof(CudaFunctions, cuDeviceGet), "cuDeviceGet"},
	{offsetof(CudaFunctions, cuDeviceGetAttribute), "cuDeviceGetAttribute"},

	{offsetof(CudaFunctions, cuCtxCreate), "cuCtxCreate_v2"},
	{offsetof(CudaFunctions, cuCtxDestroy), "cuCtxDestroy_v2"},
	{offsetof(CudaFunctions, cuCtxPushCurrent), "cuCtxPushCurrent_v2"},
	{offsetof(CudaFunctions, cuCtxPopCurrent), "cuCtxPopCurrent_v2"},

	{offsetof(CudaFunctions, cuArray3DCreate), "cuArray3DCreate_v2"},
	{offsetof(CudaFunctions, cuArrayDestroy), "cuArrayDestroy"},
	{offsetof(CudaFunctions, cuMemcpy2D), "cuMemcpy2D_v2"},

	{offsetof(CudaFunctions, cuGetErrorName), "cuGetErrorName"},
	{offsetof(CudaFunctions, cuGetErrorString), "cuGetErrorString"},

	{offsetof(CudaFunctions, cuMemHostRegister), "cuMemHostRegister_v2"},
	{offsetof(CudaFunctions, cuMemHostUnregister), "cuMemHostUnregister"},

#ifndef _WIN32
	{offsetof(CudaFunctions, cuGLGetDevices), "cuGLGetDevices_v2"},
	{offsetof(CudaFunctions, cuGraphicsGLRegisterImage), "cuGraphicsGLRegisterImage"},
	{offsetof(CudaFunctions, cuGraphicsUnregisterResource), "cuGraphicsUnregisterResource"},
	{offsetof(CudaFunctions, cuGraphicsMapResources), "cuGraphicsMapResources"},
	{offsetof(CudaFunctions, cuGraphicsUnmapResources), "cuGraphicsUnmapResources"},
	{offsetof(CudaFunctions, cuGraphicsSubResourceGetMappedArray), "cuGraphicsSubResourceGetMappedArray"},
#endif
};

static const size_t num_cuda_funcs = sizeof(cuda_functions) / sizeof(cuda_function);

bool init_cuda_internal(obs_encoder_t *encoder)
{
	static bool initialized = false;
	static bool success = false;

	if (initialized)
		return success;
	initialized = true;

	if (!load_cuda_lib()) {
		obs_encoder_set_last_error(encoder, "Loading CUDA library failed.");
		return false;
	}

	cu = bzalloc(sizeof(CudaFunctions));

	for (size_t idx = 0; idx < num_cuda_funcs; idx++) {
		const cuda_function func = cuda_functions[idx];
		void *fptr = load_cuda_func(func.name);

		if (!fptr) {
			blog(LOG_ERROR, "[obs-nvenc] Failed to find CUDA function: %s", func.name);
			obs_encoder_set_last_error(encoder, "Loading CUDA functions failed.");
			return false;
		}

		*(uintptr_t *)((uintptr_t)cu + func.offset) = (uintptr_t)fptr;
	}

	success = true;
	return true;
}
