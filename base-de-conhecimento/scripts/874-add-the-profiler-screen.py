# Add the profiler screen
# 28/08 22:59

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

# icone: um pulso de monitor
antigo = """const ICO = {
  energia:"""
novo = """const ICO = {
  pulso: `<path d="M2.5 12h4l2-6 3.5 12 2.5-7 1.8 3.4h5.2" fill="none" stroke-width="1.7" stroke-linejoin="round" stroke-linecap="round"/>`,
  energia:"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """  { ico:"replica",  rot:"Replicação", cor:"var(--ndx)",    faz:verReplicacao },"""
novo = """  { ico:"replica",  rot:"Replicação", cor:"var(--ndx)",    faz:verReplicacao },
  { ico:"pulso",    rot:"Profiler",   cor:"var(--acao-consultar)", faz:verProfiler },"""
assert antigo in s
s = s.replace(antigo, novo)

# a tela, junto das outras de administracao
antigo = """async function repararPeloMenu() {"""
novo = """/* O PROFILER: o que está chegando pela porta, antes de virar dado.
 *
 * A tela atualiza sozinha enquanto está ligada, pedindo só o que ainda não
 * viu (`desde_serial`) — rebaixar o anel inteiro a cada segundo seria a tela
 * gerando mais tráfego do que a que ela observa. */
let profTimer = null;
let profVisto = 0;
let profLinhas = [];

async function verProfiler() {
  clearInterval(profTimer);
  profVisto = 0; profLinhas = [];
  const bancos = await api("bancos").catch(() => []);
  const e = await api("profiler", { max: 200 }).catch(() => ({ ligado:false, filtro:{} }));
  const f = e.filtro || {};

  folha("Profiler", "o que chega pela porta 5000, antes de virar dado",
    `<div class="acoes prof-topo">
       <label class="mini-campo">Database
         <select id="pfDb"><option value="">— todos —</option>
           ${(bancos || []).map(b => { const n = b.nome || b;
             return `<option ${f.database === n ? "selected" : ""}>${esc(n)}</option>`; }).join("")}
         </select></label>
       <label class="mini-campo">Usuário
         <input id="pfUsr" placeholder="— todos —" value="${esc(f.usuario || "")}"></label>
       <label class="mini-campo">Operação
         <input id="pfOp" placeholder="— todas —" value="${esc(f.operacao || "")}"></label>
       <label class="mini-campo">Guardar
         <input id="pfTeto" type="number" min="10" max="20000" value="${esc(String(e.guardar || 500))}"></label>
       <label class="mini-campo largo">Arquivo de log (.txt) — opcional
         <input id="pfArq" placeholder="/var/log/phxsql-monitor.txt"
                value="${esc(e.arquivo || "")}"></label>
       <label class="mini-campo caixa"><input type="checkbox" id="pfEscrita"
         ${f.so_escrita ? "checked" : ""}> só escrita</label>
       <button class="botao consultar" id="pfLigar">${e.ligado ? "Reiniciar" : "Ligar"}</button>
       <button class="botao excluir" id="pfParar" ${e.ligado ? "" : "disabled"}>Parar</button>
       <button class="botao secundario" id="pfLimpar">Limpar</button>
     </div>
     <div id="pfEstado"></div>
     <div class="rolo"><table class="prof">
       <thead><tr><th>hora</th><th>ip</th><th>usuário</th><th>operação</th>
         <th>alvo</th><th class="num">ms</th><th></th><th class="num">bytes</th>
         <th>pedido, como chegou</th></tr></thead>
       <tbody id="pfCorpo"></tbody></table></div>
     <div class="nota">
       <span class="t">A senha não passa por aqui</span>
       <p>O pedido é mostrado como chegou pelo soquete — <b>menos</b> os campos
       sensíveis. <code>senha</code>, <code>token</code>, <code>prova</code> e
       os outros viram <code>"***"</code> <b>antes</b> de encostar na memória
       ou no arquivo. Pedido que não é JSON válido não vira texto nenhum: vira
       o tamanho dele, porque não há como tapar um campo numa estrutura que
       não se lê. Há teste que falha se uma senha aparecer.</p>
       <p>O ponto de captura é uma linha depois do <code>read_line</code> e uma
       antes do despacho: <b>nada foi gravado ainda</b>. Por isso o pedido que
       trava aparece na lista como «em curso» — que é justamente o que se
       quer achar.</p>
     </div>`);

  $("#pfLigar").onclick = async () => {
    try {
      const r = await api("profiler_ligar", {
        database: $("#pfDb").value, usuario: $("#pfUsr").value.trim(),
        operacao: $("#pfOp").value.trim(), so_escrita: $("#pfEscrita").checked,
        arquivo: $("#pfArq").value.trim(), guardar: +$("#pfTeto").value || 500,
      });
      avisar(r.arquivo ? `observando · gravando em ${r.arquivo}` : "observando");
      profVisto = 0; profLinhas = [];
      verProfiler();
    } catch (err) { avisar(String(err), true); }
  };
  $("#pfParar").onclick = async () => {
    const r = await api("profiler_desligar");
    avisar(`parado — ${fmt(r.observados)} evento(s) observados`);
    verProfiler();
  };
  $("#pfLimpar").onclick = async () => {
    await api("profiler_limpar"); profVisto = 0; profLinhas = [];
    $("#pfCorpo").innerHTML = ""; 
  };

  const pintar = d => {
    const est = $("#pfEstado"); if (!est) return false;
    est.innerHTML = d.ligado
      ? `<div class="aviso bem">observando desde <b>${esc(d.desde)}</b> ·
           ${fmt(d.observados)} evento(s)${d.esquecidos ? ` · ${fmt(d.esquecidos)} saíram do anel` : ""}
           ${d.arquivo ? ` · gravando em <code>${esc(d.arquivo)}</code>` : " · só em memória"}</div>`
      : `<div class="aviso">parado. Escolha os filtros e clique em <b>Ligar</b>.</div>`;
    return d.ligado;
  };

  const atualizar = async () => {
    let d;
    try { d = await api("profiler", { max: 200, desde_serial: profVisto }); }
    catch { clearInterval(profTimer); return; }
    if (!$("#pfCorpo")) { clearInterval(profTimer); return; }   // saiu da tela
    pintar(d);
    for (const ev of (d.eventos || []).slice().reverse()) {
      profVisto = Math.max(profVisto, ev.serial);
      profLinhas.unshift(ev);
    }
    if (profLinhas.length > 300) profLinhas.length = 300;
    $("#pfCorpo").innerHTML = profLinhas.map(ev => {
      const estado = ev.ok === null || ev.ok === undefined
        ? `<span class="pino">em curso</span>`
        : ev.ok ? `<span class="pino ok">ok</span>`
                : `<span class="pino mal" title="${esc(ev.erro || "")}">erro</span>`;
      return `<tr class="${ev.ok === false ? "prof-mal" : ""}">
        <td class="dado">${esc((ev.quando || "").slice(11))}</td>
        <td class="dado">${esc(ev.ip)}</td>
        <td>${esc(ev.usuario || "—")}</td>
        <td class="dado"><b>${esc(ev.op)}</b></td>
        <td class="dado">${esc(ev.database ? (ev.tabela ? ev.database + "." + ev.tabela : ev.database) : "—")}</td>
        <td class="num">${ev.ms === null || ev.ms === undefined ? "—" : fmt(ev.ms)}</td>
        <td>${estado}</td>
        <td class="num">${fmt(ev.bytes)}</td>
        <td class="dado prof-pedido" title="${esc(ev.pedido)}">${esc(ev.pedido)}</td></tr>`;
    }).join("");
  };
  await atualizar();
  profTimer = setInterval(atualizar, 1000);
}

async function repararPeloMenu() {"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
