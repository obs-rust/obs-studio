#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# ///
"""Check whether RTMP ingest servers also work over RTMPS, without an account.

plugins/rtmp-services/service-specific/rtmps-upgrade.c.h lists the hosts that
pass. Rerun this when services.json or Twitch's ingest list changes, and add
a host to that list only when every server of its service passes.

For each server URL this runs, over plain RTMP and over RTMPS (same host and
app path, port 443 unless the URL says otherwise):
  DNS + TCP, TLS with certificate/hostname verification (RTMPS only),
  RTMP handshake, connect(app), then publish with a fake key.
No media is ever sent. A server "matches" when RTMPS gets as far as plain RTMP
and the fake-key publish is answered the same way.

Usage:
  uv run tools/rtmps-endpoint-check.py plugins/rtmp-services/data/services.json \
      --twitch-feed --twitch-list > results.jsonl

Each output line is one server: its verdict (pass, fail, check, untestable)
and how far plain RTMP and RTMPS got.
"""
import argparse, json, os, socket, ssl, struct, sys, time
from urllib.parse import urlsplit

FAKE_KEY = "rtmps-endpoint-check-invalid-key"
TIMEOUT = 8


# --- AMF0 -------------------------------------------------------------------
def amf_enc(v):
    if v is None:
        return b"\x05"
    if isinstance(v, bool):
        return b"\x01" + (b"\x01" if v else b"\x00")
    if isinstance(v, (int, float)):
        return b"\x00" + struct.pack(">d", float(v))
    if isinstance(v, str):
        b = v.encode()
        return b"\x02" + struct.pack(">H", len(b)) + b
    if isinstance(v, dict):
        out = b"\x03"
        for k, x in v.items():
            kb = k.encode()
            out += struct.pack(">H", len(kb)) + kb + amf_enc(x)
        return out + b"\x00\x00\x09"
    raise TypeError(v)


def amf_dec(b, i=0):
    t = b[i]
    i += 1
    if t == 0x00:
        return struct.unpack(">d", b[i:i + 8])[0], i + 8
    if t == 0x01:
        return bool(b[i]), i + 1
    if t == 0x02:
        n = struct.unpack(">H", b[i:i + 2])[0]
        return b[i + 2:i + 2 + n].decode("utf-8", "replace"), i + 2 + n
    if t in (0x03, 0x08):
        if t == 0x08:
            i += 4
        o = {}
        while True:
            n = struct.unpack(">H", b[i:i + 2])[0]
            if n == 0 and b[i + 2] == 0x09:
                return o, i + 3
            k = b[i + 2:i + 2 + n].decode("utf-8", "replace")
            o[k], i = amf_dec(b, i + 2 + n)
    if t in (0x05, 0x06):
        return None, i
    raise ValueError("amf type %d" % t)


def amf_all(b):
    out, i = [], 0
    while i < len(b):
        try:
            v, i = amf_dec(b, i)
        except Exception:
            break
        out.append(v)
    return out


# --- RTMP chunk stream ------------------------------------------------------
class Conn:
    def __init__(self, sock):
        self.s = sock
        self.buf = b""
        self.in_chunk = 128
        self.prev = {}

    def recv_exact(self, n):
        while len(self.buf) < n:
            d = self.s.recv(65536)
            if not d:
                raise ConnectionError("closed by server")
            self.buf += d
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def send_msg(self, csid, mtype, payload, msid=0):
        hdr = bytes([csid & 0x3F]) + b"\x00\x00\x00" + struct.pack(">I", len(payload))[1:] + bytes([mtype]) + struct.pack("<I", msid)
        out, first = b"", True
        for off in range(0, len(payload), 128):
            out += (hdr if first else bytes([0xC0 | (csid & 0x3F)])) + payload[off:off + 128]
            first = False
        if not payload:
            out = hdr
        self.s.sendall(out)

    def invoke(self, name, txn, *args, csid=3, msid=0):
        self.send_msg(csid, 20, amf_enc(name) + amf_enc(txn) + b"".join(amf_enc(a) for a in args), msid)

    def read_msg(self):
        partial = {}
        while True:
            b0 = self.recv_exact(1)[0]
            fmt, csid = b0 >> 6, b0 & 0x3F
            if csid == 0:
                csid = 64 + self.recv_exact(1)[0]
            elif csid == 1:
                x = self.recv_exact(2)
                csid = 64 + x[0] + x[1] * 256
            p = dict(self.prev.get(csid, {"ts": 0, "len": 0, "type": 0, "msid": 0}))
            if fmt <= 2:
                ts = int.from_bytes(self.recv_exact(3), "big")
            if fmt <= 1:
                p["len"] = int.from_bytes(self.recv_exact(3), "big")
                p["type"] = self.recv_exact(1)[0]
            if fmt == 0:
                p["msid"] = struct.unpack("<I", self.recv_exact(4))[0]
            if fmt <= 2 and ts == 0xFFFFFF:
                self.recv_exact(4)
            self.prev[csid] = p
            have = partial.get(csid, b"")
            take = min(self.in_chunk, p["len"] - len(have))
            have += self.recv_exact(take)
            if len(have) < p["len"]:
                partial[csid] = have
                continue
            partial.pop(csid, None)
            if p["type"] == 1:
                self.in_chunk = struct.unpack(">I", have[:4])[0] & 0x7FFFFFFF
                continue
            return p["type"], have


def status_of(vals):
    for v in vals:
        if isinstance(v, dict) and "code" in v:
            return v.get("code"), v.get("level"), str(v.get("description", ""))[:160]
    return None, None, None


def attempt(host, port, app, tcurl, use_tls):  # noqa: C901
    r = {"host": host, "port": port, "tls": use_tls, "stage": "dns"}
    try:
        socket.getaddrinfo(host, port)
        r["stage"] = "tcp"
        raw = socket.create_connection((host, port), timeout=TIMEOUT)
        raw.settimeout(TIMEOUT)
        s = raw
        if use_tls:
            r["stage"] = "tls"
            s = ssl.create_default_context().wrap_socket(raw, server_hostname=host)
            r["tls_version"] = s.version()
        c = Conn(s)
        r["stage"] = "handshake"
        c1 = struct.pack(">I", 0) + b"\x00" * 4 + os.urandom(1528)
        s.sendall(b"\x03" + c1)
        s0 = c.recv_exact(1)
        if s0 != b"\x03":
            raise ValueError("S0=%s (not RTMP)" % s0.hex())
        s1 = c.recv_exact(1536)
        c.recv_exact(1536)
        s.sendall(s1)
        r["stage"] = "connect"
        c.invoke("connect", 1, {"app": app, "type": "nonprivate", "flashVer": "FMLE/3.0 (compatible; FMSc/1.0)", "tcUrl": tcurl})
        while True:
            t, body = c.read_msg()
            if t == 20:
                vals = amf_all(body)
                if vals and vals[0] in ("_result", "_error") and vals[1] == 1:
                    code, _, desc = status_of(vals)
                    r["connect"] = code or vals[0]
                    if vals[0] == "_error":
                        r["connect_desc"] = desc
                        return r
                    break
        r["stage"] = "publish"
        c.invoke("releaseStream", 2, None, FAKE_KEY)
        c.invoke("FCPublish", 3, None, FAKE_KEY)
        c.invoke("createStream", 4, None)
        msid = None
        deadline = time.time() + TIMEOUT
        while time.time() < deadline:
            t, body = c.read_msg()
            if t != 20:
                continue
            vals = amf_all(body)
            if not vals:
                continue
            if vals[0] == "_result" and vals[1] == 4 and msid is None:
                msid = int(vals[3]) if len(vals) > 3 and isinstance(vals[3], float) else 1
                c.invoke("publish", 5, None, FAKE_KEY, "live", csid=4, msid=msid)
                continue
            if vals[0] == "_error" and vals[1] == 4:
                r["publish"] = "createStream:_error"
                return r
            if vals[0] in ("onStatus", "onFCPublish"):
                code, level, desc = status_of(vals)
                if code and (vals[0] == "onStatus" or level == "error"):
                    r["publish"] = code
                    r["publish_desc"] = desc
                    return r
        r["publish"] = "timeout"
    except ConnectionError as e:
        r["error"] = str(e)
        if r["stage"] == "publish":
            r["publish"] = "closed"
    except ssl.SSLCertVerificationError as e:
        r["error"] = "cert: " + (e.verify_message or str(e))
    except Exception as e:
        r["error"] = "%s: %s" % (type(e).__name__, e)
    finally:
        try:
            s.close()
        except Exception:
            pass
    return r


def split(url):
    u = urlsplit(url)
    path = u.path.lstrip("/")
    app = path.rstrip("/")
    if u.query:
        app += "?" + u.query
    return u.hostname, u.port, app


def outcome(r):
    if "publish" in r:
        return "publish:" + r["publish"]
    if "connect" in r:
        return "connect:" + r["connect"]
    return "fail@" + r["stage"]


def check(service, url):
    host, port, app = split(url)
    is_tls = url.lower().startswith("rtmps://")
    plain_port = 1935 if is_tls else (port or 1935)
    tls_port = port if is_tls and port else 443
    plain = attempt(host, plain_port, app, "rtmp://%s:%d/%s" % (host, plain_port, app), False)
    time.sleep(1)
    tls = attempt(host, tls_port, app, "rtmps://%s:%d/%s" % (host, tls_port, app), True)
    po, to = outcome(plain), outcome(tls)
    if to.startswith("publish:") and to != "publish:NetStream.Publish.Start" and (po == to or po.startswith("fail@")):
        verdict = "pass"
    elif to == "publish:NetStream.Publish.Start":
        verdict = "check"  # fake key accepted; can't tell what a real key does
    elif po.startswith("fail@") and to.startswith("fail@"):
        verdict = "untestable"
    elif po.startswith("publish:") and to.startswith("publish:"):
        verdict = "check"  # both answer publish, but differently
    else:
        verdict = "fail"
    return {"service": service, "url": url, "verdict": verdict, "plain": po, "rtmps": to, "plain_detail": plain, "rtmps_detail": tls}


def main():  # noqa: C901
    ap = argparse.ArgumentParser()
    ap.add_argument("services")
    ap.add_argument("--twitch-feed", action="store_true")
    ap.add_argument("--extra", nargs="*", default=[], help="NAME=URL")
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--jobs", type=int, default=8, help="services checked in parallel")
    ap.add_argument("--twitch-list", action="store_true", help="with --twitch-feed, also check the Twitch servers in services.json")
    a = ap.parse_args()
    d = json.load(open(a.services))
    jobs = []
    for s in d["services"]:
        if a.only and s["name"] not in a.only:
            continue
        if s["name"] == "Twitch" and a.twitch_feed and not a.twitch_list:
            continue
        for x in s.get("servers", []):
            if x["url"].lower().startswith(("rtmp://", "rtmps://")):
                jobs.append((s["name"], x["url"]))
    if a.twitch_feed:
        import urllib.request
        feed = json.load(urllib.request.urlopen("https://ingest.twitch.tv/ingests", timeout=15))
        for i in feed["ingests"]:
            jobs.append(("Twitch", i["url_template_secure"].replace("/{stream_key}", "")))
        # The built-in fallback when the list can't be loaded (twitch.c.h).
        jobs.append(("Twitch", "rtmp://live.twitch.tv/app"))
    for e in a.extra:
        n, u = e.split("=", 1)
        jobs.append((n, u))
    seen, by_service = set(), {}
    for name, url in jobs:
        key = (name, url.lower().replace("rtmps://", "rtmp://").split("?")[0].rstrip("/"))
        if key in seen:
            continue
        seen.add(key)
        by_service.setdefault(name, []).append(url)

    import threading
    from concurrent.futures import ThreadPoolExecutor
    lock = threading.Lock()

    def run_service(name):
        # Each worker takes one provider and checks its servers in sequence.
        for url in by_service[name]:
            r = check(name, url)
            with lock:
                print(json.dumps(r, ensure_ascii=False), flush=True)
            time.sleep(1)

    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        list(ex.map(run_service, by_service))


if __name__ == "__main__":
    main()
