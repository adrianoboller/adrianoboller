# Test retention and the admin level
# 27/08 21:17

import socket, json
s=socket.create_connection(('127.0.0.1',5400),5); f=s.makefile('rwb')
def pede(d):
    f.write((json.dumps(d)+'\n').encode()); f.flush(); return json.loads(f.readline().decode())
print('login ana (nivel admin):', pede({"token":"tk","op":"login","usuario":"ana","senha":"x"})["ok"])
r=pede({"token":"tk","op":"backup","destino":"backups","zip":True})
print('backup manual:', json.dumps(r.get("resultado", r))[:150])
