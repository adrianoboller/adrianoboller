# Add monitor clock
# 28/08 14:27

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''async function abrirAdmin(qual) {
  est.atual = null;
  $("#abas").innerHTML = "";
  const p = $("#painel");
  p.innerHTML = `<div class="centro">carregando…</div>`;
  try {
    if (qual === "painel") {
      $("#titulo").textContent = "Painel";
      $("#subtitulo").textContent =
        `${est.painel ? "" : ""}o servidor inteiro numa tela · uma única chamada ao servidor`;
      p.innerHTML = await vPainel();
      return;
    }'''
b='''async function abrirAdmin(qual) {
  est.atual = null;
  $("#abas").innerHTML = "";
  pararMonitor();
  const p = $("#painel");
  p.innerHTML = `<div class="centro">carregando…</div>`;
  try {
    if (qual === "painel") {
      $("#titulo").textContent = "Painel";
      $("#subtitulo").textContent =
        "o servidor inteiro numa tela · os monitores da máquina renovam sozinhos";
      p.innerHTML = await vPainel();
      ligarMonitor();
      return;
    }'''
assert a in s; s=s.replace(a,b,1)

# as duas funcoes do relogio, logo depois de lerMaquina
a='''async function lerMaquina() {
  try { return await api("sistema"); } catch { return null; }
}'''
b='''async function lerMaquina() {
  try { return await api("sistema"); } catch { return null; }
}

/// Renova só os monitores da máquina, de tempos em tempos.
///
/// Só eles: repintar o painel inteiro faria o servidor varrer todos os bancos
/// a cada quatro segundos para atualizar um número de CPU. E o intervalo é o
/// que define a taxa — a leitura de agora é comparada com a de quatro segundos
/// atrás, então uma janela curta demais mostraria ruído e uma longa demais
/// esconderia o pico.
function ligarMonitor(segundos = 4) {
  pararMonitor();
  if (!est.maquina) return;   // sem permissão ou sem /proc: nada a renovar
  est.relogioMaquina = setInterval(async () => {
    const alvo = $("#maquina");
    // A tela pode ter mudado entre dois tiques; sem o alvo, o relógio para
    // sozinho em vez de escrever num elemento que já saiu da página.
    if (!alvo) return pararMonitor();
    const m = await lerMaquina();
    if (!m) return pararMonitor();
    est.maquina = m;
    if ($("#maquina")) $("#maquina").innerHTML = maquinaHtml(m);
  }, segundos * 1000);
}

function pararMonitor() {
  if (est.relogioMaquina) clearInterval(est.relogioMaquina);
  est.relogioMaquina = null;
}'''
assert a in s; s=s.replace(a,b,1)

# est ganha o campo
a='''              teto:200, esquemaAtual:null, grade:null, painel:null,'''
b='''              teto:200, esquemaAtual:null, grade:null, painel:null,
              maquina:null, relogioMaquina:null,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
