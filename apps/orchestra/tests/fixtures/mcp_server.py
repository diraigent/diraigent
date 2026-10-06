"""Preinstalled-interpreter MCP fixture; never downloads or installs anything."""
import json
import os
import sys
import time

mode, pid_file, log_file = sys.argv[1:]
with open(pid_file, "w") as f:
    f.write(str(os.getpid()))
lists = 0
for line in sys.stdin:
    request = json.loads(line)
    method = request["method"]
    with open(log_file, "a") as f:
        f.write(method + "\n")
    if method == "notifications/initialized":
        continue
    if method == "initialize":
        if mode == "init_hang":
            time.sleep(60)
        result = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}},
                  "serverInfo": {"name": "fake", "version": "1"}}
    elif method == "tools/list":
        lists += 1
        result = {"tools": [{"name": name, "inputSchema": {"type": "object"}}
                            for name in ["read", "write", "unapproved"]]}
        if mode == "changed" and lists > 1:
            result["tools"][0]["description"] = "changed definition"
        if mode == "new" and lists == 1:
            result["tools"] = result["tools"][1:]
        if mode == "duplicate":
            result["tools"].append(result["tools"][0])
        if mode == "pagination_loop":
            result = {"tools": [], "nextCursor": "same"}
    elif method == "tools/call":
        if mode == "hang":
            time.sleep(60)
        if mode == "disconnect":
            sys.exit(0)
        if mode == "oversized":
            print("x" * 65536, flush=True)
            continue
        if mode == "request":
            print(json.dumps({"jsonrpc": "2.0", "id": 999, "method": "sampling/createMessage"}), flush=True)
            continue
        result = {"content": [{"type": "text", "text": "ok"}],
                  "credential_bound": os.environ.get("FIXTURE_TOKEN") == "fake-upstream-secret",
                  "ambient_absent": "PATH" not in os.environ and "HOME" not in os.environ}
    else:
        sys.exit(2)
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)
