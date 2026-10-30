/*
 * The Accept-Encoding of OBS's outgoing HTTP calls (file-updater, the
 * frontend's RemoteTextThread). Header-only so the C and C++ call sites share
 * it without a new link dependency.
 */

#pragma once

#include <curl/curl.h>

/* Whether this build keeps the Brotli stop-gap (#2211): the Apple frontend
 * links the OS libcurl, whose Brotli decoder this project neither pins nor
 * measures, so those builds keep asking for gzip and deflate only. Every
 * other platform ships a decoder this project knows: the AppImage bundles
 * brotli 1.2.1 and the Windows package links no brotli at all, so "" (every
 * coding libcurl was built with) is safe there.
 *
 * Measured on the system libcurl (probe on 2026-10-30, #2907): macOS 14.8.9
 * (23J631), 15.8.2 (24H232) and 26.6.2 (25G83) all resolve
 * /usr/lib/libcurl.4.dylib at 8.7.1 with features 0x514f039d, which does not
 * set CURL_VERSION_BROTLI and links no libbrotlidec, and their /usr/bin/curl
 * lists the same features without brotli. So on those releases this branch is
 * inert: gzip and deflate are exactly what "" would ask for. It stays because
 * 13.0 (the deployment target) is the oldest release this project ships and
 * has not been measured: GitHub retired its macos-13 runners, so 13.0 needs a
 * real Mac before the carve-out can go.
 *
 * Set by the build (`OBS_CURL_ACCEPT_ENCODING_UNPINNED_BROTLI`, see the
 * top-level CMakeLists) rather than by a host conditional here: shared C has
 * to stay free of those (#847). */
#ifndef OBS_CURL_ACCEPT_ENCODING_UNPINNED_BROTLI
#define OBS_CURL_ACCEPT_ENCODING_UNPINNED_BROTLI 0
#endif

/* The CURLOPT_ACCEPT_ENCODING value for the libcurl described by info, given
 * whether this is the build whose Brotli decoder is not pinned by this repo
 * (unpinned_brotli).
 *
 * "" asks for every coding the linked libcurl decodes, br included. That is
 * what the call sites want from #2211 on, where the shipped Brotli is past
 * CVE-2026-111497 (GHSA-c2vc-wwm8-6gx9).
 *
 * The Apple branch keeps the stop-gap: gzip and deflate when that libcurl can
 * decode them, else identity.
 *
 * Never gzip on a libcurl without zlib (Windows' Schannel build): it cannot
 * decode the answer and fails the transfer with CURLE_BAD_CONTENT_ENCODING
 * (#2453). identity, not NULL: with no Accept-Encoding at all a server may
 * pick any coding. */
static inline const char *obs_curl_accept_encoding_for(const curl_version_info_data *info, int unpinned_brotli)
{
	if (!unpinned_brotli)
		return "";

	return info && (info->features & CURL_VERSION_LIBZ) ? "gzip, deflate" : "identity";
}

static inline const char *obs_curl_accept_encoding(const curl_version_info_data *info)
{
	return obs_curl_accept_encoding_for(info, OBS_CURL_ACCEPT_ENCODING_UNPINNED_BROTLI);
}
