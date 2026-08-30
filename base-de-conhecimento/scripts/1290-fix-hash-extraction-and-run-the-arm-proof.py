# Fix hash extraction and run the ARM proof
# 30/08 15:40

p='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/prova-arm.sh'
s=open(p).read()
velho='H=$($Q "$BIN" --senha <<< "segredo1" | tr -d \'\\r\\n\' | tail -c 200)'
novo='H=$($Q "$BIN" --senha <<< "segredo1" | grep -oE \'pbkdf2-sha256[^"]+\' | head -1)'
assert s.count(velho)==1, repr(s[s.find('H=$'):s.find('H=$')+90])
open(p,'w').write(s.replace(velho,novo))
print("extracao corrigida")
