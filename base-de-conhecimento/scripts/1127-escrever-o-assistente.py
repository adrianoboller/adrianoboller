# Escrever o assistente
# 29/08 11:45

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

# 1. o botao nas duas entradas da tela
velho='''       <button class="botao secundario" id="btDef">Definições…</button>
     </div>'''
novo='''       <button class="botao secundario" id="btDef">Definições…</button>
       <button class="botao incluir" id="btAssistente">Assistente…</button>
     </div>'''
assert s.count(velho)==1
s=s.replace(velho,novo)
s=s.replace('''$("#btDef").onclick = () => telaDbLinkDefinicoes();
  $("#btSql").onclick = () => telaDbLinkSql(DBL.ligacao);''','''$("#btDef").onclick = () => telaDbLinkDefinicoes();
  $("#btAssistente").onclick = () => assistenteDbLink();
  $("#btSql").onclick = () => telaDbLinkSql(DBL.ligacao);''')

velho2='''       <div class="acoes"><button class="botao" id="btDef">Cadastrar uma ligação…</button></div>`);
    $("#btDef").onclick = () => telaDbLinkDefinicoes();
    return;'''
novo2='''       <div class="acoes">
         <button class="botao incluir" id="btAssist0">Assistente de conexão…</button>
         <button class="botao secundario" id="btDef">Cadastrar à mão…</button>
       </div>`);
    $("#btAssist0").onclick = () => assistenteDbLink();
    $("#btDef").onclick = () => telaDbLinkDefinicoes();
    return;'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# 2. o assistente, logo antes da telaDbLinkSql
anc='''async function telaDbLinkSql(nome) {'''
assert s.count(anc)==1
wizard = r'''/* O assistente do DbLink: conexao -> teste -> base -> tabelas ligadas -> job.
   Cada passo so avanca com o anterior PROVADO (o teste tem de passar, a
   ligacao tem de gravar), porque um assistente que deixa pular o teste e um
   cadastro com etapas. */
async function assistenteDbLink() {
  const az = {
    def: { nome: "", motor: "mysql", host: "127.0.0.1", porta: 3306,
           usuario: "", senha: "", database: "", somente_leitura: true },
    base: "", tabelas: [], escolhidas: new Map(), local_db: "",
    job: { criar: true, minutos: 5 },
  };

  const fundo = document.createElement("div");
  fundo.className = "sobre";
  fundo.innerHTML = `<div class="caixa larga" role="dialog" aria-modal="true"
    aria-label="Assistente de conexão DbLink"><div id="azCorpo"></div></div>`;
  document.body.appendChild(fundo);
  const corpo = fundo.querySelector("#azCorpo");
  const fechar = () => fundo.remove();
  fundo.onclick = ev => { if (ev.target === fundo) fechar(); };

  const molde = (titulo, passo, html, acoes) => {
    corpo.innerHTML = `
      <h3>${esc(titulo)}</h3>
      <div class="sub">assistente de conexão · passo ${passo} de 5</div>
      ${html}
      <div id="azRecado"></div>
      <div class="acoes">${acoes}</div>`;
  };
  const recado = (texto, mal) => {
    $("#azRecado").innerHTML =
      `<div class="aviso ${mal ? "mal" : ""}">${esc(texto)}</div>`;
  };

  // ---------------------------------------------------- passo 1: a conexao
  const passo1 = () => {
    const d = az.def;
    molde("Para onde vamos ligar", 1, `
      <div class="par-campos">
        <label class="cmp"><span>Apelido da ligação</span>
          <input id="azNome" class="campo" value="${esc(d.nome)}" placeholder="erp, crm…"></label>
        <label class="cmp"><span>Motor</span>
          <select id="azMotor" class="campo">
            <option value="mysql" ${d.motor === "mysql" ? "selected" : ""}>MySQL(R) / MariaDB(R)</option>
            <option value="postgres" ${d.motor === "postgres" ? "selected" : ""}>PostgreSQL(R)</option>
          </select></label>
        <label class="cmp"><span>Host</span>
          <input id="azHost" class="campo" value="${esc(d.host)}"></label>
        <label class="cmp"><span>Porta</span>
          <input id="azPorta" class="campo" type="number" value="${d.porta}"></label>
        <label class="cmp"><span>Usuário lá</span>
          <input id="azUsu" class="campo" value="${esc(d.usuario)}"></label>
        <label class="cmp"><span>Senha lá</span>
          <input id="azSen" class="campo" type="password"></label>
      </div>
      <label class="cmp caixa-marcar"><input type="checkbox" id="azEscreve"
        ${d.somente_leitura ? "" : "checked"}>
        <span>Esta ligação <b>pode escrever</b> no outro banco
        <span class="leg">necessário para a sincronia empurrar; desmarcado, ela só puxa</span></span></label>`,
      `<button class="botao secundario" id="azSair">Cancelar</button>
       <button class="botao incluir" id="azIr1">Testar a conexão →</button>`);
    $("#azSair").onclick = fechar;
    $("#azIr1").onclick = async () => {
      d.nome = $("#azNome").value.trim();
      d.motor = $("#azMotor").value;
      d.host = $("#azHost").value.trim();
      d.porta = +$("#azPorta").value || 3306;
      d.usuario = $("#azUsu").value.trim();
      const senha = $("#azSen").value;
      d.somente_leitura = !$("#azEscreve").checked;
      if (!d.nome) return recado("dê um apelido à ligação", true);
      try {
        const pedido = { op: "dblink_salvar", nome: d.nome, motor: d.motor,
          host: d.host, porta: d.porta, usuario: d.usuario,
          somente_leitura: d.somente_leitura };
        if (senha) pedido.senha = senha;
        await api("dblink_salvar", pedido);
        passo2();
      } catch (e) { recado(String(e), true); }
    };
  };

  // ------------------------------------------------------ passo 2: o teste
  const passo2 = async () => {
    molde("Testando a conexão", 2,
      `<div class="centro">falando com ${esc(az.def.host)}:${az.def.porta}…</div>`,
      `<button class="botao secundario" id="azVolta">← Voltar</button>`);
    $("#azVolta").onclick = passo1;
    try {
      const t = await api("dblink_testar", { dblink: az.def.nome });
      molde("Conectou", 2, `
        <div class="aviso bom"><p><b>${esc(t.motor)} ${esc(t.versao)}</b> respondeu
        em ${t.ms} ms. Você é <code>${esc(t.usuario_efetivo || t.usuario || "?")}</code>
        do outro lado${t.database ? `, na base <code>${esc(t.database)}</code>` : ""}.</p></div>`,
        `<button class="botao secundario" id="azVolta">← Voltar</button>
         <button class="botao incluir" id="azIr2">Escolher a base →</button>`);
      $("#azVolta").onclick = passo1;
      $("#azIr2").onclick = passo3;
    } catch (e) {
      molde("Não conectou", 2,
        `<div class="aviso mal">${esc(String(e))}</div>`,
        `<button class="botao secundario" id="azVolta">← Corrigir a conexão</button>`);
      $("#azVolta").onclick = passo1;
    }
  };

  // ------------------------------------------------------- passo 3: a base
  const passo3 = async () => {
    molde("Qual base do outro lado?", 3, `<div class="centro">listando…</div>`,
      `<button class="botao secundario" id="azVolta">← Voltar</button>`);
    $("#azVolta").onclick = passo2;
    try {
      const b = await api("dblink_bancos", { dblink: az.def.nome });
      const servico = new Set(["information_schema", "mysql", "performance_schema", "sys"]);
      const bases = (b.bancos || []).filter(x => !servico.has(x))
        .concat((b.bancos || []).filter(x => servico.has(x)));
      molde("Qual base do outro lado?", 3, `
        <label class="cmp"><span>Base</span>
          <select id="azBase" class="campo">${bases.map(x =>
            `<option ${x === az.base ? "selected" : ""}>${esc(x)}</option>`).join("")}
          </select></label>`,
        `<button class="botao secundario" id="azVolta">← Voltar</button>
         <button class="botao incluir" id="azIr3">Escolher as tabelas →</button>`);
      $("#azVolta").onclick = passo2;
      $("#azIr3").onclick = async () => {
        az.base = $("#azBase").value;
        az.def.database = az.base;
        await api("dblink_salvar", { nome: az.def.nome, motor: az.def.motor,
          host: az.def.host, porta: az.def.porta, usuario: az.def.usuario,
          database: az.base, somente_leitura: az.def.somente_leitura });
        passo4();
      };
    } catch (e) { recado(String(e), true); }
  };

  // -------------------------------------------- passo 4: as tabelas ligadas
  const passo4 = async () => {
    molde("Quais tabelas ficam ligadas?", 4, `<div class="centro">listando…</div>`, ``);
    try {
      const r = await api("dblink_tabelas", { dblink: az.def.nome, database: az.base });
      az.tabelas = r.tabelas || [];
      const linhas = az.tabelas.map(t => {
        const e = az.escolhidas.get(t.nome);
        return `<tr>
          <td class="esc"><input type="checkbox" data-t="${esc(t.nome)}" ${e ? "checked" : ""}></td>
          <td><b>${esc(t.nome)}</b> <span class="leg">${t.registros_estimados} reg</span></td>
          <td><select data-sentido="${esc(t.nome)}">
            <option value="dois" ${!e || e.sentido === "dois" ? "selected" : ""}>os dois sentidos</option>
            <option value="puxar" ${e && e.sentido === "puxar" ? "selected" : ""}>só puxar de lá</option>
            <option value="empurrar" ${e && e.sentido === "empurrar" ? "selected" : ""}>só empurrar daqui</option>
          </select></td>
          <td><select data-dono="${esc(t.nome)}">
            <option value="aqui" ${!e || e.dono === "aqui" ? "selected" : ""}>conflito: aqui vence</option>
            <option value="la" ${e && e.dono === "la" ? "selected" : ""}>conflito: lá vence</option>
          </select></td></tr>`;
      }).join("");
      molde("Quais tabelas ficam ligadas?", 4, `
        <label class="cmp"><span>Gravar aqui no database</span>
          <input id="azLocal" class="campo" value="${esc(az.local_db || az.base)}">
          <span class="leg">a tabela local nasce com as mesmas colunas e a mesma chave da prima</span></label>
        <div class="rolo"><table class="conf">
          <thead><tr><th></th><th>tabela de lá</th><th>sentido</th><th>conflito</th></tr></thead>
          <tbody>${linhas}</tbody></table></div>`,
        `<button class="botao secundario" id="azVolta">← Voltar</button>
         <button class="botao incluir" id="azIr4">Ligar as marcadas →</button>`);
      $("#azVolta").onclick = passo3;
      $("#azIr4").onclick = async () => {
        az.local_db = $("#azLocal").value.trim();
        const marcadas = [...corpo.querySelectorAll('input[type=checkbox][data-t]:checked')]
          .map(c => c.dataset.t);
        if (!marcadas.length) return recado("marque ao menos uma tabela", true);
        const tabelas = marcadas.map(nome => ({
          remota: nome, local_database: az.local_db,
          sentido: corpo.querySelector(`[data-sentido="${CSS.escape(nome)}"]`).value,
          dono: corpo.querySelector(`[data-dono="${CSS.escape(nome)}"]`).value,
        }));
        try {
          const lig = await api("dblink_ligar", { dblink: az.def.nome, tabelas });
          az.escolhidas = new Map(lig.ligadas.map(l => [l.remota, l]));
          passo5(lig.ligadas);
        } catch (e) { recado(String(e), true); }
      };
    } catch (e) { recado(String(e), true); }
  };

  // ------------------------------------------------- passo 5: o job e o fim
  const passo5 = (ligadas) => {
    const resumo = ligadas.map(l =>
      `<li><code>${esc(l.remota)}</code> ⇄ <code>${esc(l.local_database)}.${esc(l.local_tabela)}</code>
       — chave <code>${esc(l.chave)}</code>, ${esc(l.sentido)}, dono ${esc(l.dono)}${
         l.tabela_criada ? " · <b>tabela criada</b>" : ""}</li>`).join("");
    molde("Sincronia automática", 5, `
      <div class="aviso bom"><ul class="lista-limpa">${resumo}</ul></div>
      <label class="cmp caixa-marcar"><input type="checkbox" id="azJob" checked>
        <span>Criar o <b>job de sincronia</b>
        <span class="leg">roda a convergência sozinho; exclusão não viaja, e o
        conflito é por linha — vence o dono escolhido</span></span></label>
      <label class="cmp"><span>A cada quantos minutos</span>
        <input id="azMin" class="campo" type="number" min="1" value="${az.job.minutos}"></label>`,
      `<button class="botao secundario" id="azFim0">Concluir sem job</button>
       <button class="botao incluir" id="azFim">Concluir →</button>`);
    $("#azFim0").onclick = () => concluir(null);
    $("#azFim").onclick = async () => {
      if (!$("#azJob").checked) return concluir(null);
      const minutos = Math.max(1, +$("#azMin").value || 5);
      try {
        await api("job_salvar", { nome: `sincronia-${az.def.nome}`,
          descricao: `convergência com ${az.def.nome} (assistente do DbLink)`,
          cada_minutos: minutos, ligado: true,
          usuario: (est.usuario && est.usuario.login) || "",
          pedido: { op: "dblink_sincronizar", dblink: az.def.nome } });
        concluir(minutos);
      } catch (e) { recado(String(e), true); }
    };
  };

  const concluir = async (minutos) => {
    molde("Pronto", 5, `<div class="centro">primeira rodada…</div>`, ``);
    let rodada = "";
    try {
      const r = await api("dblink_sincronizar", { dblink: az.def.nome });
      rodada = (r.sincronizadas || []).map(x =>
        `<li><code>${esc(x.remota)}</code>: ${x.puxadas_novas} novas puxadas,
         ${x.empurradas} empurradas, ${x.iguais} iguais, ${x.conflitos} conflito(s)</li>`).join("");
    } catch (e) { rodada = `<li class="mal">${esc(String(e))}</li>`; }
    molde("Pronto", 5, `
      <div class="aviso bom"><p><b>A ligação ${esc(az.def.nome)} está no ar.</b>${
        minutos ? ` O job <code>sincronia-${esc(az.def.nome)}</code> roda a cada ${minutos} min.` :
        " Sem job: sincronize pela operação <code>dblink_sincronizar</code> quando quiser."}</p>
      <p>Primeira rodada:</p><ul class="lista-limpa">${rodada}</ul></div>`,
      `<button class="botao" id="azFechar">Fechar</button>`);
    $("#azFechar").onclick = () => { fechar(); telaDbLink(az.def.nome); };
  };

  passo1();
}

'''
s=s.replace(anc, wizard+anc)
io.open(p,'w',encoding='utf-8').write(s)
print('assistente escrito')
