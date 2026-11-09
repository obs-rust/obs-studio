#!/usr/bin/env -S uv run --script
"""MCP probe transport tests, without devices, GUI or optional numerical packages."""
import ast
import asyncio
import json
from pathlib import Path
import unittest

# Load only the transport class: measurement dependencies are intentionally optional.
namespace = {"json": json, "asyncio": asyncio}
source = ast.parse(Path(__file__).with_name("vcam_av_sync.py").read_text())
for node in source.body:
    if isinstance(node, ast.ClassDef) and node.name == "McpSession":
        exec(compile(ast.Module(body=[node], type_ignores=[]), "vcam_av_sync.py", "exec"), namespace)
McpSession = namespace["McpSession"]

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

if __name__ == "__main__": unittest.main()
