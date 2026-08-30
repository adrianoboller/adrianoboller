# Add factory keys and run gates
# 29/08 23:40

import io
p="phxsql/crates/phxsql-server/src/idiomas.rs"
s=io.open(p,encoding="utf-8").read()
ancora='    texto!("tela.mi_tema", '
i=s.index(ancora); fim=s.index("\n",i)+1
novas = [
 ("tela.mi_nova_aba","Nova aba nesta região","Nouvel onglet dans cette zone","New tab in this region","Nuova scheda in questa regione","Neuer Tab in diesem Bereich","Nueva pestaña en esta región"),
 ("tela.mi_fechar_aba","Fechar esta aba","Fermer cet onglet","Close this tab","Chiudi questa scheda","Diesen Tab schließen","Cerrar esta pestaña"),
 ("tela.mi_uma_regiao","Uma região","Une zone","One region","Una regione","Ein Bereich","Una región"),
 ("tela.mi_duas_regioes","Duas regiões","Deux zones","Two regions","Due regioni","Zwei Bereiche","Dos regiones"),
 ("tela.mi_tres_regioes","Três regiões","Trois zones","Three regions","Tre regioni","Drei Bereiche","Tres regiones"),
 ("tela.mi_quatro_regioes","Quatro regiões","Quatre zones","Four regions","Quattro regioni","Vier Bereiche","Cuatro regiones"),
 ("tela.mi_soltar","Soltar esta tela numa janela","Détacher cet écran dans une fenêtre","Detach this screen into a window","Stacca questa schermata in una finestra","Diese Ansicht in ein Fenster lösen","Soltar esta pantalla en una ventana"),
 ("tela.mi_alinhar","Alinhar com as bordas dos monitores","Aligner sur les bords des écrans","Align with the monitor edges","Allinea ai bordi dei monitor","An den Monitorrändern ausrichten","Alinear con los bordes de los monitores"),
 ("tela.mi_sobre_multitela","Sobre o modo multitela…","À propos du mode multi-écran…","About multi-screen mode…","Informazioni sulla modalità multischermo…","Über den Multiscreen-Modus…","Acerca del modo multipantalla…"),
]
bloco = "    // A area de trabalho multitela entrou depois da regra petrea, entao ela\n"
bloco += "    // nasce na fabrica em vez de nascer cravada em portugues.\n"
for n in novas:
    bloco += '    texto!("%s", "%s", "%s", "%s", "%s", "%s", "%s"),\n' % n
s = s[:fim] + bloco + s[fim:]
io.open(p,"w",encoding="utf-8").write(s)
print("nove chaves acrescentadas a fabrica")
