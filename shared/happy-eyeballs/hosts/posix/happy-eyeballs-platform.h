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

/* Platform layer for happy-eyeballs.c on POSIX systems (BSD sockets). */
#pragma once

#include <fcntl.h>
#include <netdb.h>
#include <sys/socket.h>
#include <netinet/in.h>
#define GetSockError() errno
#define GetAddrInfoErrorString(err) gai_strerror(err)
#define closesocket(s) close(s)

/* Only Windows needs a DNS sandbox probe (COMODO). */
static inline int he_platform_dns_blocked(const char **message)
{
	(void)message;
	return 0;
}

/* Close-on-exec, so a process OBS starts while connected (a recording's
 * muxer) gets no copy of the connection. macOS has no SOCK_CLOEXEC. */
static inline SOCKET he_platform_socket(int family)
{
#ifdef SOCK_CLOEXEC
	return socket(family, SOCK_STREAM | SOCK_CLOEXEC, IPPROTO_TCP);
#else
	SOCKET fd = socket(family, SOCK_STREAM, IPPROTO_TCP);
	if (fd >= 0)
		fcntl(fd, F_SETFD, FD_CLOEXEC);
	return fd;
#endif
}

/* BSD and macOS raise SIGPIPE on a write to a closed socket unless told
 * not to; Linux has no SO_NOSIGPIPE. */
static inline void he_platform_socket_nosigpipe(SOCKET fd)
{
#ifdef SO_NOSIGPIPE
	setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &(int){1}, sizeof(int));
#else
	(void)fd;
#endif
}
