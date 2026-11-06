#include "plugins/obs-ffmpeg/build/obs-ffmpeg-host-windows.pch.h"

/* obs-ffmpeg's Windows host (#853): NVENC needs a non-blacklisted NVIDIA
 * adapter in DXGI; AMF loads with the module on x64 (OBS_FFMPEG_AMF). */

#ifdef OBS_FFMPEG_AMF
extern void amf_load(void);
extern void amf_unload(void);
#endif

typedef HRESULT(WINAPI *create_dxgi_proc)(const IID *, IDXGIFactory1 **);

bool obs_ffmpeg_host_nvenc_available(bool (*is_blacklisted)(int device_id))
{
	static HMODULE dxgi = NULL;
	static create_dxgi_proc create = NULL;
	IDXGIFactory1 *factory;
	IDXGIAdapter1 *adapter;
	bool available = false;
	HRESULT hr;
	UINT i = 0;

	if (!dxgi) {
		dxgi = GetModuleHandleW(L"dxgi");
		if (!dxgi) {
			dxgi = LoadLibraryW(L"dxgi");
			if (!dxgi) {
				return true;
			}
		}
	}

	if (!create) {
		create = (create_dxgi_proc)GetProcAddress(dxgi, "CreateDXGIFactory1");
		if (!create) {
			return true;
		}
	}

	hr = create(&IID_IDXGIFactory1, &factory);
	if (FAILED(hr)) {
		return true;
	}

	while (factory->lpVtbl->EnumAdapters1(factory, i++, &adapter) == S_OK) {
		DXGI_ADAPTER_DESC desc;

		hr = adapter->lpVtbl->GetDesc(adapter, &desc);
		adapter->lpVtbl->Release(adapter);

		if (FAILED(hr)) {
			continue;
		}

		// 0x10de = NVIDIA Corporation
		/* NVIDIA uses a 16-bit PCI device ID, which fits the callback's int. */
		if (desc.VendorId == 0x10de && !is_blacklisted((int)desc.DeviceId)) {
			available = true;
			goto finish;
		}
	}

finish:
	factory->lpVtbl->Release(factory);
	return available;
}

void obs_ffmpeg_host_load(void)
{
#ifdef OBS_FFMPEG_AMF
	amf_load();
#endif
}

void obs_ffmpeg_host_unload(void)
{
#ifdef OBS_FFMPEG_AMF
	amf_unload();
#endif
}
