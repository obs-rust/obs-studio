/******************************************************************************
    Copyright (C) 2016 B. Lee <bl4@postpile.net>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "plugins/obs-outputs/build/obs-outputs-net-if.pch.h"

#define do_log(level, format, ...) blog(level, "[net if] " format, ##__VA_ARGS__)

#define warn(format, ...) do_log(LOG_WARNING, format, ##__VA_ARGS__)
#define info(format, ...) do_log(LOG_INFO, format, ##__VA_ARGS__)
#define debug(format, ...) do_log(LOG_DEBUG, format, ##__VA_ARGS__)

static inline void netif_saddr_data_push_back(struct netif_saddr_data *sd, const char *ip, const char *adapter)
{
	struct netif_saddr_item item;
	struct dstr full_name = {0};
	char *ip_dup = bstrdup(ip);

	if (adapter && *adapter)
		dstr_printf(&full_name, "[%s] %s", adapter, ip);
	else
		dstr_copy(&full_name, ip);

	item.name = full_name.array;
	item.addr = ip_dup;

	da_push_back(sd->addrs, &item);
}

void netif_push(struct sockaddr *copy_source, struct netif_saddr_data *saddr_d, const char *adapter)
{
	char temp_char[INET6_ADDRSTRLEN] = {0};
	struct sockaddr_storage sa = {0};

	if (copy_source->sa_family == AF_INET)
		memcpy(&sa, copy_source, sizeof(struct sockaddr_in));
	else if (copy_source->sa_family == AF_INET6)
		memcpy(&sa, copy_source, sizeof(struct sockaddr_in6));

	netif_host_addr_to_string(temp_char, &sa);
	netif_saddr_data_push_back(saddr_d, temp_char, adapter);
}

void netif_log_saddrs(struct netif_saddr_data *sd)
{
	for (size_t i = 0; i < sd->addrs.num; i++)
		info("\t\t%s", sd->addrs.array[i].name);
}

bool netif_addr_to_str(struct sockaddr_storage *in, char *addr, int addr_len)
{
	if (!in || !addr)
		return false;
	if (in->ss_family != AF_INET6 && in->ss_family != AF_INET)
		return false;
	if ((in->ss_family == AF_INET6 && addr_len < INET6_ADDRSTRLEN) ||
	    (in->ss_family == AF_INET && addr_len < INET_ADDRSTRLEN))
		return false;

	/* addr_len >= INET6_ADDRSTRLEN here, so it is positive */
	memset(addr, 0, (size_t)addr_len);
	netif_host_addr_to_string(addr, in);
	return true;
}

bool netif_str_to_addr(struct sockaddr_storage *out, int *addr_len, const char *addr)
{
	bool ipv6;

	memset(out, 0, sizeof(*out));
	*addr_len = 0;

	if (!addr)
		return false;

	ipv6 = (strchr(addr, ':') != NULL);
	out->ss_family = ipv6 ? AF_INET6 : AF_INET;
	*addr_len = sizeof(*out);

	return netif_host_str_to_addr(out, addr_len, addr, ipv6);
}

void netif_get_addrs(struct netif_saddr_data *ifaddrs)
{
	da_init(ifaddrs->addrs);

	netif_host_get_addrs(ifaddrs);
}

/* File-local macros: keep them out of the next source of a unity build. */
#undef do_log

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef warn
#undef info
#undef debug
