#!/usr/bin/env python3
"""Call a tool on the running Nitro Hero dev MCP: mcp_call.py <tool> ['{"json":"args"}']"""
import json, os, sys, urllib.request

descriptor = json.load(open(os.path.join(os.environ["LOCALAPPDATA"], "Nitro Hero", "dev-mcp.json")))
body = {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
    "name": sys.argv[1], "arguments": json.loads(sys.argv[2]) if len(sys.argv) > 2 else {}}}
request = urllib.request.Request(descriptor["url"], json.dumps(body).encode(), {
    "Authorization": "Bearer " + descriptor["token"], "Content-Type": "application/json"})
reply = json.load(urllib.request.urlopen(request, timeout=15))
result = reply.get("result", reply)
sys.stdout.reconfigure(encoding="utf-8")
print(json.dumps(result.get("structuredContent", result), indent=1, ensure_ascii=False))
