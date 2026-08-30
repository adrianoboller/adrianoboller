# Wire the new login fields in JavaScript
# 27/08 20:52

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

# 1. detectarModo usa o /saude para preencher a porta real e decidir a chave
velho='''    // /saude nao pede token e nao conta tentativa: e so o sinal de vida.
    const r = await fetch("/saude");
    if (!r.ok) throw new Error("sem servidor");
    await r.json();
    est.demo = false;'''
novo='''    // /saude nao pede token e nao conta tentativa: e so o sinal de vida.
    const r = await fetch("/saude");
    if (!r.ok) throw new Error("sem servidor");
    const saude = await r.json();
    est.demo = false;
    // A porta que aparece no formulario e a que o servidor REALMENTE escuta,
    // lida dele. O 5000 e so o padrao de fabrica.
    est.portaLocal = saude.porta_dados || 5000;
    est.servidores = saude.servidores || [];
    $("#pt").value = est.portaLocal;
    $("#campoChave").hidden = !saude.exige_chave;
    if (est.servidores.length) {
      $("#h").setAttribute("list", "servidores");
      const dl = document.createElement("datalist");
      dl.id = "servidores";
      for (const d of est.servidores) {
        const o = document.createElement("option");
        o.value = d.split(":")[0];
        dl.appendChild(o);
      }
      document.body.appendChild(dl);
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# 2. estado ganha os campos novos
s=s.replace('const est = { sessao:null, usuario:null, token:"", demo:false,\n              bancos:[], atual:null, aba:"estrutura", ordem:"", linhas:[] };',
'''const est = { sessao:null, usuario:null, token:"", demo:false,
              bancos:[], atual:null, aba:"estrutura", ordem:"", linhas:[],
              // Para onde este console esta falando. Vazio = o proprio
              // servidor que serviu a pagina.
              servidor:"", portaLocal:5000, servidores:[], database:"" };''')

# 3. entrar(): monta o destino, assina se houver chave, lembra o database
velho2='''async function entrar() {
  const usuario = $("#u").value.trim(), senha = $("#s").value, token = $("#t").value;
  const recado = $("#recado");
  recado.className = "recado info"; recado.textContent = "Conferindo…";
  est.token = token;'''
novo2='''/// Este endereco e o proprio servidor que serviu a pagina?
function ehLocal(host, porta) {
  const h = (host || "").trim().toLowerCase();
  const aqui = h === "" || h === "localhost" || h === "127.0.0.1" || h === "::1"
            || h === location.hostname.toLowerCase();
  return aqui && Number(porta) === Number(est.portaLocal);
}

async function entrar() {
  const usuario = $("#u").value.trim(), senha = $("#s").value, token = $("#t").value;
  const host = $("#h").value.trim(), porta = $("#pt").value.trim() || "5000";
  const chave = $("#k").value.trim();
  const recado = $("#recado");
  recado.className = "recado info"; recado.textContent = "Conferindo…";
  est.token = token;
  est.database = $("#db").value.trim();
  // So manda "servidor" quando for OUTRO servidor. Para o proprio, o caminho
  // curto continua sendo o caminho curto.
  est.servidor = ehLocal(host, porta) ? "" : `${host}:${porta}`;'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# 4. api() leva o servidor no login; e a assinatura entra no desafio-resposta
s=s.replace('''      est.usuario = await api("login", { usuario, prova, nonce_cliente:nonceCliente });''',
'''      const pedido = { usuario, prova, nonce_cliente:nonceCliente };
      if (est.servidor) pedido.servidor = est.servidor;
      if (chave) {
        recado.textContent = "Assinando o desafio…";
        pedido.assinatura = await assinarDesafio(chave, d.nonce, nonceCliente, usuario);
      }
      est.usuario = await api("login", pedido);''')
s=s.replace('''      est.usuario = await api("login",
        { usuario_b64:b64(usuario), senha_b64:b64(senha) });''',
'''      const pedido = { usuario_b64:b64(usuario), senha_b64:b64(senha) };
      if (est.servidor) pedido.servidor = est.servidor;
      est.usuario = await api("login", pedido);''')

# 5. o desafio tambem precisa saber para qual servidor vai
s=s.replace('''      const d = await api("desafio", { usuario });''',
'''      const d = await api("desafio",
        est.servidor ? { usuario, servidor: est.servidor } : { usuario });''')

# 6. a funcao que assina
s=s.replace('''async function calcularProva(senha, salHex, iteracoes, nonce, nonceCliente, usuario) {''',
'''/// Assina o MESMO desafio da senha, com a chave privada Ed25519.
///
/// Segundo fator: a senha prova que voce sabe, a chave prova que voce tem.
/// Depende de crypto.subtle com Ed25519 -- que e recente. Sem ele, a pagina
/// diz para usar a linha de comando em vez de fingir que assinou.
async function assinarDesafio(chaveHex, nonce, nonceCliente, usuario) {
  if (!/^[0-9a-fA-F]{64}$/.test(chaveHex))
    throw new Error("a chave privada tem 64 hexadecimais");
  let chave;
  try {
    chave = await crypto.subtle.importKey(
      "raw", bytesDeHex(chaveHex), { name:"Ed25519" }, false, ["sign"]);
  } catch {
    throw new Error("este navegador não assina Ed25519; entre sem chave por "
                  + "um cliente de linha de comando");
  }
  const msg = new TextEncoder().encode(`${nonce},${nonceCliente},${usuario}`);
  return hexDeBytes(await crypto.subtle.sign("Ed25519", chave, msg));
}

async function calcularProva(senha, salHex, iteracoes, nonce, nonceCliente, usuario) {''')

# 7. abrirApp abre direto no database pedido
s=s.replace('''  await montarArvore();''','''  await montarArvore();
  // O campo "Database" do login abre a arvore ja nesse banco.
  if (est.database) {
    const no = [...document.querySelectorAll(".no.banco")]
      .find(n => n.textContent.trim() === est.database);
    if (no) no.click();
  }''')
open(p,'w').write(s)
print('js ok')
