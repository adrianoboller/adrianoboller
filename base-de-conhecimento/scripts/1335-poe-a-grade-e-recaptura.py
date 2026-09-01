# Poe a grade e recaptura
# 01/09 18:39

from pathlib import Path
p = Path("docs/dossie/trio-de-motores.py")
s = p.read_text(encoding="utf-8")
s = s.replace(
    """.trio{margin:18px 0}""",
    """/* Dois paineis por linha onde couber. Sem a grade eles empilham em largura
   cheia, e um viewBox de 460 esticado a 1.000 px vira uma figura do tamanho
   da tela para dizer tres numeros. */
.trio{margin:18px 0;display:grid;gap:16px;
  grid-template-columns:repeat(auto-fit,minmax(330px,1fr))}""",
)
p.write_text(s, encoding="utf-8")
print("grade posta")
