# Add the alphanumeric option to the create-table screen
# 28/08 18:55

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
velho='''const PARTICOES = [
  ["quantidade", "por faixa de quantidade", "volume novo a cada N registros — o endereço sai de divisão"],
  ["mensal",     "mensal",                  "volume novo quando o mês vira"],
  ["bimestral",  "bimestral",               "jan-fev, mar-abr, mai-jun, jul-ago, set-out, nov-dez"],
  ["semestral",  "semestral",               "jan-jun e jul-dez"],
  ["anual",      "anual",                   "volume novo a cada ano"],
];

/** Os tipos que podem carregar a data da particao. */
const TIPOS_DATA = ["Date", "DateTime"];'''
novo='''const PARTICOES = [
  ["quantidade", "por faixa de quantidade", "volume novo a cada N registros — o endereço sai de divisão"],
  ["letra",      "alfanumérica (A–Z, 0–9)", "um arquivo por letra inicial: Clientes_A.reg … Clientes_Outros.reg"],
  ["mensal",     "mensal",                  "volume novo quando o mês vira"],
  ["bimestral",  "bimestral",               "jan-fev, mar-abr, mai-jun, jul-ago, set-out, nov-dez"],
  ["semestral",  "semestral",               "jan-jun e jul-dez"],
  ["anual",      "anual",                   "volume novo a cada ano"],
];

/** Os tipos que podem carregar a data da particao. */
const TIPOS_DATA = ["Date", "DateTime"];

/** Os tipos que servem de REFERENCIA na particao alfanumerica.
 *
 * `Bin` e `Memo` ficam de fora, e nao por gosto: o valor deles mora fora do
 * slot, e o balde precisa ser decidido ANTES de a linha ser gravada -- ler o
 * `.memo` para saber em que arquivo gravar seria a ordem invertida. */
const TIPOS_REFERENCIA = t => !/^(Bin|Memo)$/.test(String(t || ""));'''
assert velho in s
s=s.replace(velho,novo,1)

# o formulario: quando for por letra, oferece a coluna de referencia
velho2='''         ${porPeriodo ? `
         <label class="largo"><span>coluna de data que decide o período'''
novo2='''         ${porLetra ? `
         <label class="largo"><span>coluna de referência
             <em>(a primeira letra dela decide o arquivo; precisa ser obrigatória)</em></span>
           <select id="nt_pcol">${
             colunasRef.length
               ? colunasRef.map(c => `<option value="${esc(c.nome)}"${
                   c.nome === r.particao_coluna ? " selected" : ""}>${esc(c.nome)}${
                   c.obrigatoria ? "" : " — atenção: aceita nulo"}</option>`).join("")
               : `<option value="">— nenhuma coluna serve de referência —</option>`
           }</select></label>` : ""}

         ${porPeriodo ? `
         <label class="largo"><span>coluna de data que decide o período'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

velho3='''  const porPeriodo = r.particao && r.particao !== "quantidade";
  const colunasData = r.colunas.filter(c => TIPOS_DATA.includes(c.tipo) && c.nome);'''
novo3='''  const porLetra = r.particao === "letra";
  const porPeriodo = r.particao && r.particao !== "quantidade" && !porLetra;
  const colunasData = r.colunas.filter(c => TIPOS_DATA.includes(c.tipo) && c.nome);
  const colunasRef = r.colunas.filter(c => c.nome && TIPOS_REFERENCIA(c.tipo));'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

# o teto muda de sentido
velho4='''         <label><span>${porPeriodo ? "teto de registros por volume" : "registros por arquivo"}
             <em>${porPeriodo ? "corta antes se o período render demais" : "0 = arquivo único"}</em></span>'''
novo4='''         <label><span>${porLetra ? "teto de registros POR LETRA"
                        : porPeriodo ? "teto de registros por volume" : "registros por arquivo"}
             <em>${porLetra ? "num cadastro brasileiro o _S enche muito antes do _K"
                   : porPeriodo ? "corta antes se o período render demais" : "0 = arquivo único"}</em></span>'''
assert velho4 in s
s=s.replace(velho4,novo4,1)

# os campos de digito e teto nao valem na alfanumerica
velho5='''         <label><span>dígitos do sufixo <em>(3 dá <code>_001</code>)</em></span>
           <input id="nt_dig" inputmode="numeric" placeholder="3" value="${esc(String(r.dig ?? 3))}"></label>
         <label><span>teto de volumes <em>(0 = o que couber no sufixo)</em></span>
           <input id="nt_max" inputmode="numeric" placeholder="${esc(String(tetoDoSufixo(r.dig ?? 3)))}"
                  value="${esc(String(r.max ?? 0))}"></label>
       </div>'''
novo5='''         ${porLetra ? `<p class="leg">O sufixo é a <b>letra</b>, e os volumes são
           <b>37</b> — A a Z, 0 a 9 e <code>Outros</code>. Nenhum dos dois se
           escolhe: são o formato desta partição.</p>` : `
         <label><span>dígitos do sufixo <em>(3 dá <code>_001</code>)</em></span>
           <input id="nt_dig" inputmode="numeric" placeholder="3" value="${esc(String(r.dig ?? 3))}"></label>
         <label><span>teto de volumes <em>(0 = o que couber no sufixo)</em></span>
           <input id="nt_max" inputmode="numeric" placeholder="${esc(String(tetoDoSufixo(r.dig ?? 3)))}"
                  value="${esc(String(r.max ?? 0))}"></label>`}
       </div>

       ${porLetra ? `<div class="nota"><p><strong>A ordem de digitação muda de
         campo.</strong> A linha vai para o arquivo da letra dela, então duas
         linhas digitadas em seguida caem em arquivos diferentes e o
         <code>rowid</code> deixa de crescer com a chegada — ele passa a dizer
         em que <em>arquivo</em> a linha está. A ordem de chegada continua
         inteira na coluna de sistema <code>rownum</code>, e é por ela que a
         grade pagina.</p>
         <p>Alterar a coluna de referência depois é <strong>recusado</strong>:
         mudaria o arquivo em que a linha mora, e com ele o rowid, que é a
         identidade dela em todo índice. Para mudar, exclua e insira de novo.</p>
         </div>` : ""}'''
assert velho5 in s
s=s.replace(velho5,novo5,1)
io.open(p,'w',encoding='utf-8').write(s)
