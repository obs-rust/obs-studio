#include "plugins/obs-outputs/build/obs-outputs-net-if-host-posix.pch.h"

/* obs-outputs' network interface macOS and Linux host (#851): inet_pton/inet_ntop and getifaddrs. */

#define do_log(level, format, ...) blog(level, "[net if] " format, ##__VA_ARGS__)

#define warn(format, ...) do_log(LOG_WARNING, format, ##__VA_ARGS__)

void netif_host_addr_to_string(char *dest, struct sockaddr_storage *byte_address)
{
	int family = byte_address->ss_family;
	char temp_char[INET6_ADDRSTRLEN] = {0};

	if (family == AF_INET)
		inet_ntop(family, &(((struct sockaddr_in *)byte_address)->sin_addr), temp_char, INET6_ADDRSTRLEN);
	else if (family == AF_INET6)
		inet_ntop(family, &(((struct sockaddr_in6 *)byte_address)->sin6_addr), temp_char, INET6_ADDRSTRLEN);
	strncpy(dest, temp_char, INET6_ADDRSTRLEN);
}

bool netif_host_str_to_addr(struct sockaddr_storage *out, int *addr_len, const char *addr, bool ipv6)
{
	*addr_len = ipv6 ? sizeof(struct sockaddr_in6) : sizeof(struct sockaddr_in);

	void *dst = NULL;
	if (ipv6)
		dst = &((struct sockaddr_in6 *)out)->sin6_addr;
	else
		dst = &((struct sockaddr_in *)out)->sin_addr;

	if (inet_pton(out->ss_family, addr, dst))
		return true;

	return false;
}

static inline bool is_loopback(struct ifaddrs *ifa)
{
	const char *n = ifa->ifa_name;
	return n && (strcmp(n, "lo") == 0 || strcmp(n, "lo0") == 0);
}

static inline void netif_get_addrs_nix(struct netif_saddr_data *ifaddrs)
{
	struct ifaddrs *ifaddr, *ifa;
	unsigned int family;
	int s;
	char host[NI_MAXHOST];

	if (getifaddrs(&ifaddr) == -1) {
		warn("getifaddrs() failed");
		return;
	}

	for (ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
		if (ifa->ifa_addr == NULL || is_loopback(ifa))
			continue;

		family = ifa->ifa_addr->sa_family;

		if ((family == AF_INET) || (family == AF_INET6)) {
			s = getnameinfo(ifa->ifa_addr,
					(family == AF_INET) ? sizeof(struct sockaddr_in) : sizeof(struct sockaddr_in6),
					host, NI_MAXHOST, NULL, 0, NI_NUMERICHOST);
			if (s != 0) {
				warn("getnameinfo() failed: %s", gai_strerror(s));
				continue;
			}

			netif_push(ifa->ifa_addr, ifaddrs, ifa->ifa_name);
		}
	}

	freeifaddrs(ifaddr);
}

void netif_host_get_addrs(struct netif_saddr_data *ifaddrs)
{
	netif_get_addrs_nix(ifaddrs);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef do_log
#undef warn
