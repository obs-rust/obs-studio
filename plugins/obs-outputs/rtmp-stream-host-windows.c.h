#include "plugins/obs-outputs/build/obs-outputs-windows.pch.h"

/* The RTMP output's Windows host (#851): WSA errors and ioctlsocket, the
 * interface log and the send thread ("new socket loop"). Part of the
 * obs-outputs.windows unity group with rtmp-windows.c. */

int rtmp_host_last_socket_error(void)
{
	return WSAGetLastError();
}

int rtmp_host_bytes_available(uintptr_t sock, int *size)
{
	return ioctlsocket((SOCKET)sock, FIONREAD, (u_long *)size);
}

int rtmp_host_set_nonblocking(uintptr_t sock)
{
	u_long one = 1;
	/* Winsock encodes the command in 32 bits, including the sign bit. */
	if (ioctlsocket((SOCKET)sock, (long)FIONBIO, &one))
		return WSAGetLastError();
	return 0;
}

const char *rtmp_host_socket_error_text(int code)
{
	const char *msg = NULL;
	switch (code) {
	case WSAETIMEDOUT:
		msg = obs_module_text("ConnectionTimedOut");
		break;
	case WSAEACCES:
		msg = obs_module_text("PermissionDenied");
		break;
	case WSAECONNABORTED:
		msg = obs_module_text("ConnectionAborted");
		break;
	case WSAECONNRESET:
		msg = obs_module_text("ConnectionReset");
		break;
	case WSAHOST_NOT_FOUND:
		msg = obs_module_text("HostNotFound");
		break;
	case WSANO_DATA:
		msg = obs_module_text("NoData");
		break;
	case WSAEADDRNOTAVAIL:
		msg = obs_module_text("AddressNotAvailable");
		break;
	case WSAEINVAL:
		msg = obs_module_text("InvalidParameter");
		break;
	case WSAEHOSTUNREACH:
		msg = obs_module_text("NoRoute");
		break;
	}
	return msg;
}

const char *rtmp_host_root_cert_error(void)
{
	return "Failed to load root certificates for a secure TLS connection.";
}

#define socklen_t int

void rtmp_host_log_sndbuf_size(struct rtmp_stream *stream)
{
	int cur_sendbuf_size;
	socklen_t int_size = sizeof(int);

	if (!getsockopt(stream->rtmp.m_sb.sb_socket, SOL_SOCKET, SO_SNDBUF, (char *)&cur_sendbuf_size, &int_size)) {
		info("Socket send buffer is %d bytes", cur_sendbuf_size);
	}
}

void rtmp_host_log_interface_type(struct rtmp_stream *stream)
{
	RTMP *rtmp = &stream->rtmp;
	MIB_IPFORWARDROW route;
	uint32_t dest_addr, source_addr;
	char hostname[256];
	HOSTENT *h;

	if ((size_t)rtmp->Link.hostname.av_len >= sizeof(hostname) - 1)
		return;

	strncpy(hostname, rtmp->Link.hostname.av_val, sizeof(hostname));
	hostname[rtmp->Link.hostname.av_len] = 0;

	h = gethostbyname(hostname);
	if (!h)
		return;

	dest_addr = *(uint32_t *)h->h_addr_list[0];

	if (rtmp->m_bindIP.addrLen == 0)
		source_addr = 0;
	else if (rtmp->m_bindIP.addr.ss_family == AF_INET)
		source_addr = (*(struct sockaddr_in *)&rtmp->m_bindIP.addr).sin_addr.S_un.S_addr;
	else
		return;

	if (!GetBestRoute(dest_addr, source_addr, &route)) {
		MIB_IF_ROW2 row;
		memset(&row, 0, sizeof(row));
		row.InterfaceIndex = route.dwForwardIfIndex;

		if (!GetIfEntry2(&row)) {
			/* Preserve the log's existing low 32 bits, even for oversized SDK values. */
			uint32_t rxSpeed = (uint32_t)((row.ReceiveLinkSpeed / 1000000) & UINT32_MAX);
			uint32_t txSpeed = (uint32_t)((row.TransmitLinkSpeed / 1000000) & UINT32_MAX);
			char *type;
			struct dstr other = {0};

			switch (row.PhysicalMediumType) {
			case NdisPhysicalMedium802_3:
				type = "ethernet";
				break;
			case NdisPhysicalMediumWirelessLan:
			case NdisPhysicalMediumNative802_11:
				type = "802.11";
				break;
			default:
				dstr_printf(&other, "type %d", (int)row.PhysicalMediumType);
				type = other.array;
				break;
			}

			char *desc;
			os_wcs_to_utf8_ptr(row.Description, 0, &desc);

			info("Interface: %s (%s, %lu↓/%lu↑ mbps)", desc, type, rxSpeed, txSpeed);

			bfree(desc);

			if (row.InErrors || row.OutErrors) {
				warn("Interface has non-zero error counters (%" PRIu64 "/%" PRIu64 " errors, %" PRIu64
				     "/%" PRIu64 " discards)",
				     row.InErrors, row.OutErrors, row.InDiscards, row.OutDiscards);
			}

			dstr_free(&other);
		}
	}
}

static int socket_queue_data(RTMPSockBuf *sb, const char *data, int len, void *arg)
{
	UNUSED_PARAMETER(sb);

	struct rtmp_stream *stream = arg;
	/* librtmp WriteN calls custom send only while its int byte count is positive. */
	size_t size = (size_t)len;

retry_send:

	if (!RTMP_IsConnected(&stream->rtmp))
		return 0;

	pthread_mutex_lock(&stream->write_buf_mutex);

	if (stream->write_buf_len + size > stream->write_buf_size) {

		pthread_mutex_unlock(&stream->write_buf_mutex);

		if (os_event_wait(stream->buffer_space_available_event)) {
			return 0;
		}

		goto retry_send;
	}

	memcpy(stream->write_buf + stream->write_buf_len, data, size);
	stream->write_buf_len += size;

	pthread_mutex_unlock(&stream->write_buf_mutex);

	os_event_signal(stream->buffer_has_data_event);

	return len;
}

bool rtmp_host_has_socket_loop(void)
{
	return true;
}

bool rtmp_host_start_socket_loop(struct rtmp_stream *stream)
{
	int ret;

	ret = pthread_create(&stream->socket_thread, NULL, socket_thread_windows, stream);

	if (ret != 0) {
		RTMP_Close(&stream->rtmp);
		warn("Failed to create socket thread");
		return false;
	}

	stream->socket_thread_active = true;
	stream->rtmp.m_bCustomSend = true;
	stream->rtmp.m_customSendFunc = socket_queue_data;
	stream->rtmp.m_customSendParam = stream;
	return true;
}

#undef socklen_t
