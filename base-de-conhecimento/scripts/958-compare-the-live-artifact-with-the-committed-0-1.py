# Compare the live artifact with the committed 0.16.0 file
# 29/08 00:49

import pathlib,subprocess
live = pathlib.Path("$LIVE").read_text()
i = live.index("<title>")
corpo_live = live[i:]
if corpo_live.rstrip().endswith("</body></html>"):
    corpo_live = corpo_live.rstrip()[:-len("</body></html>")]
antigo = subprocess.run(["git","show","a7faee3:phxsql/docs/dossie/dossie-phxsql-0.15.html"],capture_output=True,text=True,cwd="/home/user/adrianoboller").stdout
print("bytes live:", len(corpo_live.strip()), " bytes commit 0.16.0:", len(antigo.strip()))
print("IGUAIS" if corpo_live.strip()==antigo.strip() else "DIFEREM")
