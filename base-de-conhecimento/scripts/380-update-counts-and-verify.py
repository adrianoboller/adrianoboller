# Update counts and verify
# 28/08 13:38

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md'); s = p.read_text()
import re
s = re.sub(r'\| ☑️ \| 15 \| Organograma, fluxograma e dossiê \| \d+ seções, \d+ figuras',
           '| ☑️ | 15 | Organograma, fluxograma e dossiê | $SEC seções, $FIG figuras', s)
p.write_text(s)
