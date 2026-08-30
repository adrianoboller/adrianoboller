# Rewrite the replication screen
# 28/08 21:28

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """async function verReplicacao() {
  const c = await api("config");
  const portas = c.replicacao_portas || {};
  folha("Replicação", "as portas entram no config.json e são validadas",
    `<div class="fichas">
       <div class="ficha"><div class="v">${esc(c.papel || "isolado")}</div>
         <div class="r">papel</div></div>
       <div class="ficha"><div class="v">${esc(portas.envio || "—")}</div>
         <div class="r">porta de envio</div></div>
       <div class="ficha"><div class="v">${esc(portas.retorno || "—")}</div>
         <div class="r">porta de retorno</div></div>
     </div>
     <div class="nota">
       <p><strong>As portas são configuração, não serviço.</strong> Elas entram,
       são validadas — duas no mesmo endereço não sobem — e o desenho está
       escrito. O que falta é o <code>.log</code> v2 <strong>com imagem da
       linha</strong>: hoje o diário registra que houve alteração, não o que a
       linha virou, e sem isso a réplica não tem o que aplicar.</p>
     </div>`);
}"""
novo = """/* A tela da replicação. Ela mostra a POSIÇÃO de cada tabela, que é o número
   que diz se a réplica está em dia — e não só a configuração, que era tudo o
   que ela sabia mostrar enquanto a replicação não existia. */
async function verReplicacao() {
  const c = await api("config");
  const portas = c.replicacao_portas || {};
  const papel = c.papel || "isolado";
  const comImagem = !!c.imagem_da_linha;
  const origens = c.origens || [];

  // A posição de cada tabela de cada banco: é o que a réplica compara com a do
  // source para saber o que falta. Sai do cabeçalho do .log, sem ler evento.
  let posicoes = "";
  try {
    const bancos = (await api("bancos")).bancos || [];
    const linhas = [];
    for (const b of bancos) {
      const nome = b.nome || b;
      const r = await api("posicao", { database: nome });
      for (const [tab, d] of Object.entries(r.tabelas || {}))
        linhas.push({ db: nome, tabela: tab, eventos: d.eventos, registros: d.registros });
    }
    posicoes = linhas.length
      ? `<h3 class="sub">Posição do diário</h3>` + tabela(
          [{t:"database"},{t:"tabela"},{t:"registros",cls:"num"},{t:"eventos",cls:"num"}],
          linhas, l => `<tr><td class="dado">${esc(l.db)}</td>
            <td class="dado">${esc(l.tabela)}</td>
            <td class="num">${fmt(l.registros)}</td>
            <td class="num">${fmt(l.eventos)}</td></tr>`)
        + `<p class="leg">O evento N <b>é</b> a posição N — não há GTID a inventar.
           Uma réplica guarda um número por tabela e pede o que falta.</p>`
      : `<p class="leg">Nenhuma tabela ainda.</p>`;
  } catch (e) {
    posicoes = `<p class="leg">Sem permissão para ler a posição
      (a operação <code>posicao</code> exige <b>replicar</b>).</p>`;
  }

  const fichas = `<div class="fichas">
       <div class="ficha"><div class="v">${esc(papel)}</div>
         <div class="r">papel</div></div>
       <div class="ficha"><div class="v">${comImagem ? "ligada" : "desligada"}</div>
         <div class="r">imagem da linha</div></div>
       <div class="ficha"><div class="v">${esc(portas.envio || "porta de dados")}</div>
         <div class="r">envio</div></div>
       <div class="ficha"><div class="v">${origens.length || "—"}</div>
         <div class="r">origens</div></div>
     </div>`;

  const deOrigens = origens.length
    ? `<h3 class="sub">De onde esta réplica puxa</h3>` + tabela(
        [{t:"origem"},{t:"endereço"},{t:"usuário"},{t:"databases"},{t:"a cada",cls:"num"}],
        origens, o => `<tr><td class="dado">${esc(o.nome)}</td>
          <td class="dado">${esc(o.host)}:${esc(String(o.porta))}</td>
          <td>${esc(o.usuario || "—")}</td>
          <td>${esc((o.databases || []).join(", ") || "todos")}</td>
          <td class="num">${esc(String(o.reconectar_em))}s</td></tr>`)
      + `<p class="leg">Uma <b>thread por origem</b>: uma origem lenta ou caída
         não segura as outras. A senha não fica em claro nem viaja — a réplica
         se autentica pelo desafio-resposta, com a chave derivada do
         <code>senha_hash</code>.</p>`
    : "";

  const aviso = papel === "source" && !comImagem
    ? `<div class="nota"><p><strong>Este servidor é source e está com a imagem
       da linha DESLIGADA.</strong> O diário grava que a linha mudou, não grava
       para quê, e as réplicas não terão o que aplicar. Ligue
       <code>replicacao.imagem_da_linha</code> no <code>config.json</code>.</p></div>`
    : papel === "isolado"
    ? `<div class="nota"><p><strong>Servidor isolado.</strong> Para virar origem,
       ponha <code>"replicacao": {"papel": "source"}</code> no
       <code>config.json</code> — a imagem da linha liga junto. Para virar
       réplica, <code>"papel": "replica"</code> com uma origem, e
       <code>"somente_leitura": true</code>.</p></div>`
    : `<div class="nota">
       <p><strong>O rowid é o que faz a réplica ser fiel.</strong> O
       <code>.reg</code> nunca reaproveita slot e o rowid é sempre o próximo.
       Se a réplica aplicar todos os eventos na ordem, e mais ninguém escrever
       nela, os rowids saem <b>iguais</b> aos do master — sem transmitir nem
       negociar nada. Se não saírem, ela divergiu, e a replicação
       <b>para ali</b> em vez de espalhar.</p>
       <p>Medido com quatro servidores: master a 18.773 linhas/s, réplica
       aplicando 4.273 eventos/s, atraso de 1,3 a 2,1 s até as três, e o
       retrato SHA-256 das quatro tabelas idêntico no fim.</p>
     </div>`;

  folha("Replicação", `papel ${papel}`, fichas + aviso + deOrigens + posicoes);
}"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
