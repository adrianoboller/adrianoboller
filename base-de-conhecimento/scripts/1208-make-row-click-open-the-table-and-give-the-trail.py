# Make row click open the table and give the trail its own button
# 29/08 20:58

import io
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()

# 1) a linha ganha um botao proprio para a trilha, e o clique na linha vai para
#    a Estrutura -- que e onde se MARCA, e e o que a nota da tela promete.
velho = """             <td>${esc(a.coluna.descricao || "")}</td></tr>`)"""
novo = """             <td>${esc(a.coluna.descricao || "")}</td>
             <td><button class="botao mini consultar trilha-lg"
               data-db="${esc(a.base)}" data-tab="${esc(a.tabela)}"
               >quem mexeu</button></td></tr>`)"""
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2 = """                 {t:"tipo"},{t:"índices"},{t:"descrição"}],"""
novo2  = """                 {t:"tipo"},{t:"índices"},{t:"descrição"},{t:"trilha"}],"""
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# 2) o clique na linha abre a tabela apontada; o botao abre a trilha.
velho3 = """  $$("#painel .linha-lg").forEach(tr => tr.onclick = () =>
    telaTrilhaLgpd(tr.dataset.db, tr.dataset.tab));"""
novo3 = """  // O clique na LINHA leva ao lugar onde se AGE -- a Estrutura da tabela, com
  // a coluna LGPD -- porque e o que a nota logo acima promete. A trilha ("quem
  // mexeu") e outra pergunta e ganhou botao proprio: a bateria de frontend
  // pegou esta divergencia no merge de duas frentes que consertaram a mesma
  // tela por caminhos diferentes, e cada uma ligou o clique ao seu destino.
  $$("#painel .linha-lg").forEach(tr => tr.onclick = ev => {
    if (ev.target.closest(".trilha-lg")) return;
    abrirTabela(tr.dataset.db, tr.dataset.tab).then(() => irAba("estrutura"));
  });
  $$("#painel .trilha-lg").forEach(b => b.onclick = ev => {
    ev.stopPropagation();
    telaTrilhaLgpd(b.dataset.db, b.dataset.tab);
  });"""
assert s.count(velho3)==1
s=s.replace(velho3,novo3)
io.open(p,"w",encoding="utf-8").write(s)
print("ok")
