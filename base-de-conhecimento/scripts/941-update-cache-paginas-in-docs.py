# Update cache_paginas in docs
# 29/08 00:35

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
s = s.replace('''      "cache_paginas": 4096,''', '''      "cache_paginas": 2048,''')
p.write_text(s)
