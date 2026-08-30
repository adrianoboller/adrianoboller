# Read the cluster section
# 28/08 22:47

import pymupdf, re
d = pymupdf.open("/root/.claude/uploads/34595649-0af6-575a-8f79-80dbe8cb7a5d/845655a1-hfsql_US.pdf")
t = d[2].get_text()
i = t.upper().find("CLUSTER")
print(t[i-200:i+2600])
