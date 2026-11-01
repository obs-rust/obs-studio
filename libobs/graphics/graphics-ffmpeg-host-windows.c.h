#include "libobs/build/libobs-graphics-ffmpeg-host-windows.pch.h"

/* The image loader's Windows host (obs-rust/obs-studio#872): JPEG XR files
 * FFmpeg cannot decode go through WIC, with PQ HDR converted to scRGB. */

static void *wic_image_init_internal(const char *file, IWICBitmapFrameDecode *pFrame, enum gs_color_format *format,
				     uint32_t *cx_out, uint32_t *cy_out, enum gs_color_space *space)
{
	BYTE *bytes = NULL;

	WICPixelFormatGUID pixelFormat;
	HRESULT hr = pFrame->lpVtbl->GetPixelFormat(pFrame, &pixelFormat);
	if (SUCCEEDED(hr)) {
		const bool scrgb = memcmp(&pixelFormat, &GUID_WICPixelFormat64bppRGBAHalf, sizeof(pixelFormat)) == 0;
		const bool pq10 = memcmp(&pixelFormat, &GUID_WICPixelFormat32bppBGR101010, sizeof(pixelFormat)) == 0;
		const enum gs_image_wic_kind kind = gs_image_wic_kind(scrgb, pq10);
		if (kind != GS_IMAGE_WIC_UNSUPPORTED) {
			UINT width, height;
			hr = pFrame->lpVtbl->GetSize(pFrame, &width, &height);
			if (SUCCEEDED(hr)) {
				const UINT pitch = 8 * width;
				const UINT size = pitch * height;
				bytes = bmalloc(size);
				if (bytes) {
					bool success = false;
					if (kind == GS_IMAGE_WIC_PQ10) {
						const UINT intermediate_pitch = 4 * width;
						const UINT intermediate_size = intermediate_pitch * height;
						BYTE *intermediate = bmalloc(intermediate_size);
						if (intermediate) {
							hr = pFrame->lpVtbl->CopyPixels(pFrame, NULL,
											intermediate_pitch,
											intermediate_size,
											intermediate);
							success = SUCCEEDED(hr);
							if (success) {
								gs_image_pq10_to_scrgb(bytes, size, intermediate,
										       intermediate_size);
							} else {
								blog(LOG_WARNING,
								     "WIC: Failed to CopyPixels intermediate for file: %s",
								     file);
							}

							bfree(intermediate);
						} else {
							blog(LOG_WARNING,
							     "WIC: Failed to allocate intermediate for file: %s", file);
						}
					} else {
						hr = pFrame->lpVtbl->CopyPixels(pFrame, NULL, pitch, size, bytes);
						success = SUCCEEDED(hr);
						if (!success) {
							blog(LOG_WARNING, "WIC: Failed to CopyPixels for file: %s",
							     file);
						}
					}

					if (success) {
						*format = GS_RGBA16F;
						*cx_out = width;
						*cy_out = height;
						*space = GS_CS_709_SCRGB;
					} else {
						bfree(bytes);
						bytes = NULL;
					}
				} else {
					blog(LOG_WARNING, "WIC: Failed to allocate for file: %s", file);
				}
			} else {
				blog(LOG_WARNING, "WIC: Failed to GetSize of frame for file: %s", file);
			}
		} else {
			blog(LOG_WARNING,
			     "WIC: Only handle GUID_WICPixelFormat32bppBGR101010 and GUID_WICPixelFormat64bppRGBAHalf for now");
		}
	} else {
		blog(LOG_WARNING, "WIC: Failed to GetPixelFormat for file: %s", file);
	}

	return bytes;
}

uint8_t *gs_image_host_fallback_load(const char *file, enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out,
				     enum gs_color_space *space)
{
	const size_t len = strlen(file);
	if (gs_image_host_fallback_refuses(file, len)) {
		blog(LOG_WARNING, "WIC: Only handle JXR for WIC images for now");
		return NULL;
	}

	BYTE *bytes = NULL;

	wchar_t *file_w = NULL;
	os_utf8_to_wcs_ptr(file, 0, &file_w);
	if (file_w) {
		IWICImagingFactory *pFactory = NULL;
		HRESULT hr = CoCreateInstance(&CLSID_WICImagingFactory, NULL, CLSCTX_INPROC_SERVER,
					      &IID_IWICImagingFactory, &pFactory);
		if (SUCCEEDED(hr)) {
			IWICBitmapDecoder *pDecoder = NULL;
			hr = pFactory->lpVtbl->CreateDecoderFromFilename(pFactory, file_w, NULL, GENERIC_READ,
									 WICDecodeMetadataCacheOnDemand, &pDecoder);
			if (SUCCEEDED(hr)) {
				IWICBitmapFrameDecode *pFrame = NULL;
				hr = pDecoder->lpVtbl->GetFrame(pDecoder, 0, &pFrame);
				if (SUCCEEDED(hr)) {
					bytes = wic_image_init_internal(file, pFrame, format, cx_out, cy_out, space);

					pFrame->lpVtbl->Release(pFrame);
				} else {
					blog(LOG_WARNING, "WIC: Failed to create IWICBitmapFrameDecode from file: %s",
					     file);
				}

				pDecoder->lpVtbl->Release(pDecoder);
			} else {
				blog(LOG_WARNING, "WIC: Failed to create IWICBitmapDecoder from file: %s", file);
			}

			pFactory->lpVtbl->Release(pFactory);
		} else {
			blog(LOG_WARNING, "WIC: Failed to create IWICImagingFactory");
		}

		bfree(file_w);
	} else {
		blog(LOG_WARNING, "WIC: Failed to widen file name: %s", file);
	}

	return bytes;
}
