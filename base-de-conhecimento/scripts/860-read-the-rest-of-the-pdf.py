# Read the rest of the PDF
# 28/08 22:46

import pymupdf
d = pymupdf.open("/root/.claude/uploads/34595649-0af6-575a-8f79-80dbe8cb7a5d/845655a1-hfsql_US.pdf")
for i in range(8, 13):
    t = d[i].get_text().strip()
    print(f"\n{'='*70}\n--- pagina {i+1} --- ({len(t)} chars)")
    print(t[:4200])
