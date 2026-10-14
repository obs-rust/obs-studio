#!/usr/bin/env -S uv run --script
"""obs-websocket v5 Tier 4 black-box suite.

Language-independent wire-level tests: launches a real OBS process (headless,
xvfb or the platform's existing headless option) with obs-websocket enabled and
asserts on the documented v5 protocol only - Hello/Identify auth handshake
(with and without password), every request type listed in
plugins/obs-websocket/docs, event subscription delivery, batch requests and
error codes. No implementation hooks; the same suite runs unchanged against the
C++ server and the Rust port.

See README.md for the run command.
"""

import argparse
import asyncio
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

import websockets

from obsws import (
    ObsWsClient,
    SUB_ALL,
    SUB_CONFIG,
    SUB_GENERAL,
    SUB_INPUTS,
    SUB_NONE,
    SUB_SCENES,
    SUB_VENDORS,
    STATUS_SUCCESS,
    STATUS_MISSING_REQUEST_TYPE,
    STATUS_UNKNOWN_REQUEST_TYPE,
    STATUS_UNSUPPORTED_BATCH_EXECUTION_TYPE,
    STATUS_MISSING_REQUEST_FIELD,
    STATUS_MISSING_REQUEST_DATA,
    STATUS_INVALID_REQUEST_FIELD,
    STATUS_INVALID_REQUEST_FIELD_TYPE,
    STATUS_REQUEST_FIELD_OUT_OF_RANGE,
    STATUS_REQUEST_FIELD_EMPTY,
    STATUS_TOO_MANY_REQUEST_FIELDS,
    STATUS_OUTPUT_RUNNING,
    STATUS_OUTPUT_NOT_RUNNING,
    STATUS_OUTPUT_PAUSED,
    STATUS_OUTPUT_NOT_PAUSED,
    STATUS_STUDIO_MODE_NOT_ACTIVE,
    STATUS_RESOURCE_NOT_FOUND,
    STATUS_RESOURCE_ALREADY_EXISTS,
    STATUS_INVALID_RESOURCE_TYPE,
    STATUS_INVALID_RESOURCE_STATE,
    STATUS_INVALID_INPUT_KIND,
    STATUS_RESOURCE_NOT_CONFIGURABLE,
    STATUS_INVALID_FILTER_KIND,
    STATUS_RESOURCE_CREATION_FAILED,
    STATUS_RESOURCE_ACTION_FAILED,
    STATUS_REQUEST_PROCESSING_FAILED,
    STATUS_CANNOT_ACT,
    CLOSE_MESSAGE_DECODE_ERROR,
    CLOSE_MISSING_DATA_FIELD,
    CLOSE_INVALID_DATA_FIELD_TYPE,
    CLOSE_INVALID_DATA_FIELD_VALUE,
    CLOSE_UNKNOWN_OP_CODE,
    CLOSE_NOT_IDENTIFIED,
    CLOSE_ALREADY_IDENTIFIED,
    CLOSE_AUTHENTICATION_FAILED,
    CLOSE_UNSUPPORTED_RPC_VERSION,
)

PORT = int(os.environ.get("TIER4_PORT", "4455"))
URI = f"ws://127.0.0.1:{PORT}"
PASSWORD = "tier4-test-password"

RESULTS = []


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    status = "PASS" if ok else "FAIL"
    line = f"{status} {name}"
    if detail and not ok:
        line += f"  -- {detail}"
    print(line, flush=True)


def wait_for_port(port, timeout=60.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), 0.25):
                return True
        except OSError:
            time.sleep(0.25)
    return False


def wait_for_port_closed(port, timeout=30.0):
    """Wait until the previous OBS has let go of the port."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            socket.create_connection(("127.0.0.1", port), 0.25).close()
            time.sleep(0.5)
        except OSError:
            return True
    return False


class ObsServer:
    """Launches obs headless under xvfb with an isolated profile/config."""

    def __init__(self, obs_bin, port, password=None, keep_home=None,
                 profile=None):
        self.obs_bin = obs_bin
        self.port = port
        self.password = password
        self.profile = profile
        self.home = keep_home or tempfile.mkdtemp(prefix="obs-tier4-")
        self.proc = None

    def _write_profile(self):
        """Start OBS on a profile named Tier4 with the given basic.ini
        sections ({section: {key: value}}) instead of a first-run one."""
        cfg = os.path.join(self.home, ".config", "obs-studio")
        prof = os.path.join(cfg, "basic", "profiles", "Tier4")
        os.makedirs(prof, exist_ok=True)
        os.makedirs(os.path.join(cfg, "basic", "scenes"), exist_ok=True)
        with open(os.path.join(cfg, "global.ini"), "w") as f:
            f.write("[General]\nFirstRun=true\n")
        with open(os.path.join(cfg, "user.ini"), "w") as f:
            f.write("[General]\nFirstRun=true\nConfirmOnExit=false\n"
                    "WarnBeforeStoppingRecord=false\n"
                    "[Basic]\nProfile=Tier4\nProfileDir=Tier4\n"
                    "SceneCollection=Tier4\nSceneCollectionFile=Tier4\n")
        ini = {"General": {"Name": "Tier4"}}
        for section, values in self.profile.items():
            ini.setdefault(section, {}).update(values)
        with open(os.path.join(prof, "basic.ini"), "w") as f:
            for section, values in ini.items():
                f.write(f"[{section}]\n")
                for key, value in values.items():
                    f.write(f"{key}={value}\n")
                f.write("\n")

    def _write_config(self):
        cfg_dir = os.path.join(
            self.home, ".config", "obs-studio", "plugin_config", "obs-websocket")
        os.makedirs(cfg_dir, exist_ok=True)
        cfg = {
            "first_load": False,
            "server_enabled": True,
            "server_port": self.port,
            "alerts_enabled": False,
            "auth_required": self.password is not None,
            "debug_enabled": True,
        }
        if self.password is not None:
            cfg["server_password"] = self.password
        with open(os.path.join(cfg_dir, "config.json"), "w") as f:
            json.dump(cfg, f)

    def start(self):
        self._write_config()
        if self.profile is not None:
            self._write_profile()
        env = dict(os.environ)
        env["HOME"] = self.home
        # Stay headless: never attach to the developer's Wayland session
        # (Qt prefers it over the X display xvfb-run provides).
        env.pop("WAYLAND_DISPLAY", None)
        env["QT_QPA_PLATFORM"] = "xcb"
        for var in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"):
            env.pop(var, None)
        xvfb = shutil.which("xvfb-run")
        if xvfb is not None:
            cmd = [xvfb, "-a", self.obs_bin]
        else:
            cmd = [self.obs_bin]
        # --multi: another OBS on the same machine (a developer's, or a
        # parallel run) must not make this one wait at the single-instance
        # dialog.
        cmd += ["--multi", "--disable-shutdown-check", "--disable-updater",
                "--disable-missing-files-check"]
        self.log = os.path.join(self.home, "obs.log")
        self.logf = open(self.log, "w")
        self.proc = subprocess.Popen(
            cmd, env=env, stdout=self.logf, stderr=subprocess.STDOUT,
            preexec_fn=os.setsid)
        if not wait_for_port(self.port, timeout=90):
            self.dump_log(60)
            raise RuntimeError(
                f"obs-websocket did not open port {self.port} (log above)")

    def dump_log(self, lines=40):
        try:
            with open(self.log) as f:
                tail = f.readlines()[-lines:]
            sys.stdout.write("--- obs log tail ---\n" + "".join(tail) +
                             "--- end obs log ---\n")
        except OSError:
            pass

    def stop(self):
        if self.proc is not None:
            try:
                os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
                self.proc.wait(timeout=15)
            except Exception:
                try:
                    os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
                except Exception:
                    pass
        try:
            self.logf.close()
        except Exception:
            pass


# ---------------------------------------------------------------------------
# Protocol-envelope helpers
# ---------------------------------------------------------------------------

def resp_status(resp):
    return resp.get("d", {}).get("requestStatus", {})


def check_response(name, resp, request_type, expected_codes):
    """Assert the RequestResponse envelope is well formed and code is expected."""
    status = resp_status(resp)
    ok = (
        resp.get("op") == 7
        and resp.get("d", {}).get("requestType") == request_type
        and status.get("result") is not None
        and isinstance(status.get("result"), bool)
        and status.get("code") in expected_codes
    )
    if os.environ.get("TIER4_VERBOSE"):
        print(f"  code={status.get('code')} type={request_type}", flush=True)
    check(name, ok,
          f"op={resp.get('op')} status={status} expected={expected_codes}")
    return status


async def new_identified_client(subscriptions=SUB_ALL, rpc_version=1):
    """Connect and Identify on the no-auth server."""
    c = ObsWsClient(URI)
    hello = await c.connect()
    assert hello.get("op") == 0, f"expected Hello op=0, got {hello}"
    await c.identify(rpc_version=rpc_version,
                     event_subscriptions=subscriptions)
    msg = await c.wait_op(2)
    assert msg.get("op") == 2, f"expected Identified op=2, got {msg}"
    return c, hello


# ---------------------------------------------------------------------------
# Handshake tests
# ---------------------------------------------------------------------------

async def test_handshake_no_password():
    c = ObsWsClient(URI)
    hello = await c.connect()
    ok = (hello.get("op") == 0
          and "obsWebSocketVersion" in hello.get("d", {})
          and "rpcVersion" in hello.get("d", {})
          and "authentication" not in hello.get("d", {}))
    check("handshake:no-auth hello", ok, json.dumps(hello)[:200])
    await c.identify(event_subscriptions=SUB_ALL)
    ident = await c.wait_op(2)
    ok = (ident.get("op") == 2
          and ident.get("d", {}).get("negotiatedRpcVersion") == 1)
    check("handshake:no-auth identified", ok, json.dumps(ident)[:200])
    await c.close_conn()


async def test_request_before_identify():
    c = ObsWsClient(URI)
    await c.connect()
    await c.ws.send(json.dumps(
        {"op": 6, "d": {"requestType": "GetVersion", "requestId": "x"}}))
    await asyncio.sleep(2)
    await c.close_conn()
    check("handshake:request-before-identify closes 4007",
          c.close.code == CLOSE_NOT_IDENTIFIED,
          f"close={c.close}")


async def test_double_identify():
    c, _ = await new_identified_client()
    await c.identify()
    await asyncio.sleep(2)
    await c.close_conn()
    check("handshake:double-identify closes 4008",
          c.close.code == CLOSE_ALREADY_IDENTIFIED,
          f"close={c.close}")


async def test_bad_rpc_version():
    c = ObsWsClient(URI)
    await c.connect()
    await c.identify(rpc_version=0)
    await asyncio.sleep(2)
    await c.close_conn()
    check("handshake:bad rpcVersion closes 4010",
          c.close.code == CLOSE_UNSUPPORTED_RPC_VERSION,
          f"close={c.close}")


async def test_unknown_opcode():
    c, _ = await new_identified_client()
    await c.send_raw({"op": 999, "d": {}})
    await asyncio.sleep(2)
    await c.close_conn()
    check("proto:unknown-opcode closes 4006",
          c.close.code == CLOSE_UNKNOWN_OP_CODE, f"close={c.close}")


async def test_malformed_json():
    c, _ = await new_identified_client()
    await c.send_raw("{{{{not json")
    await asyncio.sleep(2)
    await c.close_conn()
    check("proto:malformed-json closes 4002",
          c.close.code == CLOSE_MESSAGE_DECODE_ERROR,
          f"close={c.close}")


async def test_excessive_nesting():
    # Both parsers used to recurse on untrusted frames before Identify and
    # could exhaust the worker thread's stack instead of closing the client.
    for name, subprotocol, payload in (
        ("json", "obswebsocket.json",
         '{"op":1,"d":{"rpcVersion":1,"x":' +
         "[" * 500000 + "0" + "]" * 500000 + "}}"),
        ("msgpack", "obswebsocket.msgpack", b"\x91" * 50000 + b"\xc0"),
    ):
        code = None
        try:
            async with websockets.connect(
                    URI, subprotocols=[subprotocol], max_size=16 * 1024 * 1024) as ws:
                await ws.recv()  # Hello; the payload is deliberately pre-auth.
                await ws.send(payload)
                try:
                    await asyncio.wait_for(ws.recv(), timeout=10)
                except websockets.exceptions.ConnectionClosed as exc:
                    code = exc.code
        except (OSError, asyncio.TimeoutError, websockets.exceptions.WebSocketException):
            pass
        check(f"proto:{name}-depth closes 4002",
              code == CLOSE_MESSAGE_DECODE_ERROR, f"close={code}")
        check(f"proto:{name}-depth keeps server alive",
              wait_for_port(PORT, timeout=2))


async def test_identify_missing_rpc_version():
    c = ObsWsClient(URI)
    await c.connect()
    await c.identify(raw_d={})
    await asyncio.sleep(2)
    await c.close_conn()
    check("handshake:missing rpcVersion closes "
          f"{CLOSE_MISSING_DATA_FIELD}/{CLOSE_UNSUPPORTED_RPC_VERSION}",
          c.close.code in (CLOSE_MISSING_DATA_FIELD,
                           CLOSE_UNSUPPORTED_RPC_VERSION),
          f"close={c.close}")


# ---------------------------------------------------------------------------
# Request error-path tests
# ---------------------------------------------------------------------------

async def test_request_errors():
    c, _ = await new_identified_client()

    resp = await c.request("NoSuchRequestType")
    check_response("err:unknown-request-type", resp, "NoSuchRequestType",
                   [STATUS_UNKNOWN_REQUEST_TYPE])

    # Missing requestType at the message level closes the session (4003).
    c2, _ = await new_identified_client()
    await c2.ws.send(json.dumps({"op": 6, "d": {"requestId": "no-type"}}))
    await asyncio.sleep(2)
    await c2.close_conn()
    check("err:missing-request-type closes 4003",
          c2.close.code == CLOSE_MISSING_DATA_FIELD,
          f"close={c2.close}")

    resp = await c.request("Sleep", {"sleepMillis": 1})
    check_response("err:sleep-outside-batch", resp, "Sleep",
                   [STATUS_UNSUPPORTED_BATCH_EXECUTION_TYPE])

    resp = await c.request("CreateScene", {})
    check_response("err:missing-request-field", resp, "CreateScene",
                   [STATUS_MISSING_REQUEST_FIELD,
                    STATUS_MISSING_REQUEST_DATA])

    resp = await c.request("CreateScene", {"sceneName": 12345})
    check_response("err:invalid-field-type", resp, "CreateScene",
                   [STATUS_INVALID_REQUEST_FIELD_TYPE,
                    STATUS_INVALID_REQUEST_FIELD])

    resp = await c.request("GetInputList", {"inputKind": 7})
    check_response("err:optional-field-wrong-type", resp, "GetInputList",
                   [STATUS_INVALID_REQUEST_FIELD_TYPE,
                    STATUS_INVALID_REQUEST_FIELD,
                    STATUS_SUCCESS])

    resp = await c.request("SetInputVolume",
                           {"inputName": "no-such-input-xyz",
                            "inputVolumeMul": 0.5})
    check_response("err:resource-not-found", resp, "SetInputVolume",
                   [STATUS_RESOURCE_NOT_FOUND])

    await c.close_conn()


# ---------------------------------------------------------------------------
# Event delivery tests
# ---------------------------------------------------------------------------

async def test_event_delivery():
    c, _ = await new_identified_client(
        subscriptions=SUB_GENERAL | SUB_SCENES | SUB_CONFIG)
    scene = "T4EventScene"
    await c.request("CreateScene", {"sceneName": scene})
    ev = await c.wait_for_event("SceneCreated", timeout=8)
    ok = (ev is not None
          and ev.get("d", {}).get("eventData", {}).get("sceneName") == scene
          and ev.get("d", {}).get("eventIntent") is not None)
    check("event:SceneCreated delivered", ok, json.dumps(ev)[:300])

    await c.request("BroadcastCustomEvent",
                    {"eventData": {"tier4": "mark"}})
    ev = await c.wait_for_event("CustomEvent", timeout=8)
    ok = (ev is not None
          and ev.get("d", {}).get("eventData", {}).get("tier4") == "mark")
    check("event:CustomEvent delivered", ok, json.dumps(ev)[:300])

    c2, _ = await new_identified_client(subscriptions=SUB_NONE)
    await c.request("BroadcastCustomEvent",
                    {"eventData": {"tier4": "nope"}})
    await asyncio.sleep(1.5)
    check("event:none-subscription gets no events",
          c2.events("CustomEvent") == [])
    await c2.close_conn()
    await c.close_conn()


# ---------------------------------------------------------------------------
# Batch request tests
# ---------------------------------------------------------------------------

async def test_batch():
    c, _ = await new_identified_client()

    resp = await c.request_batch(
        [{"requestType": "GetVersion", "requestId": "b1"},
         {"requestType": "GetStats", "requestId": "b2"},
         {"requestType": "GetSceneList", "requestId": "b3"}],
        execution_type=0)
    d = resp.get("d", {})
    results = d.get("results", [])
    ok = (resp.get("op") == 9
          and len(results) == 3
          and [r.get("requestId") for r in results] == ["b1", "b2", "b3"]
          and all(r.get("requestStatus", {}).get("result") for r in results))
    check("batch:serial-ok", ok, json.dumps(resp)[:400])

    resp = await c.request_batch(
        [{"requestType": "GetVersion", "requestId": "hb1"},
         {"requestType": "NoSuchType", "requestId": "hb2"},
         {"requestType": "GetStats", "requestId": "hb3"}],
        execution_type=0, halt_on_failure=True)
    results = resp.get("d", {}).get("results", [])
    ok = (resp.get("op") == 9
          and len(results) == 2
          and results[0].get("requestStatus", {}).get("result") is True
          and results[1].get("requestStatus", {}).get("result") is False)
    check("batch:halt-on-failure", ok, json.dumps(resp)[:400])

    resp = await c.request_batch(
        [{"requestType": "GetVersion", "requestId": "nb1"},
         {"requestType": "NoSuchType", "requestId": "nb2"},
         {"requestType": "GetStats", "requestId": "nb3"}],
        execution_type=0, halt_on_failure=False)
    results = resp.get("d", {}).get("results", [])
    ok = (resp.get("op") == 9
          and len(results) == 3
          and results[1].get("requestStatus", {}).get("code")
          == STATUS_UNKNOWN_REQUEST_TYPE)
    check("batch:no-halt-continues", ok, json.dumps(resp)[:400])

    resp = await c.request_batch(
        [{"requestType": "GetVersion", "requestId": "pb1"}],
        execution_type=2)
    ok = (resp.get("op") == 9
          or resp_status(resp).get("code")
          == STATUS_UNSUPPORTED_BATCH_EXECUTION_TYPE)
    check("batch:parallel-accepted-or-206", ok, json.dumps(resp)[:300])

    await c.close_conn()


# ---------------------------------------------------------------------------
# Full request-type coverage
# ---------------------------------------------------------------------------

# Each entry: requestType, requestData, expected RequestStatus codes.
# `live` entries are filled at runtime with seeded resource values.
S = STATUS_SUCCESS


def coverage_matrix(ctx):
    """Build the per-request coverage table.

    ctx carries seeded values: scene, scene_b, color, media, filter_name,
    transition_name, transition_kind, scene_item_id, stream_rtmp_settings,
    output_name, hotkey_name, group_scene.
    """
    scene = ctx["scene"]
    scene_b = ctx["scene_b"]
    color = ctx["color"]
    media = ctx["media"]
    filt = ctx["filter_name"]
    trans = ctx["transition_name"]
    item = ctx["scene_item_id"]
    out = ctx["output_name"]
    hotkey = ctx["hotkey_name"]

    ok_or_missing = [S, STATUS_RESOURCE_NOT_FOUND]
    statey = [S, STATUS_OUTPUT_RUNNING, STATUS_OUTPUT_NOT_RUNNING,
              STATUS_INVALID_RESOURCE_STATE, STATUS_RESOURCE_NOT_FOUND,
              STATUS_CANNOT_ACT, STATUS_RESOURCE_NOT_CONFIGURABLE]
    act_fail = [S, STATUS_RESOURCE_ACTION_FAILED,
                STATUS_REQUEST_PROCESSING_FAILED,
                STATUS_OUTPUT_NOT_RUNNING, STATUS_INVALID_RESOURCE_STATE,
                STATUS_RESOURCE_NOT_FOUND, STATUS_CANNOT_ACT]

    return [
        # canvases
        ("GetCanvasList", {}, [S, STATUS_UNKNOWN_REQUEST_TYPE]),
        # config
        ("GetPersistentData", {"realm": "OBS_WEBSOCKET_DATA_REALM_GLOBAL",
                               "slotName": "tier4slot"}, ok_or_missing),
        ("SetPersistentData", {"realm": "OBS_WEBSOCKET_DATA_REALM_GLOBAL",
                               "slotName": "tier4slot", "slotValue": 42}, [S]),
        ("GetSceneCollectionList", {}, [S]),
        ("SetCurrentSceneCollection",
         {"sceneCollectionName": ctx["scene_collection"]}, [S]),
        ("GetProfileList", {}, [S]),
        ("SetCurrentProfile", {"profileName": ctx["profile"]}, [S]),
        ("GetProfileParameter", {"parameterCategory": "General",
                                 "parameterName": "Name"}, ok_or_missing),
        ("SetProfileParameter", {"parameterCategory": "Tier4",
                                 "parameterName": "K",
                                 "parameterValue": "v"}, [S]),
        ("GetVideoSettings", {}, [S]),
        ("SetVideoSettings",
         {"fpsNumerator": 30, "fpsDenominator": 1,
          "baseWidth": 1280, "baseHeight": 720,
          "outputWidth": 1280, "outputHeight": 720}, [S]),
        ("GetStreamServiceSettings", {}, [S]),
        ("SetStreamServiceSettings",
         {"streamServiceType": "rtmp_custom",
          "streamServiceSettings": ctx["stream_rtmp_settings"]}, [S]),
        ("GetRecordDirectory", {}, [S]),
        ("SetRecordDirectory", {"recordDirectory": ctx["record_dir"]}, [S]),
        # filters
        ("GetSourceFilterKindList", {}, [S]),
        ("GetSourceFilterList", {"sourceName": color}, [S]),
        ("GetSourceFilterDefaultSettings", {"filterKind": "color_filter"},
         [S, STATUS_INVALID_FILTER_KIND]),
        ("CreateSourceFilter",
         {"sourceName": color, "filterName": filt,
          "filterKind": "color_filter",
          "filterSettings": {}}, [S, STATUS_RESOURCE_ALREADY_EXISTS,
                                  STATUS_INVALID_FILTER_KIND]),
        ("RemoveSourceFilter",
         {"sourceName": color, "filterName": "T4FilterRemove"}, ok_or_missing),
        ("SetSourceFilterName",
         {"sourceName": color, "filterName": filt,
          "newFilterName": "T4FilterRenamed"}, [S]),
        ("GetSourceFilter", {"sourceName": color,
                             "filterName": "T4FilterRenamed"}, [S]),
        ("SetSourceFilterIndex",
         {"sourceName": color, "filterName": "T4FilterRenamed",
          "filterIndex": 0}, [S]),
        ("SetSourceFilterSettings",
         {"sourceName": color, "filterName": "T4FilterRenamed",
          "filterSettings": {"opacity": 80}, "overlay": False}, [S]),
        ("SetSourceFilterEnabled",
         {"sourceName": color, "filterName": "T4FilterRenamed",
          "filterEnabled": False}, [S]),
        # general
        ("GetVersion", {}, [S]),
        ("GetStats", {}, [S]),
        ("BroadcastCustomEvent", {"eventData": {"k": "v"}}, [S]),
        ("CallVendorRequest",
         {"vendorName": "obs-websocket-test",
          "requestType": "TestRequest", "requestData": {"a": 1}},
         [S, STATUS_RESOURCE_NOT_FOUND, STATUS_UNKNOWN_REQUEST_TYPE]),
        ("GetHotkeyList", {}, [S]),
        ("TriggerHotkeyByName", {"hotkeyName": hotkey}, [S,
          STATUS_RESOURCE_NOT_FOUND]),
        ("TriggerHotkeyByKeySequence",
         {"keyId": "OBS_KEY_S", "keyModifiers": {"shift": True}},
         [S, STATUS_INVALID_REQUEST_FIELD]),
        # Sleep is only valid inside a request batch on this server version.
        ("Sleep", {"sleepMillis": 25},
         [S, STATUS_UNSUPPORTED_BATCH_EXECUTION_TYPE]),
        # inputs
        ("GetInputList", {}, [S]),
        ("GetInputList", {"inputKind": "color_source_v3"}, [S]),
        ("GetInputKindList", {}, [S]),
        ("GetInputKindList", {"unversioned": True}, [S]),
        ("GetSpecialInputs", {}, [S]),
        ("CreateInput",
         {"sceneName": scene, "inputName": "T4Color2",
          "inputKind": "color_source_v3",
          "inputSettings": {"color": 4294901760},
          "sceneItemEnabled": True}, [S, STATUS_RESOURCE_ALREADY_EXISTS]),
        ("RemoveInput", {"inputName": "T4TempInput"},
         [S, STATUS_RESOURCE_NOT_FOUND]),
        ("SetInputName", {"inputName": color,
                          "newInputName": "T4ColorRenamed"}, [S]),
        ("GetInputDefaultSettings", {"inputKind": "color_source_v3"}, [S]),
        ("GetInputSettings", {"inputName": "T4ColorRenamed"}, [S]),
        ("SetInputSettings",
         {"inputName": "T4ColorRenamed",
          "inputSettings": {"color": 4278190080}, "overlay": True}, [S]),
        ("GetInputMute", {"inputName": media}, [S]),
        ("SetInputMute", {"inputName": media,
                          "inputMuted": True}, [S]),
        ("ToggleInputMute", {"inputName": media}, [S]),
        ("GetInputVolume", {"inputName": media}, [S]),
        ("SetInputVolume", {"inputName": media,
                            "inputVolumeMul": 0.5}, [S]),
        ("GetInputAudioBalance", {"inputName": media}, [S]),
        ("SetInputAudioBalance", {"inputName": media,
                                  "inputAudioBalance": 0.5}, [S]),
        ("GetInputAudioSyncOffset", {"inputName": media}, [S]),
        ("SetInputAudioSyncOffset",
         {"inputName": media, "inputAudioSyncOffset": 0}, [S]),
        ("GetInputAudioMonitorType", {"inputName": media}, [S]),
        ("SetInputAudioMonitorType",
         {"inputName": media,
          "monitorType": "OBS_MONITORING_TYPE_NONE"}, [S]),
        ("GetInputAudioTracks", {"inputName": media}, [S]),
        ("SetInputAudioTracks",
         {"inputName": media,
          "inputAudioTracks": {"1": True}}, [S]),
        # Deinterlace requests need an async input (the media source);
        # a sync input such as a color source answers 604.
        ("GetInputDeinterlaceMode", {"inputName": media}, [S]),
        ("SetInputDeinterlaceMode",
         {"inputName": media,
          "inputDeinterlaceMode": "OBS_DEINTERLACE_MODE_DISABLE"}, [S]),
        ("GetInputDeinterlaceFieldOrder", {"inputName": media}, [S]),
        ("SetInputDeinterlaceFieldOrder",
         {"inputName": media,
          "inputDeinterlaceFieldOrder":
              "OBS_DEINTERLACE_FIELD_ORDER_TOP"}, [S]),
        ("GetInputDeinterlaceMode", {"inputName": "T4ColorRenamed"},
         [STATUS_INVALID_RESOURCE_STATE]),
        ("GetInputPropertiesListPropertyItems",
         {"inputName": media, "propertyName": ctx["media_list_property"]},
         [S, STATUS_INVALID_REQUEST_FIELD, STATUS_RESOURCE_NOT_FOUND,
          STATUS_INVALID_RESOURCE_TYPE]),
        ("PressInputPropertiesButton",
         {"inputName": media, "propertyName": ctx["media_button_property"]},
         [S, STATUS_INVALID_REQUEST_FIELD, STATUS_RESOURCE_NOT_FOUND]),
        # media inputs
        ("GetMediaInputStatus", {"inputName": media}, [S]),
        ("SetMediaInputCursor", {"inputName": media,
                                 "mediaCursor": 0}, statey),
        ("OffsetMediaInputCursor", {"inputName": media,
                                    "mediaCursorOffset": 0}, statey),
        ("TriggerMediaInputAction",
         {"inputName": media,
          "mediaAction": "OBS_WEBSOCKET_MEDIA_INPUT_ACTION_STOP"}, statey),
        # outputs (status/settings reads; lifecycle actuators run in the tail)
        ("GetVirtualCamStatus", {}, [S]),
        ("GetReplayBufferStatus", {},
         [S, STATUS_INVALID_RESOURCE_STATE]),
        ("GetLastReplayBufferReplay", {}, [S,
          STATUS_INVALID_RESOURCE_STATE, STATUS_RESOURCE_NOT_FOUND]),
        ("GetOutputList", {}, [S]),
        ("GetOutputStatus", {"outputName": out}, ok_or_missing),
        ("GetOutputSettings", {"outputName": out}, ok_or_missing),
        ("SetOutputSettings",
         {"outputName": out, "outputSettings": {}}, ok_or_missing),
        # record: a lifecycle order (start -> pause -> resume -> extras ->
        # stop). Codes reflect each call's state at run time.
        ("GetRecordStatus", {}, [S]),
        ("StartRecord", {}, [S, STATUS_OUTPUT_RUNNING,
                             STATUS_RESOURCE_CREATION_FAILED,
                             STATUS_RESOURCE_ACTION_FAILED,
                             STATUS_CANNOT_ACT]),
        ("PauseRecord", {}, [S, STATUS_OUTPUT_NOT_RUNNING,
                             STATUS_INVALID_RESOURCE_STATE,
                             STATUS_CANNOT_ACT]),
        ("ResumeRecord", {}, [S, STATUS_OUTPUT_NOT_RUNNING,
                              STATUS_INVALID_RESOURCE_STATE,
                              STATUS_OUTPUT_PAUSED,
                              STATUS_OUTPUT_NOT_PAUSED,
                              STATUS_CANNOT_ACT]),
        ("ToggleRecordPause", {}, [S, STATUS_OUTPUT_NOT_RUNNING,
                                   STATUS_INVALID_RESOURCE_STATE,
                                   STATUS_CANNOT_ACT]),
        ("SplitRecordFile", {}, [S, STATUS_OUTPUT_NOT_RUNNING,
                                 STATUS_INVALID_RESOURCE_STATE,
                                 STATUS_REQUEST_PROCESSING_FAILED,
                                 STATUS_CANNOT_ACT]),
        ("CreateRecordChapter", {"chapterName": "t4"},
         [S, STATUS_OUTPUT_NOT_RUNNING, STATUS_INVALID_RESOURCE_STATE,
          STATUS_REQUEST_PROCESSING_FAILED, STATUS_CANNOT_ACT]),
        ("StopRecord", {}, [S, STATUS_OUTPUT_NOT_RUNNING,
                            STATUS_CANNOT_ACT]),
        ("ToggleRecord", {}, [S, STATUS_OUTPUT_RUNNING,
                              STATUS_OUTPUT_NOT_RUNNING,
                              STATUS_RESOURCE_CREATION_FAILED,
                              STATUS_RESOURCE_ACTION_FAILED,
                              STATUS_CANNOT_ACT]),
        # scene items
        ("GetSceneItemList", {"sceneName": scene}, [S]),
        ("GetGroupSceneItemList", {"sceneName": ctx["group_scene"]},
         [S, STATUS_RESOURCE_NOT_FOUND, STATUS_INVALID_RESOURCE_TYPE]),
        ("GetSceneItemId", {"sceneName": scene,
                            "sourceName": "T4ColorRenamed"}, [S]),
        ("GetSceneItemSource", {"sceneName": scene,
                                "sceneItemId": item}, [S]),
        ("CreateSceneItem",
         {"sceneName": scene, "sourceName": media,
          "sceneItemEnabled": True}, [S]),
        ("RemoveSceneItem",
         {"sceneName": scene, "sceneItemId": ctx["temp_item_id"]},
         [S, STATUS_RESOURCE_NOT_FOUND]),
        ("DuplicateSceneItem",
         {"sceneName": scene, "sceneItemId": item,
          "destinationSceneName": scene_b}, [S]),
        ("GetSceneItemTransform", {"sceneName": scene,
                                   "sceneItemId": item}, [S]),
        ("SetSceneItemTransform",
         {"sceneName": scene, "sceneItemId": item,
          "sceneItemTransform": {"positionX": 10, "positionY": 10}}, [S]),
        ("GetSceneItemEnabled", {"sceneName": scene,
                                 "sceneItemId": item}, [S]),
        ("SetSceneItemEnabled",
         {"sceneName": scene, "sceneItemId": item,
          "sceneItemEnabled": False}, [S]),
        ("GetSceneItemLocked", {"sceneName": scene,
                                "sceneItemId": item}, [S]),
        ("SetSceneItemLocked",
         {"sceneName": scene, "sceneItemId": item,
          "sceneItemLocked": True}, [S]),
        ("GetSceneItemIndex", {"sceneName": scene,
                               "sceneItemId": item}, [S]),
        ("SetSceneItemIndex",
         {"sceneName": scene, "sceneItemId": item,
          "sceneItemIndex": 0}, [S]),
        ("GetSceneItemBlendMode", {"sceneName": scene,
                                   "sceneItemId": item}, [S]),
        ("SetSceneItemBlendMode",
         {"sceneName": scene, "sceneItemId": item,
          "sceneItemBlendMode": "OBS_BLEND_NORMAL"}, [S]),
        # scenes
        ("GetSceneList", {}, [S]),
        ("GetGroupList", {}, [S]),
        ("GetCurrentProgramScene", {}, [S]),
        ("SetCurrentProgramScene", {"sceneName": scene}, [S]),
        ("GetCurrentPreviewScene", {}, [S,
          STATUS_INVALID_RESOURCE_STATE,
          STATUS_STUDIO_MODE_NOT_ACTIVE]),
        ("SetCurrentPreviewScene", {"sceneName": scene_b}, [S,
          STATUS_INVALID_RESOURCE_STATE,
          STATUS_STUDIO_MODE_NOT_ACTIVE]),
        ("CreateScene", {"sceneName": "T4SceneC"},
         [S, STATUS_RESOURCE_ALREADY_EXISTS]),
        ("RemoveScene", {"sceneName": "T4SceneRemove"},
         [S, STATUS_RESOURCE_NOT_FOUND]),
        ("SetSceneName", {"sceneName": "T4SceneC",
                          "newSceneName": "T4SceneCRenamed"}, [S]),
        ("GetSceneSceneTransitionOverride", {"sceneName": scene}, [S]),
        ("SetSceneSceneTransitionOverride",
         {"sceneName": scene, "transitionName": trans,
          "transitionDuration": 250}, [S]),
        # sources
        ("GetSourceActive", {"sourceName": "T4ColorRenamed"}, [S]),
        ("GetSourceScreenshot",
         {"sourceName": "T4ColorRenamed", "imageFormat": "png",
          "imageWidth": 64, "imageHeight": 64},
         [S, STATUS_RESOURCE_ACTION_FAILED,
          STATUS_REQUEST_PROCESSING_FAILED]),
        ("SaveSourceScreenshot",
         {"sourceName": "T4ColorRenamed", "imageFormat": "png",
          "imageFilePath": ctx["screenshot_path"],
          "imageWidth": 64, "imageHeight": 64},
         [S, STATUS_RESOURCE_ACTION_FAILED,
          STATUS_REQUEST_PROCESSING_FAILED]),
        ("GetStreamStatus", {}, [S]),
        ("SendStreamCaption", {"captionText": "hi"},
         [S, STATUS_OUTPUT_NOT_RUNNING, STATUS_INVALID_RESOURCE_STATE,
          STATUS_CANNOT_ACT]),
        # transitions
        ("GetTransitionKindList", {}, [S]),
        ("GetSceneTransitionList", {}, [S]),
        ("GetCurrentSceneTransition", {}, [S]),
        ("SetCurrentSceneTransition", {"transitionName": trans}, [S]),
        ("SetCurrentSceneTransitionDuration",
         {"transitionDuration": 250}, [S]),
        ("SetCurrentSceneTransitionSettings",
         {"transitionSettings": {}},
         [S, STATUS_RESOURCE_NOT_CONFIGURABLE]),
        ("GetCurrentSceneTransitionCursor", {}, [S]),
        ("TriggerStudioModeTransition", {},
         [S, STATUS_INVALID_RESOURCE_STATE, STATUS_CANNOT_ACT,
          STATUS_STUDIO_MODE_NOT_ACTIVE]),
        ("SetTBarPosition", {"position": 0.5, "release": True},
         [S, STATUS_INVALID_RESOURCE_STATE, STATUS_CANNOT_ACT,
          STATUS_STUDIO_MODE_NOT_ACTIVE]),
        # ui
        ("GetStudioModeEnabled", {}, [S]),
        ("SetStudioModeEnabled", {"studioModeEnabled": True}, [S]),
        ("OpenInputPropertiesDialog",
         {"inputName": "T4ColorRenamed"}, [S]),
        ("OpenInputFiltersDialog",
         {"inputName": "T4ColorRenamed"}, [S]),
        ("OpenInputInteractDialog",
         {"inputName": "T4ColorRenamed"},
         [S, STATUS_INVALID_RESOURCE_TYPE, STATUS_CANNOT_ACT,
          STATUS_INVALID_RESOURCE_STATE]),
        ("GetMonitorList", {}, [S]),
        ("OpenVideoMixProjector",
         {"videoMixType": "OBS_WEBSOCKET_VIDEO_MIX_TYPE_PREVIEW",
          "monitorIndex": -1}, [S, STATUS_INVALID_REQUEST_FIELD]),
        ("OpenSourceProjector",
         {"sourceName": "T4ColorRenamed", "monitorIndex": -1},
         [S, STATUS_INVALID_REQUEST_FIELD]),
        # Collection/profile mutations are last: they switch the active
        # scene collection, invalidating every seeded scene/input name.
        ("CreateSceneCollection",
         {"sceneCollectionName": "T4Collection"},
         [S, STATUS_RESOURCE_ALREADY_EXISTS]),
        ("CreateProfile", {"profileName": "T4Profile"},
         [S, STATUS_RESOURCE_ALREADY_EXISTS]),
        ("RemoveProfile", {"profileName": "T4Profile"},
         [S, STATUS_RESOURCE_NOT_FOUND, STATUS_CANNOT_ACT]),
    ]


def lifecycle_tail(ctx):
    """Output/stream actuator calls that can wedge the request thread while
    an output connect attempt unwinds. Run last, one fresh client each."""
    out = ctx["output_name"]
    act_fail = [S, STATUS_RESOURCE_ACTION_FAILED,
                STATUS_REQUEST_PROCESSING_FAILED,
                STATUS_OUTPUT_NOT_RUNNING, STATUS_OUTPUT_RUNNING,
                STATUS_OUTPUT_PAUSED, STATUS_OUTPUT_NOT_PAUSED,
                STATUS_INVALID_RESOURCE_STATE,
                STATUS_RESOURCE_NOT_FOUND, STATUS_CANNOT_ACT,
                STATUS_RESOURCE_CREATION_FAILED]
    return [
        ("ToggleReplayBuffer", {}, act_fail),
        ("StartReplayBuffer", {}, act_fail),
        ("StopReplayBuffer", {}, act_fail),
        ("SaveReplayBuffer", {}, act_fail),
        ("ToggleOutput", {"outputName": out}, act_fail),
        ("StartOutput", {"outputName": out}, act_fail),
        ("StopOutput", {"outputName": out}, act_fail, 60.0),
        ("ToggleVirtualCam", {}, act_fail),
        ("StartVirtualCam", {}, act_fail),
        ("StopVirtualCam", {}, act_fail, 60.0),
        ("StartStream", {}, act_fail),
        ("ToggleStream", {}, act_fail),
        ("StopStream", {}, act_fail, 90.0),
    ]


async def seed_resources(c, ctx):
    """Create the resources the coverage matrix needs, via the wire."""
    r = await c.request("GetSceneCollectionList")
    ctx["scene_collection"] = (
        r.get("d", {}).get("responseData", {})
        .get("sceneCollections", [{}])[0]
        if isinstance(
            r.get("d", {}).get("responseData", {})
            .get("sceneCollections", []), list)
        and r["d"]["responseData"]["sceneCollections"]
        else "default")
    r = await c.request("GetProfileList")
    ctx["profile"] = (
        r.get("d", {}).get("responseData", {})
        .get("currentProfileName", "default"))

    async def must(req_type, data):
        r = await c.request(req_type, data)
        st = resp_status(r)
        if st.get("code") != STATUS_SUCCESS:
            raise RuntimeError(
                f"seed {req_type} failed: {st}")
        return r

    await must("CreateScene", {"sceneName": ctx["scene"]})
    await must("CreateScene", {"sceneName": ctx["scene_b"]})
    await must("CreateScene", {"sceneName": "T4SceneRemove"})
    await must("CreateInput", {
        "sceneName": ctx["scene"], "inputName": ctx["color"],
        "inputKind": "color_source_v3",
        "inputSettings": {"color": 4294901760}})
    await must("CreateInput", {
        "sceneName": ctx["scene"], "inputName": "T4TempInput",
        "inputKind": "color_source_v3"})
    # A media input kind: prefer ffmpeg_source, fall back to any kind whose
    # name contains 'media' for portability across builds.
    kind = "ffmpeg_source"
    r = await c.request("CreateInput", {
        "sceneName": ctx["scene"], "inputName": ctx["media"],
        "inputKind": kind,
        "inputSettings": {"local_file": "/nonexistent-t4.mp4",
                          "is_local_file": True}})
    if resp_status(r).get("code") != STATUS_SUCCESS:
        r = await c.request("GetInputKindList")
        kinds = (r.get("d", {}).get("responseData", {})
                 .get("inputKinds", []))
        alt = next((k for k in kinds if "media" in k.lower()
                    or "ffmpeg" in k.lower() or "vlc" in k.lower()), None)
        if alt is None:
            raise RuntimeError(
                f"no usable media input kind; CreateInput failed and "
                f"kinds={kinds}")
        kind = alt
        await must("CreateInput", {
            "sceneName": ctx["scene"], "inputName": ctx["media"],
            "inputKind": kind})
    ctx["media_kind"] = kind
    await must("CreateSourceFilter", {
        "sourceName": ctx["color"], "filterName": ctx["filter_name"],
        "filterKind": "color_filter"})

    r = await c.request("GetSceneItemId", {
        "sceneName": ctx["scene"], "sourceName": ctx["color"]})
    ctx["scene_item_id"] = (
        r.get("d", {}).get("responseData", {}).get("sceneItemId", 1))

    r = await c.request("GetSceneItemList", {"sceneName": ctx["scene"]})
    items = r.get("d", {}).get("responseData", {}).get("sceneItems", [])
    for it in items:
        if it.get("sourceName") == "T4TempInput":
            ctx["temp_item_id"] = it.get("sceneItemId")
    ctx.setdefault("temp_item_id", 9999)

    r = await c.request("GetSceneTransitionList")
    transitions = (r.get("d", {}).get("responseData", {})
                   .get("transitions", []))
    ctx["transition_name"] = (
        transitions[0]["transitionName"] if transitions else "Fade")

    r = await c.request("GetHotkeyList")
    hotkeys = (r.get("d", {}).get("responseData", {})
               .get("hotkeys", []))
    # Only provably inert hotkeys are eligible: anything that could start,
    # stop, toggle or otherwise actuate outputs/scenes/UI is excluded so a
    # trigger cannot change global state mid-suite.
    dangerous = ("stream", "record", "virtualcam", "replay",
                 "studiomode", "screenshot", "exit", "remove", "start",
                 "stop", "toggle", "transition", "fullscreen", "projector",
                 "pause", "save", "enable", "disable", "action", "output",
                 "mixer", "scene", "refresh", "vanish", "force")
    safe = [h for h in hotkeys
            if not any(d in h.lower() for d in dangerous)]
    # If nothing is provably inert, a nonexistent name still exercises the
    # request type and returns the documented ResourceNotFound status.
    ctx["hotkey_name"] = safe[0] if safe else "T4NoSuchHotkey"

    r = await c.request("GetOutputList")
    outputs = r.get("d", {}).get("responseData", {}).get("outputs", [])
    ctx["output_name"] = outputs[0]["outputName"] if outputs else "rtmp_output"

    # Empty server: StartStream answers a fast error instead of a blocking
    # connect loop to an unreachable endpoint.
    ctx["stream_rtmp_settings"] = {
        "bwtest": False, "server": "", "key": "tier4"}
    ctx["record_dir"] = tempfile.mkdtemp(prefix="obs-tier4-rec-")
    ctx["screenshot_path"] = os.path.join(ctx["record_dir"], "t4.png")
    ctx["group_scene"] = "T4GroupScene"
    ctx["media_list_property"] = "input_format"
    ctx["media_button_property"] = "refresh"

    # Enable studio mode (via the wire) so preview/transition requests
    # exercise their real success path.
    await c.request("SetStudioModeEnabled", {"studioModeEnabled": True})


async def test_full_coverage():
    c, _ = await new_identified_client()
    ctx = {
        "scene": "T4Scene",
        "scene_b": "T4SceneB",
        "color": "T4Color",
        "media": "T4Media",
        "filter_name": "T4Filter",
    }
    await seed_resources(c, ctx)

    # Type-validation path against a real input name (before the matrix's
    # collection mutations invalidate names).
    resp = await c.request("SetInputVolume",
                           {"inputName": ctx["media"],
                            "inputVolumeMul": "loud"})
    check_response("err:invalid-field-type-volume", resp, "SetInputVolume",
                   [STATUS_INVALID_REQUEST_FIELD_TYPE,
                    STATUS_INVALID_REQUEST_FIELD])

    covered = 0
    seen = set()
    for entry in coverage_matrix(ctx):
        request_type, data, codes = entry[0], entry[1], entry[2]
        timeout = entry[3] if len(entry) > 3 else 15.0
        resp = None
        try:
            resp = await c.request(request_type, data, timeout=timeout)
        except Exception:
            # OBS can stall the request thread transiently (output connect
            # attempts, media probes). Reconnect and retry once.
            try:
                await c.close_conn()
            except Exception:
                pass
            try:
                c, _ = await new_identified_client()
                resp = await c.request(request_type, data, timeout=timeout)
            except Exception as e2:
                check(f"req:{request_type}", False,
                      f"{type(e2).__name__}: {e2}")
                continue
        st = resp_status(resp)
        check_response(f"req:{request_type}", resp, request_type, codes)
        covered += 1
        seen.add(request_type)

    # Lifecycle actuators: outputs can wedge the request thread while a
    # connect attempt unwinds, so each runs on a fresh client and a conn
    # drop only fails its own check. StopStream gets time to unwind.
    for entry in lifecycle_tail(ctx):
        request_type, data, codes = entry[0], entry[1], entry[2]
        timeout = entry[3] if len(entry) > 3 else 20.0
        tc = None
        try:
            tc, _ = await new_identified_client()
            resp = await tc.request(request_type, data, timeout=timeout)
            check_response(f"req:{request_type}", resp, request_type, codes)
            seen.add(request_type)
        except Exception as e:
            check(f"req:{request_type}", False, f"{type(e).__name__}: {e}")
        finally:
            if tc is not None:
                await tc.close_conn()
        covered += 1

    await c.close_conn()
    return covered, seen


# ---------------------------------------------------------------------------
# Auth-required phase
# ---------------------------------------------------------------------------

async def test_auth_handshake():
    # correct password
    hello = None
    for _ in range(4):
        try:
            c = ObsWsClient(URI)
            hello = await c.connect()
            if hello is not None:
                break
        except Exception:
            hello = None
        await asyncio.sleep(3)
    assert hello is not None, "no Hello from auth server"
    auth = hello.get("d", {}).get("authentication", {})
    ok = (hello.get("op") == 0
          and "salt" in auth and "challenge" in auth)
    check("auth:hello carries challenge", ok, json.dumps(hello)[:200])
    await c.identify(password=PASSWORD, auth_challenge=auth)
    ident = await c.wait_op(2)
    check("auth:correct-password identifies",
          ident.get("op") == 2, json.dumps(ident)[:200])
    # The server accepts connections before the frontend finishes loading
    # and answers 207 (NotReady) until then; wait that out.
    for _ in range(120):
        resp = await c.request("GetVersion")
        if resp_status(resp).get("code") != 207:
            break
        await asyncio.sleep(0.5)
    check_response("auth:authed-request", resp, "GetVersion", [S])
    await c.close_conn()

    # wrong password
    c2 = ObsWsClient(URI)
    hello = await c2.connect()
    await c2.identify(password="wrong-password",
                      auth_challenge=hello["d"]["authentication"])
    await asyncio.sleep(2)
    await c2.close_conn()
    check("auth:wrong-password closes 4009",
          c2.close.code == CLOSE_AUTHENTICATION_FAILED,
          f"close={c2.close}")

    # identify without authentication field
    c3 = ObsWsClient(URI)
    hello = await c3.connect()
    await c3.identify()
    await asyncio.sleep(2)
    await c3.close_conn()
    check("auth:no-auth-field closes 4009",
          c3.close.code == CLOSE_AUTHENTICATION_FAILED,
          f"close={c3.close}")

    # request before identify under auth
    c4 = ObsWsClient(URI)
    await c4.connect()
    await c4.ws.send(json.dumps(
        {"op": 6, "d": {"requestType": "GetVersion", "requestId": "x"}}))
    await asyncio.sleep(2)
    await c4.close_conn()
    check("auth:request-before-identify closes 4007",
          c4.close.code == CLOSE_NOT_IDENTIFIED,
          f"close={c4.close}")


# ---------------------------------------------------------------------------
# Entrypoint
# ---------------------------------------------------------------------------

PROTOCOL_JSON = os.path.normpath(os.path.join(
    os.path.dirname(__file__), "..", "..", "plugins", "obs-websocket",
    "docs", "generated", "protocol.json"))


def documented_request_types():
    try:
        with open(PROTOCOL_JSON) as f:
            proto = json.load(f)
        return [r["requestType"] for r in proto.get("requests", [])]
    except OSError:
        return []


async def test_stats_rescale_output():
    """GetStats must count the frames of a recording encoder that rescales
    the output (Settings > Output > Recording > Rescale Output). Such an
    encoder renders from a mix of its own, so the main video_t counts
    nothing while it records; OBS's Stats dock uses
    obs_get_video_frame_counts(), which includes those mixes."""
    c, _ = await new_identified_client(subscriptions=SUB_NONE)
    # The server accepts connections before the frontend finishes loading
    # and answers 207 (NotReady) until then; wait that out.
    for _ in range(120):
        r = await c.request("StartRecord")
        if resp_status(r).get("code") != 207:
            break
        await asyncio.sleep(0.5)
    check("stats:rescaled recording starts",
          resp_status(r).get("code") == STATUS_SUCCESS, json.dumps(r)[:300])
    active = False
    deadline = time.time() + 15
    while time.time() < deadline and not active:
        r = await c.request("GetRecordStatus")
        active = r.get("d", {}).get("responseData", {}).get("outputActive")
        await asyncio.sleep(0.25)
    await asyncio.sleep(3)
    r = await c.request("GetStats")
    data = r.get("d", {}).get("responseData", {})
    total = data.get("outputTotalFrames")
    skipped = data.get("outputSkippedFrames")
    # 30 fps for about 3 s: a recording that counts gives ~90 frames.
    check("stats:outputTotalFrames counts a rescaled recording",
          active and isinstance(total, int) and total >= 30,
          f"recording active={active} outputSkippedFrames={skipped} "
          f"outputTotalFrames={total}")
    check("stats:outputSkippedFrames <= outputTotalFrames",
          isinstance(skipped, int) and isinstance(total, int)
          and skipped <= total,
          f"{skipped} / {total}")
    await c.request("StopRecord", timeout=60.0)
    deadline = time.time() + 30
    while time.time() < deadline and active:
        r = await c.request("GetRecordStatus")
        active = r.get("d", {}).get("responseData", {}).get("outputActive")
        await asyncio.sleep(0.25)
    await c.close_conn()


# Advanced output, recording at 640x360 from a 1280x720 canvas, rescaled on
# the GPU (RecRescaleFilter 2 = bilinear), x264.
RESCALE_PROFILE = {
    "Video": {"BaseCX": "1280", "BaseCY": "720", "OutputCX": "1280",
              "OutputCY": "720", "FPSType": "0", "FPSCommon": "30"},
    "Output": {"Mode": "Advanced"},
    "AdvOut": {"RecType": "Standard", "RecEncoder": "obs_x264",
               "RecFormat2": "mkv", "RecTracks": "1",
               "RecRescaleFilter": "2", "RecRescaleRes": "640x360"},
}


async def run_all(obs_bin, skip_obs, skip_auth_phase):
    server_a = server_b = None
    if not skip_obs:
        server_a = ObsServer(obs_bin, PORT, password=None)
        server_a.start()

    try:
        print("== handshake (no auth) ==", flush=True)
        await test_handshake_no_password()
        await test_request_before_identify()
        await test_double_identify()
        await test_bad_rpc_version()
        await test_identify_missing_rpc_version()

        print("== protocol errors ==", flush=True)
        await test_unknown_opcode()
        await test_malformed_json()
        await test_excessive_nesting()
        await test_request_errors()

        print("== events ==", flush=True)
        await test_event_delivery()

        print("== batch ==", flush=True)
        await test_batch()

        print("== request coverage ==", flush=True)
        covered, seen = await test_full_coverage()
    finally:
        if server_a is not None:
            server_a.stop()

    if not (skip_obs and skip_auth_phase):
        if not skip_obs:
            wait_for_port_closed(PORT)
            server_b = ObsServer(obs_bin, PORT, password=PASSWORD)
            server_b.start()
        try:
            print("== handshake (auth required) ==", flush=True)
            await test_auth_handshake()
        finally:
            if server_b is not None:
                server_b.stop()

    if not skip_obs:
        wait_for_port_closed(PORT)
        home_c = tempfile.mkdtemp(prefix="obs-tier4-")
        rec_dir = os.path.join(home_c, "recordings")
        os.makedirs(rec_dir)
        profile = dict(RESCALE_PROFILE)
        profile["AdvOut"] = dict(profile["AdvOut"], RecFilePath=rec_dir)
        server_c = ObsServer(obs_bin, PORT, password=None, keep_home=home_c,
                             profile=profile)
        server_c.start()
        try:
            print("== stats with Rescale Output ==", flush=True)
            await test_stats_rescale_output()
        finally:
            server_c.stop()

    total = len(RESULTS)
    passed = sum(1 for _, ok, _ in RESULTS if ok)
    failed = [n for n, ok, _ in RESULTS if not ok]

    doc = documented_request_types()
    missing = sorted(set(doc) - seen) if doc else []
    print("=" * 60)
    print(f"checks: {passed}/{total} passed")
    print(f"request types exercised: {len(seen)}"
          + (f"/{len(doc)} documented" if doc else ""))
    if missing:
        print("not exercised: " + ", ".join(missing))
    if failed:
        print("failed checks:")
        for n in failed:
            print("  - " + n)
    return 1 if failed else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--obs", default=os.environ.get("OBS_BIN", "obs"),
                    help="obs binary to launch (default: obs on PATH)")
    ap.add_argument("--no-spawn", action="store_true",
                    help="do not launch OBS; connect to an already-running "
                         "server on the port (no-auth for phases A, then the "
                         "auth phase expects a passworded server - used "
                         "mainly for debugging)")
    ap.add_argument("--skip-auth-phase", action="store_true",
                    help="with --no-spawn only: skip the auth-required phase")
    args = ap.parse_args()
    sys.exit(asyncio.run(
        run_all(args.obs, args.no_spawn, args.skip_auth_phase)))


if __name__ == "__main__":
    main()
