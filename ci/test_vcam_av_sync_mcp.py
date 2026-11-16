#!/usr/bin/env -S uv run --script
"""MCP probe transport tests, without devices, GUI or optional numerical packages."""
import asyncio
import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import AsyncMock, Mock, patch

# Numerical analysis is not exercised by these transport tests.
spec = importlib.util.spec_from_file_location("vcam_probe_under_test", Path(__file__).with_name("vcam_av_sync.py"))
probe = importlib.util.module_from_spec(spec)
with patch.dict(sys.modules, {"numpy": Mock()}):
    spec.loader.exec_module(probe)
McpSession = probe.McpSession

class Reader:
    def __init__(self, reply): self.reply = reply
    async def readline(self): return json.dumps(self.reply).encode() + b"\n"
class Writer:
    def __init__(self): self.data = b""
    def write(self, data): self.data += data
    async def drain(self): pass

class McpProbeTransportTest(unittest.IsolatedAsyncioTestCase):
    async def test_structured_reply_and_wire_request(self):
        writer = Writer()
        session = McpSession(Reader({"jsonrpc":"2.0", "id":1, "result":{"structuredContent":{"active":True}}}), writer)
        self.assertEqual(await session.call("vcam_start", {"wait":True}), {"active":True})
        request = json.loads(writer.data)
        self.assertEqual(request["params"], {"name":"vcam_start", "arguments":{"wait":True}})

    async def test_domain_failure_and_wrong_id_are_errors(self):
        for reply in [
            {"jsonrpc":"2.0", "id":1, "result":{"isError":True, "structuredContent":{"code":207, "message":"not ready"}}},
            {"jsonrpc":"2.0", "id":9, "result":{}},
        ]:
            with self.assertRaises(RuntimeError):
                await McpSession(Reader(reply), Writer()).call("vcam_start")

    async def test_probe_waits_and_always_removes_its_own_source(self):
        async def reply(name, arguments=None):
            return {"scene_get_current": {"name":"program"}, "screenshot":{"width":1280,"height":720},
                    "vmic_status":{"active":True}, "wait_for":{"satisfied":True}}.get(name, {})
        session = Mock()
        session.call = AsyncMock(side_effect=reply)
        session.writer.wait_closed = AsyncMock()
        recorder = Mock(returncode=0)
        recorder.poll.return_value = 0
        args = Mock(mcp_socket="/test/socket", seconds=0, device="/dev/test", source="test-mic")
        with patch.object(probe, "connect_mcp", AsyncMock(return_value=session)), \
             patch.object(probe.subprocess, "Popen", return_value=recorder), \
             patch.object(probe.asyncio, "sleep", AsyncMock()), \
             patch.object(probe, "analyze", return_value={"av_offset_ms":0}):
            result = await probe.measure_mcp(args, Path("probe.mkv"), Path("capture.mkv"))
        calls = session.call.await_args_list
        names = [call.args[0] for call in calls]
        self.assertLess(names.index("wait_for"), names.index("media_restart"))
        self.assertEqual(names[-1], "source_remove")
        created = next(call.args[1]["name"] for call in calls if call.args[0] == "source_create")
        self.assertEqual(calls[-1].args[1]["name"], created)
        self.assertEqual(result["transport"], "mcp")
        session.writer.close.assert_called_once()

    async def test_auto_falls_back_only_when_websocket_connection_is_absent(self):
        import tempfile
        args = Mock(transport="auto", url="ws://absent")
        websocket = Mock(connect=AsyncMock(side_effect=ConnectionRefusedError()))
        with tempfile.TemporaryDirectory() as directory, \
             patch.dict(sys.modules, {"websockets":websocket}), \
             patch.object(probe.tempfile, "mkdtemp", return_value=directory), \
             patch.object(probe, "write_clip"), \
             patch.object(probe, "measure_mcp", AsyncMock(return_value={"transport":"mcp"})) as fallback:
            self.assertEqual(await probe.measure(args), {"transport":"mcp"})
            fallback.assert_awaited_once()
            args.transport = "websocket"
            with self.assertRaises(ConnectionRefusedError):
                await probe.measure(args)

    async def test_capture_that_ignores_sigint_is_killed_and_reaped(self):
        recorder = Mock()
        recorder.poll.return_value = None
        with patch.object(probe.asyncio, "to_thread", AsyncMock(side_effect=[probe.subprocess.TimeoutExpired("ffmpeg", 10), 0])) as wait:
            await probe.stop_capture(recorder)
        recorder.send_signal.assert_called_once_with(probe.signal.SIGINT)
        recorder.kill.assert_called_once()
        self.assertEqual(wait.await_count, 2)
        self.assertEqual([entry.kwargs for entry in wait.await_args_list], [{"timeout":10}, {"timeout":5}])

if __name__ == "__main__": unittest.main()
