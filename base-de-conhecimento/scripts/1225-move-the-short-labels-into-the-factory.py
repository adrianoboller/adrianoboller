# Move the short labels into the factory
# 29/08 23:43

import io
# 1) as quatro etiquetas curtas entram na fabrica
p="phxsql/crates/phxsql-server/src/idiomas.rs"
s=io.open(p,encoding="utf-8").read()
anc='    texto!("tela.mi_sobre_multitela", '
i=s.index(anc); fim=s.index("\n",i)+1
novas=[
 ("tela.alternar_tema","Alternar tema","Changer de thème","Toggle theme","Cambia tema","Design wechseln","Cambiar tema"),
 ("tela.abas_da_regiao","Telas abertas nesta região","Écrans ouverts dans cette zone","Screens open in this region","Schermate aperte in questa regione","Geöffnete Ansichten in diesem Bereich","Pantallas abiertas en esta región"),
 ("tela.cores_de_fabrica","Voltar às cores de fábrica","Revenir aux couleurs d'origine","Back to factory colours","Torna ai colori di fabbrica","Zurück zu den Werksfarben","Volver a los colores de fábrica"),
]
bloco="".join('    texto!("%s", "%s", "%s", "%s", "%s", "%s", "%s"),\n' % n for n in novas)
s=s[:fim]+bloco+s[fim:]
io.open(p,"w",encoding="utf-8").write(s)

# 2) a tela passa a pedir por chave
p="phxsql/crates/phxsql-server/ui/index.html"
h=io.open(p,encoding="utf-8").read()
h=h.replace('id="btTema" title="Alternar tema"','id="btTema" data-txt-al="tela.alternar_tema" title="Alternar tema"',1)
h=h.replace('aria-label="Telas abertas nesta região"','data-txt-al="tela.abas_da_regiao" aria-label="Telas abertas nesta região"',1)
h=h.replace('id="cfCoresFabrica" type="button">Voltar às','id="cfCoresFabrica" type="button" data-txt="tela.cores_de_fabrica">Voltar às',1)
io.open(p,"w",encoding="utf-8").write(h)
print("tres etiquetas na fabrica")
