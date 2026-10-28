"""REL-01 (#115), criterion 4: the MCP server of a release archive answers.

    python3 -I packaging/mcp-smoke.py BINARY VERSION TOOLS_SOURCE

Starts `BINARY mcp` on an empty temporary folder, sends `initialize` with
protocol revision 2025-11-25 and `tools/list`, and checks that the server names
itself `apiforge` at VERSION and lists exactly the tools of the `TOOLS` array in
TOOLS_SOURCE (crates/apiforge-cli/tests/mcp.rs). It then closes stdin and
expects exit code 0. Standard library only; it runs on Linux and Windows.
"""

import json
import queue
import re
import subprocess
import sys
import tempfile
import threading

REVISION = "2025-11-25"
TIMEOUT = 30


def expected_tools(source):
    with open(source, encoding="utf-8") as file:
        text = file.read()
    found = re.search(r"const TOOLS: \[&str; (\d+)\] = \[(.*?)\];", text, re.S)
    if not found:
        sys.exit(f"{source} has no `const TOOLS: [&str; N]` array")
    names = re.findall(r'"([a-z_]+)"', found.group(2))
    if len(names) != int(found.group(1)):
        sys.exit(f"{source} declares {found.group(1)} tools but lists {len(names)}")
    return names


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    binary, version, source = sys.argv[1:]
    tools = expected_tools(source)
    with tempfile.TemporaryDirectory() as folder:
        server = subprocess.Popen(
            [binary, "mcp", "--folder", folder],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        lines = queue.Queue()

        def read_stdout():
            for line in server.stdout:
                lines.put(line)
            lines.put(None)

        threading.Thread(target=read_stdout, daemon=True).start()
        errors = []
        stderr = threading.Thread(
            target=lambda: errors.append(server.stderr.read()), daemon=True
        )
        stderr.start()

        def send(message):
            server.stdin.write(json.dumps(message).encode() + b"\n")
            server.stdin.flush()

        def response(request_id):
            while True:
                try:
                    line = lines.get(timeout=TIMEOUT)
                except queue.Empty:
                    server.kill()
                    sys.exit(f"no answer to request {request_id} in {TIMEOUT} s")
                if line is None:
                    sys.exit(f"the server closed stdout before answering {request_id}")
                message = json.loads(line)
                if message.get("id") == request_id:
                    return message

        try:
            send(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": REVISION,
                        "capabilities": {},
                        "clientInfo": {"name": "mcp-smoke", "version": version},
                    },
                }
            )
            result = response(1).get("result") or {}
            info = result.get("serverInfo") or {}
            if result.get("protocolVersion") != REVISION:
                sys.exit(f"initialize answered revision {result.get('protocolVersion')!r}")
            if (info.get("name"), info.get("version")) != ("apiforge", version):
                sys.exit(f"initialize answered server {info!r}, not apiforge {version}")
            send({"jsonrpc": "2.0", "method": "notifications/initialized"})

            send({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
            listed = [tool["name"] for tool in response(2)["result"]["tools"]]
            if sorted(listed) != sorted(tools):
                sys.exit(f"tools/list answered {listed}, expected {tools}")

            server.stdin.close()
            code = server.wait(timeout=TIMEOUT)
        finally:
            if server.poll() is None:
                server.kill()
                server.wait()
        if code != 0:
            stderr.join(timeout=TIMEOUT)
            text = b"".join(errors).decode(errors="replace")
            sys.exit(f"the server exited with {code} at EOF:\n{text}")
    print(f"apiforge {version}: initialize {REVISION} and {len(tools)} tools, exit 0 at EOF")


if __name__ == "__main__":
    main()
