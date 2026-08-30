# Build the import screen
# 28/08 19:26

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho='''  { ico:"importar", rot:"Importar",   cor:"var(--bin)",    faz:null,
    falta:"Ler CSV, .tps do TopSpeed(R) ou outra base e gravar como tabela "
        + "PhxSql. Depende de decidir os formatos de entrada." },'''
novo='''  { ico:"importar", rot:"Importar",   cor:"var(--bin)",    faz:() => telaImportar() },'''
assert velho in s
s=s.replace(velho,novo,1)

# a tela, logo antes da de exportar
velho2='''/* ================================================= Exclusão: as duas formas'''
novo2='''/* ============================================== Importar: colar uma carga
   O caminho de volta do Exportar. Os cinco formatos de TEXTO voltam; o XLSX e
   o DOCX não — ler ZIP de XML é outro trabalho, e quem tem planilha salva como
   CSV em dois cliques.

   A tela adivinha o formato pelo primeiro caractere, e mostra o que entendeu
   ANTES de gravar. Uma carga que entra errada é pior que uma que não entra:
   dar para conferir a prévia é metade da funcionalidade. */

const FORMATOS_ENTRADA = [
  ["auto", "adivinhar", "pelo primeiro caractere do que foi colado"],
  ["csv",  "CSV",  "separado por ; ou , — com aspas quando o campo tem separador dentro"],
  ["txt",  "TXT",  "separado por tabulação"],
  ["json", "JSON", "lista de objetos, ou o que o Exportar escreve"],
  ["xml",  "XML",  "elementos repetidos com campos simples dentro"],
  ["html", "HTML", "a primeira <table> do documento"],
];

async function telaImportar(db, tab) {
  db = db || (est.atual || {}).db || databaseCorrente();
  tab = tab || (est.atual || {}).tab;
  if (!db || !tab) return avisar("escolha uma tabela primeiro", true);

  folha(`Importar para ${tab}`, `${esc(db)} · cole a carga e confira antes de gravar`,
    `<div class="form-dbl">
       <label class="cmp"><span>Formato</span>
         <select id="impFmt" class="campo">${FORMATOS_ENTRADA.map(([k, r, d]) =>
           `<option value="${k}">${esc(r)} — ${esc(d)}</option>`).join("")}</select></label>
       <label class="cmp"><span>Ao encontrar erro</span>
         <select id="impErro" class="campo">
           <option value="parar">parar na primeira linha ruim</option>
           <option value="seguir">pular a linha ruim e seguir</option>
         </select>
         <span class="leg"><b>Não há transação.</b> O que entrou antes do erro
           fica gravado — não há como desfazer, porque o <code>.reg</code> não
           reaproveita slot e desfazer deixaria buracos.</span></label>
     </div>
     <label class="cmp largo"><span>A carga</span>
       <textarea id="impTexto" rows="12" class="campo mono"
         placeholder="id;nome;cidade&#10;1;Adriano;Blumenau&#10;2;Maria;Itajaí"></textarea>
       <span class="leg">A primeira linha é o <b>cabeçalho</b>, e os nomes são
         casados com as colunas da tabela <b>por nome</b> — não por posição.
         Coluna que a tabela não tem é erro; coluna que falta fica nula.</span></label>
     <div class="acoes">
       <button class="botao secundario" id="btPrever">Conferir</button>
       <button class="botao" id="btImportar" disabled>Gravar</button>
       <button class="botao secundario" id="btVoltaImp">← Gerir tabelas</button>
     </div>
     <div id="impSaida"></div>`);

  $("#btVoltaImp").onclick = () => gerirTabelasAtual();

  const previa = () => {
    const texto = $("#impTexto").value;
    if (!texto.trim()) { avisar("cole a carga primeiro", true); return null; }
    return texto;
  };

  $("#btPrever").onclick = async () => {
    const texto = previa();
    if (texto === null) return;
    // A prévia é lida NO NAVEGADOR pelo mesmo caminho do servidor? Não —
    // seria uma segunda implementação, e as duas divergiriam. Manda para o
    // servidor com `conferir`, que lê e devolve sem gravar nada.
    $("#impSaida").innerHTML = `<div class="centro">lendo…</div>`;
    try {
      const r = await api("importar_conferir", {
        database: db, tabela: tab, texto,
        formato: $("#impFmt").value,
      });
      const cols = r.colunas || [];
      $("#impSaida").innerHTML =
        `<div class="aviso">Entendi como <b>${esc(r.formato)}</b>:
           ${fmt(r.linhas_lidas)} linha(s), ${cols.length} coluna(s).
           ${r.desconhecidas && r.desconhecidas.length
             ? `<b class="marca-excluida">Colunas que a tabela não tem:
                 ${r.desconhecidas.map(esc).join(", ")}</b>`
             : ""}
           ${r.faltando && r.faltando.length
             ? `<span class="leg">Ficam nulas: ${r.faltando.map(esc).join(", ")}</span>`
             : ""}</div>
         ${r.amostra && r.amostra.length
           ? tabela(cols.map(c => ({ t: c })), r.amostra,
               l => `<tr>${l.map(v => celulaValor(v)).join("")}</tr>`)
           : `<div class="vazio">nenhuma linha</div>`}
         <p class="leg">Amostra das primeiras ${r.amostra ? r.amostra.length : 0} linhas,
           como o leitor entendeu — <b>antes</b> de converter para o tipo de cada coluna.</p>`;
      $("#btImportar").disabled = !!(r.desconhecidas && r.desconhecidas.length) || !r.linhas_lidas;
    } catch (e) {
      $("#impSaida").innerHTML = `<div class="aviso mal">${esc(String(e))}</div>`;
      $("#btImportar").disabled = true;
    }
  };

  $("#btImportar").onclick = async () => {
    const texto = previa();
    if (texto === null) return;
    $("#impSaida").innerHTML = `<div class="centro">gravando…</div>`;
    try {
      const r = await api("inserir_lote", {
        database: db, tabela: tab, texto,
        formato: $("#impFmt").value,
        parar_no_erro: $("#impErro").value === "parar",
      });
      $("#impSaida").innerHTML =
        `<div class="aviso ${r.recusadas ? "mal" : ""}">
           <b>${fmt(r.gravadas)}</b> de ${fmt(r.recebidas)} linha(s) gravadas em
           ${r.ms} ms${r.por_segundo ? ` — ${fmt(r.por_segundo)} linhas/s` : ""}.
           ${r.gravadas ? `rowid ${r.primeiro_rowid} a ${r.ultimo_rowid}.` : ""}
           ${r.aviso ? `<br><b>${esc(r.aviso)}</b>` : ""}</div>
         ${(r.erros || []).length
           ? tabela([{t:"linha da carga",cls:"num"},{t:"por quê"}], r.erros,
               e => `<tr><td class="num">${e.linha}</td><td>${esc(e.erro)}</td></tr>`)
           : ""}`;
      if (r.gravadas) avisar(`${fmt(r.gravadas)} linha(s) gravadas em ${tab}`);
      $("#btImportar").disabled = true;
    } catch (e) {
      $("#impSaida").innerHTML = `<div class="aviso mal">${esc(String(e))}</div>`;
    }
  };
}

/* ================================================= Exclusão: as duas formas'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

# menu e gestao de tabelas
s=s.replace('''    { rot:"Exportar…",            ico:"⭳", quando:comTabela, faz:() => telaExportar() },''',
            '''    { rot:"Exportar…",            ico:"⭳", quando:comTabela, faz:() => telaExportar() },
    { rot:"Importar carga…",      ico:"⭱", quando:comTabela, faz:() => telaImportar() },''',1)
s=s.replace('''    ["lixeira", "♲", "Lixeira da tabela",''',
            '''    ["importar", "⭱", "Importar uma carga",
     "Cole CSV, TXT, JSON, XML ou HTML e grave de uma vez. Confere antes.",
     () => telaImportar(db, tab)],
    ["lixeira", "♲", "Lixeira da tabela",''',1)

s=s.replace('''.chip-visao button.ativo{background:var(--laranja);color:#10060a}''',
'''.chip-visao button.ativo{background:var(--laranja);color:#10060a}
textarea.campo.mono{font-family:"IBM Plex Mono",monospace;font-size:12px;line-height:1.5}
label.cmp.largo{grid-column:1/-1}''',1)
io.open(p,'w',encoding='utf-8').write(s)
