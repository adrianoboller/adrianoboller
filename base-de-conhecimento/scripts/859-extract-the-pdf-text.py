# Extract the PDF text
# 28/08 22:46

import fitz
d = fitz.open("/root/.claude/uploads/34595649-0af6-575a-8f79-80dbe8cb7a5d/845655a1-hfsql_US.pdf")
print("paginas:", len(d))
for i, p in enumerate(d):
    t = p.get_text().strip()
    print(f"\n{'='*70}\n--- pagina {i+1} --- ({len(t)} chars, {len(p.get_images())} imagens)")
    print(t[:1500])
    if i >= 7: break
