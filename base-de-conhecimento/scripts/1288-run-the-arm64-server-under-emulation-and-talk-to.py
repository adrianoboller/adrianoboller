# Run the ARM64 server under emulation and talk to it
# 30/08 15:38

import json, socket
s=socket.create_connection(("127.0.0.1",6992), timeout=20)
f=s.makefile("rw")
def call(d):
    s.sendall((json.dumps(d)+"\n").encode()); return json.loads(f.readline())
print("  ping:", call({"op":"ping"}).get("ok"))
