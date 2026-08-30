# Render the key pages
# 28/08 22:46

import pymupdf
d = pymupdf.open("/root/.claude/uploads/34595649-0af6-575a-8f79-80dbe8cb7a5d/845655a1-hfsql_US.pdf")
for i in [2, 5, 6]:
    p = d[i]
    pix = p.get_pixmap(matrix=pymupdf.Matrix(1.6, 1.6))
    pix.save(f"pag{i+1}.png")
    print(f"pag{i+1}.png {pix.width}x{pix.height}")
