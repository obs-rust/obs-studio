#include "plugins/obs-qsv11/build/obs-qsv11-probe.pch.h"

bool qsv_probe_adapter_from_facts(const struct qsv_probe_facts *facts, struct qsv_probe_adapter *out)
{
	(void)facts;
	memset(out, 0, sizeof(*out));
	return false;
}

void qsv_probe_codecs_from_adapters(const struct qsv_probe_adapter *adapters, size_t count,
				    struct qsv_probe_codecs *out)
{
	(void)adapters;
	(void)count;
	memset(out, 0, sizeof(*out));
}

void qsv_probe_format(const struct qsv_probe_facts *facts, char *buf, size_t size)
{
	(void)facts;
	if (size) {
		buf[0] = '\0';
	}
}

bool qsv_probe_parse(const char *line, struct qsv_probe_facts *out)
{
	(void)line;
	memset(out, 0, sizeof(*out));
	return false;
}
