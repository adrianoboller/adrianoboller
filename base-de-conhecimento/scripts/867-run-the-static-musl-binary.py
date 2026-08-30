# Run the static musl binary
# 28/08 22:54

import json, socket
s = socket.create_connection(("127.0.0.1", 5920)); f = s.makefile("rwb")
f.write((json.dumps({"op":"ping","token":"demo"}) + "\n").encode()); f.flush()
print("ping no binario musl:", json.loads(f.readline().decode())["resultado"])
