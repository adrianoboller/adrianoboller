# Finish refreshing and regenerate the dossier panel
# 29/08 02:01

import pathlib
trocas = [
  ("contra 18.773 linhas/s, com as três réplicas competindo pela mesma máquina.",
   "contra 28.914 linhas/s, com as três réplicas competindo pela mesma máquina."),
  ('<div><div class="v">18.773</div><div class="r">linhas/s no master</div></div>',
   '<div><div class="v">28.914</div><div class="r">linhas/s no master</div></div>'),
  ('<div><div class="v">4.273</div><div class="r">eventos/s por réplica</div></div>',
   '<div><div class="v">4.357</div><div class="r">eventos/s por réplica</div></div>'),
  ("<p>4.273 eventos/s contra 18.773 linhas/s, com as três competindo pela mesma",
   "<p>4.357 eventos/s contra 28.914 linhas/s, com as três competindo pela mesma"),
  ("<tr><td>Réplica acompanhar a escrita do master · 4.273/s contra 18.773/s</td>",
   "<tr><td>Réplica acompanhar a escrita do master · 4.357/s contra 28.914/s</td>"),
  ("<li><strong>A réplica não acompanha a escrita do master.</strong> 4.273\n    eventos/s contra 18.773 linhas/s: sob carga sustentada ela fica para trás.",
   "<li><strong>A réplica não acompanha a escrita do master.</strong> 4.357\n    eventos/s contra 28.914 linhas/s: sob carga sustentada ela fica para trás."),
  ("'E a réplica aplica mais devagar que o master escreve: 4.273/s contra 18.773/s.'",
   "'E a réplica aplica mais devagar que o master escreve: 4.357/s contra 28.914/s.'"),
]
for f in ["docs/REPLICACAO.md","docs/dossie/dossie-phxsql-0.15.html","docs/video/roteiro.mjs"]:
    q = pathlib.Path(f); s = q.read_text(); antes = s
    for a,b in trocas: s = s.replace(a,b)
    if s != antes: q.write_text(s); print("atualizado:", f)
