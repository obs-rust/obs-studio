#include "plugins/obs-qsv11/build/obs-qsv11-probe.pch.h"

bool qsv_probe_adapter_from_facts(const struct qsv_probe_facts *facts, struct qsv_probe_adapter *out)
{
	memset(out, 0, sizeof(*out));
	if (!facts->opened) {
		return false;
	}
	/* QSV runs on the iHD media driver; the legacy intel-vaapi-driver
	 * cannot back a VPL session. */
	if (strstr(facts->driver, "Intel i965 driver") != NULL) {
		return false;
	}
	out->is_intel = strstr(facts->driver, "Intel") != NULL;
	out->supports_hevc = facts->hevc;
	out->supports_av1 = facts->av1;
	return true;
}

void qsv_probe_codecs_from_adapters(const struct qsv_probe_adapter *adapters, size_t count,
				    struct qsv_probe_codecs *out)
{
	memset(out, 0, sizeof(*out));
	for (size_t i = 0; i < count; i++) {
		out->avc |= adapters[i].is_intel;
		out->hevc |= adapters[i].is_intel && adapters[i].supports_hevc;
		out->av1 |= adapters[i].is_intel && adapters[i].supports_av1;
	}
}

void qsv_probe_format(const struct qsv_probe_facts *facts, char *buf, size_t size)
{
	snprintf(buf, size, "node=%s opened=%d hevc=%d av1=%d driver=%s", facts->node, facts->opened ? 1 : 0,
		 facts->hevc ? 1 : 0, facts->av1 ? 1 : 0, facts->driver);
}

/* Reads "<key>=<0|1> " at *p. */
static bool qsv_probe_parse_flag(const char **p, const char *key, bool *out)
{
	size_t n = strlen(key);
	if (strncmp(*p, key, n) != 0 || (*p)[n] != '=' || ((*p)[n + 1] != '0' && (*p)[n + 1] != '1') ||
	    (*p)[n + 2] != ' ') {
		return false;
	}
	*out = (*p)[n + 1] == '1';
	*p += n + 3;
	return true;
}

bool qsv_probe_parse(const char *line, struct qsv_probe_facts *out)
{
	const char *p = strstr(line, QSV_PROBE_LOG_PREFIX);
	const char *end;
	size_t n;

	memset(out, 0, sizeof(*out));
	if (!p) {
		return false;
	}
	p += strlen(QSV_PROBE_LOG_PREFIX);

	if (strncmp(p, "node=", 5) != 0) {
		return false;
	}
	p += 5;
	end = strchr(p, ' ');
	if (!end || (size_t)(end - p) >= sizeof(out->node)) {
		return false;
	}
	memcpy(out->node, p, (size_t)(end - p));
	p = end + 1;

	if (!qsv_probe_parse_flag(&p, "opened", &out->opened) || !qsv_probe_parse_flag(&p, "hevc", &out->hevc) ||
	    !qsv_probe_parse_flag(&p, "av1", &out->av1)) {
		return false;
	}
	if (strncmp(p, "driver=", 7) != 0) {
		return false;
	}
	p += 7;
	/* To the end of the line, without a log file's line break. */
	n = strcspn(p, "\r\n");
	if (n >= sizeof(out->driver)) {
		n = sizeof(out->driver) - 1;
	}
	memcpy(out->driver, p, n);
	out->driver[n] = '\0';
	return true;
}
