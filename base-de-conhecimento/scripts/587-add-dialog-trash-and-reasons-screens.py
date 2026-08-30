# Add dialog, trash and reasons screens
# 28/08 17:45

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho = '''/* ============================================ Sessões e estatísticas de uso'''

novo = r'''/* ================================================= Exclusão: as duas formas
   O botão de excluir passou a perguntar DUAS coisas, e por isso deixou de ser
   um `confirm()`: qual das duas exclusões, e por quê.

   O padrão é a reversível, e isso é uma decisão e não uma preferência: a que
   não tem volta não pode ser escolhida por distração. Quem quer apagar de vez
   troca o modo e lê, em vermelho, o que vai acontecer.

   O campo do motivo aparece sempre. Fica obrigatório quando a tabela foi
   criada com «exigir motivo» — e aí o servidor recusa mesmo que a tela deixe
   passar, porque validação de tela é conveniência e não regra. */

/** Pergunta o modo e o motivo, e executa. `aoTerminar` roda se deu certo. */
async function dialogoExcluir(db, tab, rowid, aoTerminar) {
  // Quem sabe se a tabela exige motivo é o servidor. Perguntar antes evita
  // o vaivém de mandar sem motivo e receber a recusa.
  let exige = false;
  try {
    exige = !!(await api("motivos", { database: db, tabela: tab, limite: 1 })).motivo_obrigatorio;
  } catch (_) {
    // Sem poder de administrar não dá para saber, e isso não pode impedir a
    // exclusão: quem tem `excluir` continua excluindo. O servidor decide.
  }

  const fundo = document.createElement("div");
  fundo.className = "sobre";
  fundo.innerHTML = `
    <div class="caixa" role="dialog" aria-modal="true" aria-label="Excluir registro">
      <h3>Excluir o registro ${rowid}</h3>
      <div class="sub">${esc(db)} · ${esc(tab)}</div>
      <div class="modos" id="modosExc">
        <button class="modo escolhido" data-modo="suave">
          <span class="m-ico">🚫</span>
          <span><span class="m-rot">Marcar como excluído</span>
            <span class="m-diz">A linha some das listas e continua no
              <code>.reg</code>, inteira. Dá para restaurar.</span></span>
        </button>
        <button class="modo risco" data-modo="fisico">
          <span class="m-ico">🗑</span>
          <span><span class="m-rot">Excluir de vez</span>
            <span class="m-diz">Sai do <code>.reg</code>. A linha inteira vai
              antes para o <code>.trash</code>, de onde só o administrador
              lê — e o slot não é reaproveitado.</span></span>
        </button>
      </div>
      <label class="cmp"><span>Motivo${exige ? " <b>(obrigatório)</b>" : ""}</span>
        <input id="excMotivo" class="campo" maxlength="2000"
               placeholder="${exige ? "esta tabela exige o motivo escrito"
                                    : "opcional, mas fica registrado no .reason"}">
        <span class="leg">Vai para o <code>.reason</code> com a data, a hora e
          quem você é. Sobrevive à linha.</span></label>
      <div id="excRecado"></div>
      <div class="acoes">
        <button class="botao secundario" id="btExcNao">Cancelar</button>
        <button class="botao" id="btExcSim">Marcar como excluído</button>
      </div>
    </div>`;
  document.body.appendChild(fundo);

  const fechar = () => fundo.remove();
  let modo = "suave";

  const pintar = () => {
    $$("#modosExc .modo", fundo).forEach(b =>
      b.classList.toggle("escolhido", b.dataset.modo === modo));
    const sim = fundo.querySelector("#btExcSim");
    sim.textContent = modo === "suave" ? "Marcar como excluído" : "Excluir de vez";
    sim.classList.toggle("perigo", modo === "fisico");
  };
  $$("#modosExc .modo", fundo).forEach(b => b.onclick = ev => {
    ev.preventDefault(); modo = b.dataset.modo; pintar();
  });
  pintar();

  fundo.querySelector("#btExcNao").onclick = ev => { ev.preventDefault(); fechar(); };
  fundo.onclick = ev => { if (ev.target === fundo) fechar(); };
  document.addEventListener("keydown", function fuga(ev) {
    if (ev.key === "Escape") { fechar(); document.removeEventListener("keydown", fuga); }
  });
  setTimeout(() => fundo.querySelector("#excMotivo").focus(), 30);

  fundo.querySelector("#btExcSim").onclick = async ev => {
    ev.preventDefault();
    const motivo = fundo.querySelector("#excMotivo").value.trim();
    if (exige && !motivo) {
      fundo.querySelector("#excRecado").innerHTML =
        `<div class="aviso mal">esta tabela exige o motivo escrito</div>`;
      return;
    }
    // A que não tem volta pede uma confirmação a mais. A reversível não pede:
    // encher de confirmação o caminho seguro ensina a clicar em «sim» sem ler,
    // e aí a confirmação que importa também passa batida.
    if (modo === "fisico" &&
        !confirm(`Excluir de vez o registro ${rowid}?\n\n`
          + `A linha sai do .reg. Ela fica no .trash, que só o administrador `
          + `lê, e o slot NÃO é reaproveitado — é assim que a ordem de `
          + `digitação se mantém.`)) return;
    try {
      const r = await api("excluir",
        { database: db, tabela: tab, rowid, motivo, fisico: modo === "fisico" });
      fechar();
      avisar(r.modo === "suave"
        ? `registro ${rowid} marcado como excluído — dá para restaurar`
        : `registro ${rowid} excluído; a linha está no .trash`);
      if (aoTerminar) aoTerminar();
    } catch (err) {
      fundo.querySelector("#excRecado").innerHTML =
        `<div class="aviso mal">${esc(String(err))}</div>`;
    }
  };
}

/** Restaura uma linha marcada. */
async function restaurarLinha(db, tab, rowid, aoTerminar) {
  const motivo = prompt(`Restaurar o registro ${rowid}?\n\n`
    + `Motivo (fica no .reason):`, "");
  if (motivo === null) return;
  try {
    await api("restaurar", { database: db, tabela: tab, rowid, motivo: motivo.trim() });
    avisar(`registro ${rowid} restaurado`);
    if (aoTerminar) aoTerminar();
  } catch (err) { avisar(String(err), true); }
}

/* ------------------------------------------------------------- a lixeira
   Só quem administra chega aqui, e o servidor recusa quem não tem o poder.
   A tela diz isso com todas as letras em vez de mostrar uma lista vazia. */

async function telaLixeira(db, tab) {
  db = db || (est.atual || {}).db || databaseCorrente();
  tab = tab || (est.atual || {}).tab;
  if (!db || !tab) return avisar("escolha uma tabela primeiro", true);

  folha(`Lixeira de ${tab}`, `${esc(db)} · carregando…`,
    `<div class="centro">carregando…</div>`);
  let d;
  try {
    d = await api("lixeira", { database: db, tabela: tab, limite: 300 });
  } catch (e) {
    return folha(`Lixeira de ${tab}`, esc(db),
      `<div class="aviso mal">${esc(String(e))}</div>
       <p class="leg">O <code>.trash</code> guarda o dado que alguém mandou
         apagar. Ver o conteúdo dele exige <b>administrar</b> — quem só tem
         <code>ler</code> perdeu o direito àquela linha no instante em que ela
         foi excluída, e a lixeira devolveria o direito por outra porta.</p>
       <div class="acoes">
         <button class="botao secundario" id="btVoltaLix">← Gerir tabelas</button>
       </div>`) || ($("#btVoltaLix").onclick = () => gerirTabelasAtual());
  }

  const cols = (d.colunas || []).filter(c => !c.sistema);
  const itens = d.descartadas || [];
  $("#subtitulo").textContent =
    `${esc(db)} · ${fmt(d.total)} linha(s), ${fmtBytes(d.bytes)}`;

  $("#painel").innerHTML =
    `<p class="leg">Cada linha aqui saiu do <code>.reg</code>. Ela foi gravada
       neste arquivo <b>e o disco confirmou</b> antes de o slot ser liberado:
       entre perder e duplicar, o motor duplica.</p>
     ${itens.length === 0
       ? `<div class="aviso">A lixeira está vazia. Nenhuma linha desta tabela
            foi excluída de vez.</div>`
       : tabela(
           [{ t: "quando" }, { t: "rowid" }, { t: "quem" }, { t: "anexos" },
            ...cols.map(c => ({ t: c.rotulo || c.nome }))],
           itens,
           d_ => `<tr>
             <td class="dado">${esc(d_.quando)}</td>
             <td class="num">${d_.rowid}</td>
             <td>${esc(d_.usuario_nome || (d_.usuario ? "#" + d_.usuario : "—"))}</td>
             <td class="num">${d_.anexos}</td>
             ${d_.aviso
               ? `<td colspan="${cols.length}" class="mal">${esc(d_.aviso)}</td>`
               : cols.map(c => `<td>${celulaValor((d_.linha || {})[c.nome])}</td>`).join("")}
           </tr>`)}
     <div class="acoes">
       <button class="botao secundario" id="btVoltaLix">← Gerir tabelas</button>
       <button class="botao secundario" id="btVerMotivos">Ver os motivos</button>
       ${itens.length ? `<button class="botao perigo" id="btEsvaziar">Esvaziar a lixeira</button>` : ""}
     </div>`;

  $("#btVoltaLix").onclick = () => gerirTabelasAtual();
  $("#btVerMotivos").onclick = () => telaMotivos(db, tab);
  const bt = $("#btEsvaziar");
  if (bt) bt.onclick = async () => {
    const motivo = prompt(
      `Esvaziar a lixeira de ${tab}?\n\n`
      + `Isto apaga ${d.total} linha(s) DE VEZ — daqui não volta.\n`
      + `O expurgo fica registrado no .reason antes de o dado sair.\n\n`
      + `Motivo (obrigatório):`, "");
    if (motivo === null) return;
    if (!motivo.trim()) return avisar("o motivo é obrigatório para esvaziar", true);
    try {
      const r = await api("esvaziar_lixeira", { database: db, tabela: tab, motivo: motivo.trim() });
      avisar(`${fmt(r.apagadas)} linha(s) apagadas de vez`);
      telaLixeira(db, tab);
    } catch (err) { avisar(String(err), true); }
  };
}

/* ------------------------------------------------------------ os motivos */

async function telaMotivos(db, tab) {
  db = db || (est.atual || {}).db || databaseCorrente();
  tab = tab || (est.atual || {}).tab;
  if (!db || !tab) return avisar("escolha uma tabela primeiro", true);

  folha(`Motivos de ${tab}`, `${esc(db)} · carregando…`,
    `<div class="centro">carregando…</div>`);
  let d;
  try {
    d = await api("motivos", { database: db, tabela: tab, limite: 500 });
  } catch (e) {
    folha(`Motivos de ${tab}`, esc(db),
      `<div class="aviso mal">${esc(String(e))}</div>
       <p class="leg">O <code>.reason</code> costuma ser mais revelador que o
         registro que foi excluído — «fraude», «pedido de remoção do titular»,
         «duplicidade com o contrato X». Por isso exige <b>administrar</b>.</p>
       <div class="acoes">
         <button class="botao secundario" id="btVoltaMot">← Gerir tabelas</button>
       </div>`);
    $("#btVoltaMot").onclick = () => gerirTabelasAtual();
    return;
  }

  const CORES = { suave: "var(--aviso)", fisica: "var(--log)",
                  restauracao: "var(--ok)", expurgo: "var(--vermelhao)" };
  const regs = d.motivos || [];
  $("#subtitulo").textContent = `${esc(db)} · ${fmt(d.total)} registro(s)`;
  $("#painel").innerHTML =
    `<p class="leg">O <code>.log</code> diz que houve uma exclusão no rowid tal,
       no instante tal. O que ele não tem onde dizer — o evento dele tem 36
       bytes fixos — é <b>por quê</b>. É este arquivo.
       ${d.motivo_obrigatorio
         ? `<b>Esta tabela exige motivo escrito</b> em toda exclusão.`
         : `Esta tabela não exige motivo; o campo continua valendo.`}</p>
     ${regs.length === 0
       ? `<div class="aviso">Nenhuma exclusão registrada nesta tabela.</div>`
       : tabela(
           [{ t: "quando" }, { t: "o quê" }, { t: "rowid" }, { t: "registro" },
            { t: "quem" }, { t: "motivo" }],
           regs,
           m => `<tr>
             <td class="dado">${esc(m.quando)}</td>
             <td><b style="color:${CORES[m.tipo] || "var(--texto-2)"}">${esc(m.tipo)}</b></td>
             <td class="num">${m.rowid || "—"}</td>
             <td class="dado">${esc(m.identidade || "—")}</td>
             <td>${esc(m.usuario_nome || (m.usuario ? "#" + m.usuario : "—"))}</td>
             <td>${esc(m.motivo || "—")}</td>
           </tr>`)}
     <div class="acoes">
       <button class="botao secundario" id="btVoltaMot">← Gerir tabelas</button>
       <button class="botao secundario" id="btVerLixo">Ver a lixeira</button>
     </div>`;
  $("#btVoltaMot").onclick = () => gerirTabelasAtual();
  $("#btVerLixo").onclick = () => telaLixeira(db, tab);
}

/* ============================================ Sessões e estatísticas de uso'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
