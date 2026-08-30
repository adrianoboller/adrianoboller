# Wire the submit path
# 28/08 18:55

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
# lerForm: o campo de digitos pode nao existir na alfanumerica -- ja e opcional
import re
print('nt_dig guardado?', 'if ($("#nt_dig"))' in s)
