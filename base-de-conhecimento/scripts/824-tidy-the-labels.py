# Tidy the labels
# 28/08 20:42

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
s = s.replace('<text x="398" y="179" text-anchor="middle" font-size="9" opacity=".55">44 bytes + carimbo em ms</text>',
              '<text x="398" y="179" text-anchor="middle" font-size="9" opacity=".55">44 bytes, carimbo em ms</text>')
s = s.replace('<text x="398" y="270" text-anchor="middle" font-size="8.5" opacity=".5">44 → 223 bytes por evento</text>',
              '<text x="398" y="270" text-anchor="middle" font-size="9" opacity=".55">44 → 223 bytes por evento</text>')
p.write_text(s)
