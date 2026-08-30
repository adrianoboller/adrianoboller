# Inspect blacklist file and CLI commands
# 27/08 19:29

import socket, json
for i in range(4):
    try:
        s=socket.create_connection(("127.0.0.1",5002),timeout=10); f=s.makefile("rwb")
        f.write((json.dumps({"token":"ERRADO","op":"ping"})+"\n").encode()); f.flush()
        r=json.loads(f.readline().decode())
        print(f"  tentativa {i+1}: {r.get('erro','')[:78]}")
        s.close()
    except Exception as e:
        print(f"  tentativa {i+1}: {e}")
