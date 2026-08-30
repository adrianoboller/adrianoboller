# Hide the system column from the record form
# 28/08 17:58

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho='''  folha(novo ? `Nova linha em ${tab}` : `${tab} · registro ${rowid}`,
    novo ? `${db} · em branco significa nulo`
         : `${db} · rowid ${rowid} — a posição física, que não muda`,
    `<form class="ficha-edit" id="fichaEdit" autocomplete="off">
       ${e.colunas.map(c => campoDaColuna(c, linha[c.nome])).join("")}
     </form>
     <div class="acoes">
       <button class="botao" id="btSalvar">${novo ? "Incluir" : "Salvar"}</button>
       ${novo ? "" : `<button class="botao perigo" id="btExcluir">Excluir</button>`}
       <button class="botao secundario" id="btVoltar">Voltar</button>
       <span class="leg" id="recadoFicha"></span>
     </div>`);

  const valores = () => e.colunas.map(c => {
    const el = $(`#f_${c.nome}`);
    const v = el.value.trim();
    if (v === "") return null;              // vazio e NULL, sempre
    if (c.tipo === "Bool") return v === "true";
    if (/^Int|^UInt|^Sequence$/.test(c.tipo)) return Number(v);
    if (c.tipo === "Real4" || c.tipo === "Real8") return Number(v);
    return v;                                // texto, data, decimal, uuid…
  });'''

novo='''  // A coluna de sistema NAO vira campo de formulario. Quem manda nela e o
  // botao de excluir e o de restaurar; oferecer um `select` com «verdadeiro /
  // falso» convidaria a excluir uma linha digitando, sem motivo registrado e
  // sem passar por lugar nenhum que registre.
  const sistema = (e.colunas.find(c => c.sistema) || {}).nome || "softdeleted";
  const editaveis = e.colunas.filter(c => c.nome !== sistema);
  const marcada = linha[sistema] === true;

  folha(novo ? `Nova linha em ${tab}` : `${tab} · registro ${rowid}`,
    novo ? `${db} · em branco significa nulo`
         : `${db} · rowid ${rowid} — a posição física, que não muda`,
    `${marcada ? `<div class="aviso">
        <b class="marca-excluida">Esta linha está marcada como excluída.</b>
        Ela não aparece nas listas e continua inteira no <code>.reg</code>.
        <button class="botao mini" id="btRestaurarFicha">restaurar</button>
      </div>` : ""}
     <form class="ficha-edit" id="fichaEdit" autocomplete="off">
       ${editaveis.map(c => campoDaColuna(c, linha[c.nome])).join("")}
     </form>
     <div class="acoes">
       <button class="botao" id="btSalvar">${novo ? "Incluir" : "Salvar"}</button>
       ${novo || marcada ? "" : `<button class="botao perigo" id="btExcluir">Excluir</button>`}
       <button class="botao secundario" id="btVoltar">Voltar</button>
       <span class="leg" id="recadoFicha"></span>
     </div>`);

  // A lista sai SEM a coluna de sistema, e o motor entende: numa inclusao ela
  // nasce falsa, e numa alteracao mantem o que a linha ja tinha. E o que
  // impede um «salvar» de rotina de ressuscitar linha marcada.
  const valores = () => editaveis.map(c => {
    const el = $(`#f_${c.nome}`);
    const v = el.value.trim();
    if (v === "") return null;              // vazio e NULL, sempre
    if (c.tipo === "Bool") return v === "true";
    if (/^Int|^UInt|^Sequence$/.test(c.tipo)) return Number(v);
    if (c.tipo === "Real4" || c.tipo === "Real8") return Number(v);
    return v;                                // texto, data, decimal, uuid…
  });

  if (marcada) $("#btRestaurarFicha").onclick = ev => {
    ev.preventDefault();
    restaurarLinha(db, tab, rowid, () => {
      est.esquemaAtual = null;
      abrirFicha(db, tab, rowid);
    });
  };'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
