"""Minimal client for FigCraft's headless MCP server (`figcraft-cli mcp`, JSON-RPC over stdio).

Usage:
    from fc import FigCraft
    fc = FigCraft("/path/to/figcraft-cli")
    fc.run("node.create", {"type": "frame", ...})   # engine command -> result
    fc.batch([...])                                  # command.batch, one undo step
"""

import json
import os
import subprocess


class FigCraftError(RuntimeError):
    pass


class FigCraft:
    def __init__(self, cli, env=None):
        e = dict(os.environ)
        e.update(env or {})
        # nice: the build machine is shared with other work.
        self.p = subprocess.Popen(
            ["nice", "-n", "10", cli, "mcp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            env=e,
            text=True,
            bufsize=1,
        )
        self.n = 0
        self._rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                 "clientInfo": {"name": "pingcraft-design", "version": "0.1"}})
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, msg):
        self.p.stdin.write(json.dumps(msg) + "\n")
        self.p.stdin.flush()

    def _rpc(self, method, params):
        self.n += 1
        self._send({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params})
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise FigCraftError("figcraft-cli mcp exited")
            msg = json.loads(line)
            if msg.get("id") == self.n:
                if "error" in msg:
                    raise FigCraftError(json.dumps(msg["error"]))
                return msg["result"]

    def tool(self, name, args):
        r = self._rpc("tools/call", {"name": name, "arguments": args})
        if r.get("isError"):
            raise FigCraftError(f"{name} {json.dumps(args)[:400]}: {r['content'][0].get('text')}")
        out = []
        for c in r.get("content", []):
            if c.get("type") == "text":
                try:
                    out.append(json.loads(c["text"]))
                except ValueError:
                    out.append(c["text"])
            else:
                out.append(c)
        return out[0] if len(out) == 1 else out

    def run(self, command, params=None):
        return self.tool("run_command", {"command": command, "params": params or {}})

    def batch(self, commands, label=None):
        args = {"commands": commands}
        if label:
            args["label"] = label
        return self.tool("batch", args)

    def close(self):
        try:
            self.p.stdin.close()
            self.p.wait(timeout=10)
        except Exception:
            self.p.kill()
