# Verify the reload fix with a running server
# 27/08 19:30

import socket, json
def uma(p):
    try:
        s=socket.create_connection(("127.0.0.1",5002),timeout=10); f=s.makefile("rwb")
        f.write((json.dumps(p)+"\n").encode()); f.flush()
        r=json.loads(f.readline().decode()); s.close(); return r
    except Exception as e: return {"erro":str(e)}
print("=== bloqueio por tentativas: token errado, limite 3 ===")
for i in range(4):
    r=uma({"token":"ERRADO","op":"ping"})
    print(f"  tentativa {i+1}: {r.get('erro','')[:76]}")
