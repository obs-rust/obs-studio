/**
 * Copyright (c) 2023 Twitch Interactive, Inc.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

/* Platform layer for happy-eyeballs.c on Windows (Winsock). */
#pragma once

#ifdef _MSC_VER
#pragma warning(disable : 4996) /* depricated warnings */
#endif
#include <winsock2.h>
#include <ws2tcpip.h>
#define GetSockError() WSAGetLastError()
/* the narrow form: gai_strerror is the wide one under UNICODE */
#define GetAddrInfoErrorString(err) gai_strerrorA(err)
#ifdef _MSC_VER
#define snprintf _snprintf
#endif
#define SHUT_RDWR SD_BOTH

/* COMODO security software sandbox blocks all DNS by returning "host not
 * found". Returns 0, or the error code with *message set. */
static inline int he_platform_dns_blocked(const char **message)
{
	HOSTENT *h = gethostbyname("localhost");
	if (!h && GetLastError() == WSAHOST_NOT_FOUND) {
		*message = "happy-eyeballs: Connection test failed. "
			   "This error is likely caused by Comodo Internet "
			   "Security running OBS in sandbox mode. Please add "
			   "OBS to the Comodo automatic sandbox exclusion list, "
			   "restart OBS and try again (11001).";
		return WSAHOST_NOT_FOUND;
	}
	return 0;
}

static inline SOCKET he_platform_socket(int family)
{
	return WSASocket(family, SOCK_STREAM, IPPROTO_TCP, NULL, 0, WSA_FLAG_OVERLAPPED);
}

/* Winsock has no SIGPIPE. */
static inline void he_platform_socket_nosigpipe(SOCKET fd)
{
	(void)fd;
}

/* Winsock's bind()/connect() take the address length as an int, while the
 * resolved length is a size_t; the conversion is the one the call already
 * made implicitly, so it is spelled out here rather than at the call sites. */
static inline int he_platform_addr_len(size_t len)
{
	return (int)len;
}
