# Add the export screen and toolbar button
# 28/08 16:59

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
novo = '''
/* ================================================== Exportar uma tabela
   Sete formatos, e os dois de escritório saem do mesmo compactador que o
   backup já usava: um .xlsx e um .docx são ZIP de XML.

   O download acontece no navegador, a partir de um Blob: o servidor devolve o
   arquivo (texto ou base64) e a página o entrega. Assim o mesmo caminho serve
   para quem clica e para quem chama por `curl`. */

const FORMATOS = [
  { id:"xlsx", rot:"Excel",    ico:"▦", diz:"planilha formatada, com filtro e painel congelado" },
  { id:"csv",  rot:"CSV",      ico:"☰", diz:"ponto e vírgula, decimal com vírgula, BOM para o acento" },
  { id:"json", rot:"JSON",     ico:"{}", diz:"uma lista de objetos, tipos preservados" },
  { id:"xml",  rot:"XML",      ico:"◇", diz:"um elemento por linha, nulo declarado" },
  { id:"html", rot:"HTML",     ico:"◫", diz:"tabela pronta, com filtro que funciona sem rede" },
  { id:"docx", rot:"Word",     ico:"▤", diz:"documento em paisagem, cabeçalho repetido por página" },
  { id:"txt",  rot:"Texto",    ico:"⌷", diz:"largura fixa, alinhado para ler no terminal" },
];

/// Entrega o arquivo ao navegador.
///
/// `base64` para binário, texto direto para o resto — decodificar base64 de um
/// CSV de 40 MB seria pagar um terço a mais de transporte e uma passada a mais
/// de CPU por nada.
function baixar(nome, mime, conteudo, ehBase64) {
  let corpo;
  if (ehBase64) {
    const bin = atob(conteudo);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    corpo = bytes;
  } else {
    corpo = conteudo;
  }
  const url = URL.createObjectURL(new Blob([corpo], { type: mime }));
  const a = document.createElement("a");
  a.href = url; a.download = nome;
  document.body.appendChild(a); a.click(); a.remove();
  // Revogar na hora cancelaria o download em alguns navegadores.
  setTimeout(() => URL.revokeObjectURL(url), 30000);
}

async function telaExportar(db, tab) {
  db = db || (est.atual || {}).db || databaseCorrente();
  tab = tab || (est.atual || {}).tab;
  if (!db || !tab) return avisar("escolha uma tabela primeiro", true);

  folha(`Exportar ${tab}`, `${esc(db)} · escolha o formato`,
    `<div class="formatos" id="fmts">
       ${FORMATOS.map(f => `<button class="fmt" data-f="${f.id}">
            <span class="f-ico">${f.ico}</span>
            <span class="f-rot">${f.rot}</span>
            <span class="f-ext">.${f.id}</span>
            <span class="f-diz">${f.diz}</span>
          </button>`).join("")}
     </div>
     <div class="form-dbl">
       <label class="cmp"><span>Máximo de linhas</span>
         <input id="expMax" class="campo" type="number" value="100000" min="1">
         <span class="leg">a tabela inteira, se couber — o teto existe para não
           montar um arquivo de gigabytes por engano</span></label>
     </div>
     <div id="expSaida"></div>
     <div class="acoes">
       <button class="botao secundario" id="btVoltaExp">← Gerir tabelas</button>
     </div>`);

  $("#btVoltaExp").onclick = () => gerirTabelasAtual();
  $$("#fmts .fmt").forEach(b => b.onclick = async () => {
    const f = b.dataset.f;
    $("#expSaida").innerHTML = `<div class="centro">gerando ${esc(f)}…</div>`;
    let r;
    try {
      r = await api("exportar", {
        database: db, tabela: tab, formato: f,
        max: Number($("#expMax").value) || 100000,
      });
    } catch (e) { return $("#expSaida").innerHTML = `<div class="aviso mal">${esc(String(e))}</div>`; }
    baixar(r.arquivo, r.mime, r.binario ? r.base64 : r.conteudo, r.binario);
    $("#expSaida").innerHTML =
      `<div class="aviso"><b>${esc(r.arquivo)}</b> — ${fmt(r.linhas)} linha(s),
        ${fmtBytes(r.bytes)}, ${r.ms} ms.${
        r.truncado ? ` <b>Cortado no teto</b>: há mais linhas do que o máximo pedido.` : ""}</div>`;
  });
}
'''
marca='''/* ============================================ Sessões e estatísticas de uso'''
assert marca in s
s=s.replace(marca, novo.strip()+"\n\n"+marca,1)

a='''.botao.mini{width:auto'''
b='''.formatos{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));
          gap:10px;margin:6px 0 18px}
.fmt{display:flex;flex-direction:column;align-items:flex-start;gap:3px;
     padding:12px 14px;background:var(--painel-2);border:1px solid var(--linha);
     border-radius:9px;color:var(--texto-2);cursor:pointer;font:inherit;
     width:auto;text-align:left}
.fmt:hover{border-color:var(--laranja);color:var(--texto)}
.fmt .f-ico{font-size:17px;line-height:1;color:var(--laranja)}
.fmt .f-rot{font-size:13px;font-weight:600}
.fmt .f-ext{font-size:10.5px;font-family:"IBM Plex Mono",monospace;color:var(--texto-3)}
.fmt .f-diz{font-size:10.5px;color:var(--texto-3);line-height:1.35}

.botao.mini{width:auto'''
assert a in s; s=s.replace(a,b,1)

a='''    { rot:"Soma de verificação",  ico:"⑈", quando:comTabela, faz:() => checksumTabela() },'''
b='''    { rot:"Soma de verificação",  ico:"⑈", quando:comTabela, faz:() => checksumTabela() },
    { rot:"Exportar…",            ico:"⭳", quando:comTabela, faz:() => telaExportar() },'''
assert a in s; s=s.replace(a,b,1)

alvo=[l for l in s.split("\n") if 'rot:"Importar"' in l and 'ico:"importar"' in l]
assert len(alvo)==1, alvo
s=s.replace(alvo[0], '  { ico:"exportar", rot:"Exportar",   cor:"var(--ok)",     faz:() => telaExportar() },\n'+alvo[0],1)

a='''  // Dois circulos que se cruzam, com o meio cheio'''
b='''  // Seta para baixo saindo de uma folha: o dado saindo da tabela para o disco
  // de quem pediu. E o espelho do `importar`, de proposito.
  exportar: `<path d="M12 3v11M8 10.5l4 4 4-4" fill="none" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" transform="rotate(180 12 12)"/><path d="M4 17.5v2a1.5 1.5 0 001.5 1.5h13a1.5 1.5 0 001.5-1.5v-2" fill="none" stroke-width="1.6"/>`,
  // Dois circulos que se cruzam, com o meio cheio'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
