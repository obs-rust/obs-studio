#include "plugins/rtmp-services/build/rtmp-services.pch.h"

/* Ingest hosts that answered on port 443 (except where noted) with a valid certificate for the
 * host name, completed the RTMP handshake and connect over TLS, and rejected a
 * publish with a made-up stream key the same way as over plain RTMP. Checked
 * on 2026-10-24 for every server of every built-in RTMP service and every
 * server in Twitch's ingest list (obs-rust/obs-studio#2205) with
 * tools/rtmps-endpoint-check.py. A host that is not here stays on plain RTMP. */
static const struct {
	const char *service;
	const char *host;
} rtmps_hosts[] = {
	{"Bitmovin", "live-input.bitmovin.com"},
	{"BoxCast", "rtmp.boxcast.com"},
	{"Eventials", "transmission.eventials.com"},
	{"EventLive.pro", "go.eventlive.pro"},
	/* nanoStream uses port 1937: https://docs.nanocosmos.de/docs/cloud/security
	 * Both global hosts passed the same checker on 2026-11-07 (#2216): verified
	 * TLS, connect success, identical fake-key rejection on plain/TLS. Regional
	 * hosts have not been verified and must not be added by suffix matching. */
	{"nanoStream Cloud / bintu", "bintu-stream.nanocosmos.de"},
	{"nanoStream Cloud / bintu", "bintu-vtrans.nanocosmos.de"},
	/* Restream's encoder guide shows live.restream.io:1937. Verified on
	 * 2026-11-07: TLSv1.3, RTMP connect, same fake-key response as plain RTMP.
	 * https://support.restream.io/en/articles/8523770-encrypt-your-stream-with-rtmps
	 * Regional hosts have not been verified and stay unchanged (#2216). */
	{"Restream.io", "live.restream.io"},
	{"niconico (ニコニコ生放送)", "liveorigin.dlive.nicovideo.jp"},
	{"Twitch", "apn10.contribute.live-video.net"},
	{"Twitch", "apn20.contribute.live-video.net"},
	{"Twitch", "aps10.contribute.live-video.net"},
	{"Twitch", "aps20.contribute.live-video.net"},
	{"Twitch", "aps30.contribute.live-video.net"},
	{"Twitch", "euc10.contribute.live-video.net"},
	{"Twitch", "eun10.contribute.live-video.net"},
	{"Twitch", "euw10.contribute.live-video.net"},
	{"Twitch", "euw30.contribute.live-video.net"},
	{"Twitch", "ingest.global-contribute.live-video.net"},
	{"Twitch", "live-ams.twitch.tv"},
	{"Twitch", "live-arn.twitch.tv"},
	{"Twitch", "live-atl.twitch.tv"},
	{"Twitch", "live-ber.twitch.tv"},
	{"Twitch", "live-cdg.twitch.tv"},
	{"Twitch", "live-cph.twitch.tv"},
	{"Twitch", "live-den.twitch.tv"},
	{"Twitch", "live-dfw.twitch.tv"},
	{"Twitch", "live-eze.twitch.tv"},
	{"Twitch", "live-fra.twitch.tv"},
	{"Twitch", "live-hel.twitch.tv"},
	{"Twitch", "live-hkg.twitch.tv"},
	{"Twitch", "live-hou.twitch.tv"},
	{"Twitch", "live-iad.twitch.tv"},
	{"Twitch", "live-jfk.twitch.tv"},
	{"Twitch", "live-lax.twitch.tv"},
	{"Twitch", "live-lhr.twitch.tv"},
	{"Twitch", "live-lim.twitch.tv"},
	{"Twitch", "live-lis.twitch.tv"},
	{"Twitch", "live-mad.twitch.tv"},
	{"Twitch", "live-mde.twitch.tv"},
	{"Twitch", "live-mia.twitch.tv"},
	{"Twitch", "live-mil.twitch.tv"},
	{"Twitch", "live-mrs.twitch.tv"},
	{"Twitch", "live-ord.twitch.tv"},
	{"Twitch", "live-osl.twitch.tv"},
	{"Twitch", "live-pdx.twitch.tv"},
	{"Twitch", "live-phx.twitch.tv"},
	{"Twitch", "live-prg.twitch.tv"},
	{"Twitch", "live-qro.twitch.tv"},
	{"Twitch", "live-rio.twitch.tv"},
	{"Twitch", "live-sao.twitch.tv"},
	{"Twitch", "live-scl.twitch.tv"},
	{"Twitch", "live-sea.twitch.tv"},
	{"Twitch", "live-sel.twitch.tv"},
	{"Twitch", "live-sfo.twitch.tv"},
	{"Twitch", "live-sin.twitch.tv"},
	{"Twitch", "live-sjc.twitch.tv"},
	{"Twitch", "live-slc.twitch.tv"},
	{"Twitch", "live-syd.twitch.tv"},
	{"Twitch", "live-tpe.twitch.tv"},
	{"Twitch", "live-tyo.twitch.tv"},
	{"Twitch", "live-vie.twitch.tv"},
	{"Twitch", "live-waw.twitch.tv"},
	{"Twitch", "live-ymq.twitch.tv"},
	{"Twitch", "live-yto.twitch.tv"},
	{"Twitch", "live.twitch.tv"},
	{"Twitch", "sae10.contribute.live-video.net"},
	{"Twitch", "use10.contribute.live-video.net"},
	{"Twitch", "use20.contribute.live-video.net"},
	{"Twitch", "usw20.contribute.live-video.net"},
	{"Vimeo", "rtmp.cloud.vimeo.com"},
};

#define UPGRADE_FROM_SCHEME "rtmp://"
#define UPGRADE_FROM_PORT "1935"

static bool rtmps_host_listed(const char *service, const char *host, size_t host_len)
{
	for (size_t i = 0; i < sizeof(rtmps_hosts) / sizeof(rtmps_hosts[0]); i++) {
		if (strcmp(rtmps_hosts[i].service, service) == 0 && strlen(rtmps_hosts[i].host) == host_len &&
		    astrcmpi_n(rtmps_hosts[i].host, host, host_len) == 0)
			return true;
	}
	return false;
}

bool rtmps_upgrade_url(const char *service, const char *url, struct dstr *out)
{
	if (!service || !url || astrcmpi_n(url, UPGRADE_FROM_SCHEME, sizeof(UPGRADE_FROM_SCHEME) - 1) != 0)
		return false;

	const char *host = url + sizeof(UPGRADE_FROM_SCHEME) - 1;
	size_t host_len = strcspn(host, ":/?");
	const char *rest = host + host_len;

	/* Only the default RTMP port maps to the verified RTMPS port. */
	if (*rest == ':') {
		const char *port = rest + 1;
		size_t port_len = strcspn(port, "/?");
		if (port_len != sizeof(UPGRADE_FROM_PORT) - 1 ||
		    strncmp(port, UPGRADE_FROM_PORT, sizeof(UPGRADE_FROM_PORT) - 1) != 0)
			return false;
		rest = port + port_len;
	}

	if (!host_len || !rtmps_host_listed(service, host, host_len))
		return false;

	dstr_copy(out, "rtmps://");
	dstr_ncat(out, host, host_len);
	dstr_cat(out, strcmp(service, "nanoStream Cloud / bintu") == 0 || strcmp(service, "Restream.io") == 0 ? ":1937"
													      : ":443");
	dstr_cat(out, rest);
	return true;
}

#undef UPGRADE_FROM_SCHEME
#undef UPGRADE_FROM_PORT
