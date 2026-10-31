#pragma once

#include <stdbool.h>
#include <util/dstr.h>

/* Built-in services whose plain RTMP ingest servers also accept RTMPS on port
 * 443. rtmp-services connects to these over RTMPS and keeps the plain URL so
 * the frontend can offer it if the encrypted connection fails.
 *
 * Writes the RTMPS form of url (same host and path, port 443) to out and
 * returns true when url is a plain rtmp:// URL on the default port and its host
 * is in the list for service. Returns false and leaves out untouched
 * otherwise. */
bool rtmps_upgrade_url(const char *service, const char *url, struct dstr *out);
