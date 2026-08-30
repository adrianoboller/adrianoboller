# Add key generation to the CLI
# 27/08 20:44

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
import re
m=re.search(r'--senha[^\n]*\n', s)
print(repr(m.group(0)) if m else 'sem USO')
