#include "plugins/obs-outputs/build/obs-outputs-net-if-host-windows.pch.h"

/* obs-outputs' network interface Windows host (#851): WSAStringToAddressA/InetNtopA and GetAdaptersAddresses. */

#define do_log(level, format, ...) blog(level, "[net if] " format, ##__VA_ARGS__)

#define warn(format, ...) do_log(LOG_WARNING, format, ##__VA_ARGS__)

void netif_host_addr_to_string(char *dest, struct sockaddr_storage *byte_address)
{
	int family = byte_address->ss_family;
	char temp_char[INET6_ADDRSTRLEN] = {0};

	if (family == AF_INET)
		InetNtopA(family, &(((SOCKADDR_IN *)byte_address)->sin_addr), temp_char, INET6_ADDRSTRLEN);
	else if (family == AF_INET6)
		InetNtopA(family, &(((SOCKADDR_IN6 *)byte_address)->sin6_addr), temp_char, INET6_ADDRSTRLEN);
	strncpy(dest, temp_char, INET6_ADDRSTRLEN);
}

bool netif_host_str_to_addr(struct sockaddr_storage *out, int *addr_len, const char *addr, bool ipv6)
{
	UNUSED_PARAMETER(ipv6);

	int ret = WSAStringToAddressA((LPSTR)addr, out->ss_family, NULL, (LPSOCKADDR)out, addr_len);
	if (ret == SOCKET_ERROR)
		warn("Could not parse address, error code: %d", GetLastError());
	return ret != SOCKET_ERROR;
}

static inline PIP_ADAPTER_ADDRESSES get_adapters(void)
{
	PIP_ADAPTER_ADDRESSES adapter = NULL;
	unsigned long ret = 0;
	unsigned long out_buf_len = 16384;
	unsigned long flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
	const int max_tries = 3;
	int i = 0;

	do {
		adapter = (IP_ADAPTER_ADDRESSES *)bmalloc(out_buf_len);
		if (!adapter)
			return NULL;

		ret = GetAdaptersAddresses(AF_UNSPEC, flags, NULL, adapter, &out_buf_len);
		if (ret == ERROR_BUFFER_OVERFLOW) {
			bfree(adapter);
			adapter = NULL;
		} else {
			break;
		}
		i++;
	} while ((ret == ERROR_BUFFER_OVERFLOW) && (i < max_tries));

	if (ret != NO_ERROR && ret != ERROR_NO_DATA) {
		LPSTR msg_buf = NULL;

		bfree(adapter);
		adapter = NULL;

		FormatMessageA(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM |
				       FORMAT_MESSAGE_IGNORE_INSERTS,
			       NULL, ret, MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), (LPSTR)&msg_buf, 0, NULL);
		if (msg_buf) {
			warn("Call to GetAdaptersAddresses failed: %s (%d)", msg_buf, ret);
			LocalFree(msg_buf);
		}
	}

	return adapter;
}

static inline void netif_get_addrs_win32(struct netif_saddr_data *ifaddrs)
{
	PIP_ADAPTER_ADDRESSES adapter = get_adapters();
	PIP_ADAPTER_UNICAST_ADDRESS unicast = NULL;
	PIP_ADAPTER_ADDRESSES cur_adap = NULL;
	SOCKET_ADDRESS socket_addr;
	int family;

	if (!adapter)
		return;

	for (cur_adap = adapter; !!cur_adap; cur_adap = cur_adap->Next) {
		char *adap_name = NULL;

		if (cur_adap->OperStatus != IfOperStatusUp || cur_adap->IfType == IF_TYPE_SOFTWARE_LOOPBACK)
			continue;

		os_wcs_to_utf8_ptr(cur_adap->FriendlyName, 0, &adap_name);

		unicast = cur_adap->FirstUnicastAddress;

		for (; !!unicast; unicast = unicast->Next) {
			socket_addr = unicast->Address;

			family = socket_addr.lpSockaddr->sa_family;
			if (family == AF_INET || family == AF_INET6)
				netif_push(socket_addr.lpSockaddr, ifaddrs, adap_name);
		}

		bfree(adap_name);
	}

	bfree(adapter);
}

void netif_host_get_addrs(struct netif_saddr_data *ifaddrs)
{
	netif_get_addrs_win32(ifaddrs);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef do_log
#undef warn
