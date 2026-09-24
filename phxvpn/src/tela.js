"use strict";
const $ = (id) => document.getElementById(id);
let sessao = null;
try { sessao = JSON.parse(sessionStorage.getItem("phxvpn") || "null"); } catch (_) {}

async function api(metodo, rota, corpo) {
  const cab = { "Content-Type": "application/json" };
  if (sessao) cab.Authorization = "Bearer " + sessao.token;
  const r = await fetch(rota, { method: metodo, headers: cab, body: corpo ? JSON.stringify(corpo) : undefined });
  const j = await r.json().catch(() => ({ erro: "resposta ilegível do painel" }));
  if (r.status === 401 && sessao && rota !== "/api/login") { sair(); }
  if (!r.ok) throw new Error(j.erro || ("erro " + r.status));
  return j;
}
function msg(id, texto, ok) { const m = $(id); m.textContent = texto || ""; m.className = "msg " + (ok ? "ok" : "erro"); }
function dados(form) { return Object.fromEntries(new FormData(form).entries()); }
function mostrar(tela) { for (const t of ["tela-instalar","tela-login","tela-redes","tela-admin","tela-mfa","tela-senha","tela-historico"]) $(t).hidden = t !== tela; $("barra").hidden = !sessao; }

// Confirmacao DENTRO da pagina -- nada de confirm() do navegador (acao
// vermelha precisa de um passo a mais, nao de um dialogo do sistema que o
// usuario aprende a clicar sem ler).
function confirmar(titulo, texto, rotuloOk) {
  return new Promise((resolve) => {
    $("dc-titulo").textContent = titulo;
    $("dc-texto").textContent = texto;
    $("dc-ok").textContent = rotuloOk || "Confirmar";
    msg("m-confirmar", "");
    const d = $("d-confirmar");
    const limpar = () => { d.removeEventListener("close", aoFechar); $("f-confirmar").onsubmit = null; $("dc-cancelar").onclick = null; };
    const aoFechar = () => { limpar(); resolve(d.returnValue === "confirmar"); };
    $("f-confirmar").onsubmit = (ev) => { ev.preventDefault(); d.close("confirmar"); };
    $("dc-cancelar").onclick = () => d.close("cancelar");
    d.addEventListener("close", aoFechar);
    d.showModal();
  });
}
function baixar(nome, texto) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([texto], { type: "application/x-openvpn-profile" }));
  a.download = nome; document.body.appendChild(a); a.click(); a.remove();
}
function sair() { if (sessao) api("POST", "/api/sair").catch(() => {}); sessao = null; try { sessionStorage.removeItem("phxvpn"); } catch (_) {} iniciar(); }

async function iniciar() {
  let e;
  try { e = await api("GET", "/api/estado"); } catch (x) { document.body.textContent = "Painel indisponível: " + x.message; return; }
  $("empresa-nome").textContent = e.empresa ? e.empresa.nome : "";
  if (!e.instalado) return mostrar("tela-instalar");
  if (!sessao) return mostrar("tela-login");
  $("quem").textContent = sessao.login + (sessao.admin ? " · admin" : "");
  $("b-admin").hidden = !sessao.admin;
  $("selo-cofre").hidden = e.destrancado;
  $("cartao-cofre").hidden = e.destrancado || !sessao.admin;
  mostrar("tela-redes");
  carregarRedes();
}

async function carregarRedes() {
  const caixa = $("lista-redes"); caixa.textContent = "";
  let redes;
  try { redes = await api("GET", "/api/redes"); } catch (x) { return msg("m-redes", x.message); }
  if (!redes.length) { const v = document.createElement("div"); v.className = "vazio"; v.textContent = "Nenhuma rede ainda. Crie uma ou entre numa existente."; caixa.appendChild(v); return; }
  for (const r of redes) {
    const bloco = document.createElement("div"); bloco.className = "rede";
    const topo = document.createElement("div"); topo.className = "topo";
    const nome = document.createElement("span"); nome.className = "nome"; nome.textContent = r.nome;
    const info = document.createElement("span"); info.className = "info";
    info.textContent = `${r.subrede} · porta ${r.porta} · ${r.membros} membro(s) · servidor ${r.servidor}` + (r.meu_ip ? ` · meu IP ${r.meu_ip}` : "") + (r.finalidade ? ` · ${r.finalidade}` : "") + (r.exige_mfa ? " · exige autenticador" : "");
    const acoes = document.createElement("span"); acoes.className = "acoes";
    if (sessao.admin || r.dono === sessao.login) {
      const b = document.createElement("button"); b.className = "altera";
      b.textContent = r.exige_mfa ? "Dispensar autenticador" : "Exigir autenticador";
      b.onclick = async () => {
        const frase = r.exige_mfa ? `Dispensar o autenticador na rede «${r.nome}»?` : `Exigir usuário, senha e código do autenticador para conectar na rede «${r.nome}»? Os membros precisam baixar o perfil de novo.`;
        if (!confirm(frase)) return;
        // Quem tem autenticador prova o código para mudar a exigência.
        let codigo = "";
        if (sessao.mfa) { codigo = prompt("Código do autenticador") || ""; if (!codigo) return; }
        try { const x = await api("POST", "/api/redes/mfa", { rede_id: r.id, exige: !r.exige_mfa, codigo }); msg("m-redes", x.aviso, true); carregarRedes(); } catch (x) { msg("m-redes", x.message); }
      };
      acoes.appendChild(b);
    }
    if (r.meu_ip) {
      const b = document.createElement("button"); b.className = "exclui"; b.textContent = "Sair da rede";
      b.onclick = async () => {
        if (!await confirmar("Sair da rede", `Sair da rede «${r.nome}»?`, "Sair")) return;
        try { await api("POST", "/api/redes/sair", { rede_id: r.id }); carregarRedes(); } catch (x) { msg("m-redes", x.message); }
      };
      acoes.appendChild(b);
    }
    topo.append(nome, info, acoes); bloco.appendChild(topo);
    // So admin ou o DONO da rede remove membro -- o mesmo direito que ja
    // vale para exigir/dispensar autenticador (`sessao.admin || r.dono === sessao.login`).
    const podeRemover = sessao.admin || r.dono === sessao.login;
    try {
      const membros = await api("POST", "/api/redes/membros", { rede_id: r.id });
      for (const m of membros) {
        const l = document.createElement("div"); l.className = "membro";
        const p = document.createElement("span"); p.className = "ponto" + (m.online ? " on" : ""); p.title = m.online ? "conectado" : "desconectado";
        const n = document.createElement("span"); n.textContent = m.login;
        const ip = document.createElement("span"); ip.className = "ip"; ip.textContent = m.ip;
        l.append(p, n, ip);
        if (podeRemover && m.login !== r.dono) {
          const br = document.createElement("button"); br.className = "exclui"; br.textContent = "Remover";
          br.onclick = async () => {
            if (!await confirmar("Remover membro", `Remover «${m.login}» da rede «${r.nome}»? O acesso dele é revogado na hora.`, "Remover")) return;
            try { await api("POST", "/api/redes/remover", { rede_id: r.id, login: m.login }); msg("m-redes", `${m.login} removido`, true); carregarRedes(); } catch (x) { msg("m-redes", x.message); }
          };
          l.appendChild(br);
        }
        bloco.appendChild(l);
      }
      await blocoRotas(bloco, r, membros);
      await blocoSaida(bloco, r);
      await blocoAlcance(bloco, r);
    } catch (_) {}
    caixa.appendChild(bloco);
  }
}

// Redes alcancaveis pela VPN: a LAN atras do servidor e a filial atras de um
// membro. Incluir e so do admin (abre a LAN da empresa); remover, admin ou dono.
const ONDE = { nat: "atrás do servidor · NAT", rota: "atrás do servidor · rota de volta no roteador", filial: "atrás de" };
async function blocoRotas(bloco, r, membros) {
  let rotas;
  try { rotas = await api("POST", "/api/redes/rotas", { rede_id: r.id }); } catch (_) { return; }
  const pode = sessao.admin || r.dono === sessao.login;
  if (!rotas.length && !sessao.admin) return;
  const caixa = document.createElement("div"); caixa.className = "rotas";
  const t = document.createElement("div"); t.className = "titulo"; t.textContent = "Redes alcançáveis pela VPN"; caixa.appendChild(t);
  const aviso = document.createElement("p"); aviso.className = "msg"; aviso.id = "m-rota-" + r.id;
  for (const x of rotas) {
    const l = document.createElement("div"); l.className = "rota";
    const c = document.createElement("span"); c.className = "cidr"; c.textContent = x.cidr;
    const o = document.createElement("span"); o.className = "onde"; o.textContent = x.volta === "filial" ? `${ONDE.filial} ${x.membro}` : ONDE[x.volta];
    l.append(c, o);
    if (pode) {
      const b = document.createElement("button"); b.className = "exclui"; b.textContent = "Remover";
      b.onclick = async () => {
        if (!await confirmar("Remover rota", `Remover ${x.cidr} da rede «${r.nome}»? O OpenVPN da rede reinicia e os membros reconectam.`, "Remover")) return;
        try { await api("POST", "/api/redes/rotas/remover", { rede_id: r.id, cidr: x.cidr }); carregarRedes(); } catch (e) { msg("m-redes", e.message); }
      };
      l.appendChild(b);
    }
    caixa.appendChild(l);
  }
  if (sessao.admin) {
    const f = document.createElement("form"); f.className = "nova-rota";
    const cidr = document.createElement("input"); cidr.placeholder = "192.168.10.0/24"; cidr.required = true; cidr.setAttribute("aria-label", "Rede (CIDR)");
    const onde = document.createElement("select"); onde.setAttribute("aria-label", "Onde fica a rede");
    const opcao = (v, texto) => { const op = document.createElement("option"); op.value = v; op.textContent = texto; onde.appendChild(op); };
    opcao("nat", ONDE.nat); opcao("rota", ONDE.rota);
    for (const m of membros) opcao("m:" + m.login, `${ONDE.filial} ${m.login}`);
    const b = document.createElement("button"); b.className = "inclui"; b.type = "submit"; b.textContent = "Incluir rota";
    f.append(cidr, onde, b);
    f.onsubmit = async (ev) => {
      ev.preventDefault();
      const v = onde.value, corpo = { rede_id: r.id, cidr: cidr.value.trim() };
      if (v.startsWith("m:")) corpo.membro = v.slice(2); else corpo.volta = v;
      if (sessao.mfa) { corpo.codigo = prompt("Código do autenticador") || ""; if (!corpo.codigo) return; }
      try {
        const x = await api("POST", "/api/redes/rotas/incluir", corpo);
        const fw = x.firewall || {};
        let texto = `${corpo.cidr} incluída` + (x.openvpn_reiniciado ? "; os membros recebem a rota ao reconectar" : "");
        if (corpo.volta === "rota") texto += `; no roteador da LAN: rota ${r.subrede} via o IP deste servidor`;
        if (fw.aviso) texto += ` — atenção: ${fw.aviso}`;
        await carregarRedes(); msg("m-redes", texto, !fw.aviso);
      } catch (e) { msg(aviso.id, e.message); }
    };
    caixa.append(f, aviso);
  }
  bloco.appendChild(caixa);
}

// Saida da rede: tunel total (a internet do membro pelo servidor) e o DNS
// empurrado. Ligar e so do admin; o dono ve e pode desligar.
async function blocoSaida(bloco, r) {
  let s;
  try { s = await api("POST", "/api/redes/saida", { rede_id: r.id }); } catch (_) { return; }
  const pode = sessao.admin || r.dono === sessao.login;
  const ligado = s.tunel_total || s.dns_nomes || s.dns_empresa;
  if (!ligado && !sessao.admin) return;
  const caixa = document.createElement("div"); caixa.className = "saida";
  const t = document.createElement("div"); t.className = "titulo"; t.textContent = "Saída de internet e DNS"; caixa.appendChild(t);
  const estado = document.createElement("div"); estado.className = "estado";
  const parte = (rotulo, valor, mono) => {
    const l = document.createElement("div");
    const b = document.createElement("b"); b.textContent = rotulo;
    const v = document.createElement("span"); if (mono) v.className = "zona"; v.textContent = valor;
    l.append(b, " ", v); estado.appendChild(l);
  };
  parte("Túnel total:", s.tunel_total ? (s.bloquear_local ? "ligado, sem acesso à LAN local" : "ligado") : "desligado (só a VPN passa pelo servidor)");
  if (s.dns_nomes) parte("Nomes:", `membro.${s.zona}`, true);
  if (s.dns_empurrado) parte("DNS empurrado:", s.dns_empurrado, true);
  caixa.appendChild(estado);
  if (pode) {
    const f = document.createElement("form");
    const marca = (nome, texto, v) => {
      const l = document.createElement("label"); l.className = "marca";
      const i = document.createElement("input"); i.type = "checkbox"; i.name = nome; i.checked = v;
      l.append(i, " " + texto); f.appendChild(l); return i;
    };
    const tt = marca("tunel_total", "Túnel total — toda a internet do membro sai pelo servidor (com bloqueio de DNS e IPv6 por fora)", s.tunel_total);
    const bl = marca("bloquear_local", "Bloquear a LAN local do membro enquanto conectado", s.bloquear_local);
    const nomes = marca("dns_nomes", `Nomes dos membros (membro.${s.zona}) pelo DNS do servidor`, s.dns_nomes);
    const linha = document.createElement("div"); linha.className = "dns";
    const dns = document.createElement("input"); dns.value = s.dns_empresa; dns.placeholder = "DNS da empresa (opcional): 192.168.10.53"; dns.setAttribute("aria-label", "DNS da empresa");
    const b = document.createElement("button"); b.className = "altera"; b.type = "submit"; b.textContent = "Salvar saída";
    linha.append(dns, b); f.appendChild(linha);
    const aviso = document.createElement("p"); aviso.className = "msg"; aviso.id = "m-saida-" + r.id;
    const ajustar = () => { bl.disabled = !tt.checked; if (!tt.checked) bl.checked = false; };
    tt.onchange = ajustar; ajustar();
    f.onsubmit = async (ev) => {
      ev.preventDefault();
      const corpo = { rede_id: r.id, tunel_total: tt.checked, bloquear_local: bl.checked, dns_nomes: nomes.checked, dns_empresa: dns.value.trim() };
      if (sessao.admin && sessao.mfa) { corpo.codigo = prompt("Código do autenticador") || ""; if (!corpo.codigo) return; }
      try {
        const x = await api("POST", "/api/redes/saida/definir", corpo);
        const fw = x.firewall || {};
        await carregarRedes(); msg("m-redes", (fw.aviso ? `atenção: ${fw.aviso} — ` : "") + x.aviso, !fw.aviso);
      } catch (e) { msg(aviso.id, e.message); }
    };
    caixa.append(f, aviso);
  }
  bloco.appendChild(caixa);
}

// Alcance do servidor: enderecos alternativos, queda UDP->TCP e port-share.
// Ver e do admin e do dono; mudar, so do admin (abre porta no host).
async function blocoAlcance(bloco, r) {
  if (!sessao.admin && r.dono !== sessao.login) return;
  let a;
  try { a = await api("POST", "/api/redes/alcance", { rede_id: r.id }); } catch (_) { return; }
  const caixa = document.createElement("div"); caixa.className = "alcance";
  const t = document.createElement("div"); t.className = "titulo"; t.textContent = "Alcance do servidor"; caixa.appendChild(t);
  const partes = [];
  if (a.remotos.length) partes.push(`alternativos: ${a.remotos.join(", ")}` + (a.aleatorio ? " (ordem sorteada)" : ""));
  if (a.queda_tcp) partes.push(`queda UDP→TCP na porta ${a.queda_tcp}`);
  if (a.port_share) partes.push(`porta TCP dividida com ${a.port_share}`);
  const resumo = document.createElement("div"); resumo.className = "resumo"; resumo.textContent = partes.length ? partes.join(" · ") : "só o endereço do servidor"; caixa.appendChild(resumo);
  if (sessao.admin) {
    const f = document.createElement("form");
    const campo = (rotulo, el, largo) => { const l = document.createElement("label"); l.className = "campo" + (largo ? " largo" : ""); const s = document.createElement("span"); s.textContent = rotulo; l.append(s, el); f.appendChild(l); return el; };
    const rem = document.createElement("textarea"); rem.value = a.remotos.join("\n"); rem.placeholder = "vpn2.empresa.com.br\n200.1.2.3:1196";
    campo("Endereços alternativos (um por linha, HOST ou HOST:PORTA)", rem, true);
    const queda = document.createElement("input"); queda.type = "number"; queda.min = 1; queda.max = 65535; queda.placeholder = "443"; queda.value = a.queda_tcp || "";
    const ps = document.createElement("input"); ps.placeholder = "127.0.0.1:8443"; ps.value = a.port_share || ""; ps.autocomplete = "off";
    if (a.protocolo === "udp") campo("Queda para TCP (porta; vazio desliga)", queda);
    campo("Dividir a porta TCP com HTTPS em (HOST:PORTA)", ps);
    const sorteio = document.createElement("label"); sorteio.className = "marca largo";
    const cb = document.createElement("input"); cb.type = "checkbox"; cb.checked = a.aleatorio;
    sorteio.append(cb, " Sortear a ordem dos endereços (remote-random)"); f.appendChild(sorteio);
    const b = document.createElement("button"); b.className = "altera largo"; b.type = "submit"; b.textContent = "Gravar alcance"; f.appendChild(b);
    const aviso = document.createElement("p"); aviso.className = "msg largo"; aviso.id = "m-alcance-" + r.id; f.appendChild(aviso);
    f.onsubmit = async (ev) => {
      ev.preventDefault();
      const corpo = { rede_id: r.id, remotos: rem.value, aleatorio: cb.checked, queda_tcp: a.protocolo === "udp" ? Number(queda.value || 0) : 0, port_share: ps.value.trim() };
      if (sessao.mfa) { corpo.codigo = prompt("Código do autenticador") || ""; if (!corpo.codigo) return; }
      try { const x = await api("POST", "/api/redes/alcance/gravar", corpo); await carregarRedes(); msg("m-redes", "alcance gravado; " + x.aviso, true); } catch (e) { msg(aviso.id, e.message); }
    };
    caixa.appendChild(f);
  }
  bloco.appendChild(caixa);
}

let modo = "criar";
// O protocolo e do SERVIDOR (so faz sentido ao criar); o proxy vale nos dois
// -- ele so muda o perfil .ovpn de quem baixa, nunca a rede em si.
function ajustarProxyPeloProtocolo() {
  if (modo !== "criar") { $("c-proxy").hidden = false; return; }
  const esconde = $("c-protocolo").value !== "tcp";
  $("c-proxy").hidden = esconde;
  // Campo escondido nao pode mandar um proxy que o UDP recusaria.
  if (esconde) for (const i of $("c-proxy").querySelectorAll("input")) { if (i.type === "checkbox") i.checked = false; else i.value = ""; }
}
$("c-protocolo").onchange = ajustarProxyPeloProtocolo;
function abrirDialogo(m) {
  modo = m; $("f-rede").reset(); msg("m-rede", "");
  $("d-titulo").textContent = m === "criar" ? "Criar rede" : "Entrar na rede";
  $("d-ok").textContent = m === "criar" ? "Criar" : "Entrar";
  $("d-ok").className = m === "criar" ? "inclui" : "";
  $("c-finalidade").hidden = m !== "criar";
  $("c-transporte").hidden = m !== "criar";
  ajustarProxyPeloProtocolo();
  $("d-rede").showModal();
}
$("b-criar").onclick = () => abrirDialogo("criar");
$("b-entrar").onclick = () => abrirDialogo("entrar");
$("d-cancelar").onclick = () => $("d-rede").close();
$("f-rede").onsubmit = async (ev) => {
  ev.preventDefault();
  try {
    const corpo = dados(ev.target);
    // Caixa de marcar: o FormData manda "on" ou nada; o painel quer booleano.
    corpo.sem_ipv6 = "sem_ipv6" in corpo;
    const r = await api("POST", modo === "criar" ? "/api/redes" : "/api/redes/entrar", corpo);
    baixar(r.arquivo, r.perfil); $("d-rede").close(); carregarRedes();
  } catch (x) { msg("m-rede", x.message); }
};
$("b-atualizar").onclick = iniciar;
$("b-logout").onclick = sair;
$("b-voltar").onclick = iniciar;
$("b-admin").onclick = async () => { mostrar("tela-admin"); carregarAdmin(); };
$("b-mfa").onclick = () => { mostrar("tela-mfa"); carregarMfa(); };
$("b-mfa-voltar").onclick = iniciar;
$("b-senha").onclick = () => { $("f-senha").reset(); msg("m-senha", ""); mostrar("tela-senha"); };
$("b-senha-voltar").onclick = iniciar;
$("f-senha").onsubmit = async (ev) => {
  ev.preventDefault();
  try {
    await api("POST", "/api/senha", dados(ev.target));
    ev.target.reset();
    msg("m-senha", "senha trocada -- as outras sessões caíram", true);
  } catch (x) { msg("m-senha", x.message); }
};
let mfaAtivo = false;
async function carregarMfa() {
  $("mfa-novo").hidden = true; $("mfa-qr").replaceChildren(); $("mfa-segredo").textContent = ""; $("f-mfa").reset(); msg("m-mfa", "");
  try { mfaAtivo = (await api("GET", "/api/mfa")).ativo; } catch (x) { return msg("m-mfa", x.message); }
  $("mfa-estado").textContent = mfaAtivo ? "Ativo: o login e as redes que o exigem pedem o código." : "Não cadastrado.";
  $("b-mfa-iniciar").hidden = mfaAtivo;
  $("b-mfa-ok").textContent = mfaAtivo ? "Desativar" : "Confirmar";
  $("b-mfa-ok").className = mfaAtivo ? "exclui" : "";
  $("b-mfa-ok").hidden = !mfaAtivo;
  $("mfa-rotulo").textContent = mfaAtivo ? "Código atual (para desativar)" : "Código que o aplicativo mostra";
}
$("b-mfa-iniciar").onclick = async () => {
  try {
    const r = await api("POST", "/api/mfa/iniciar");
    // Analisado como XML e posto no DOM: imagem por data: a CSP do painel
    // (default-src 'self') recusa, e innerHTML nao entra nesta tela.
    const svg = new DOMParser().parseFromString(r.qr_svg, "image/svg+xml").documentElement;
    svg.setAttribute("width", "220"); svg.setAttribute("height", "220");
    $("mfa-qr").replaceChildren(document.importNode(svg, true));
    $("mfa-segredo").textContent = r.segredo.replace(/(.{4})/g, "$1 ").trim();
    $("mfa-novo").hidden = false; $("b-mfa-ok").hidden = false; $("b-mfa-iniciar").hidden = true;
  } catch (x) { msg("m-mfa", x.message); }
};
$("f-mfa").onsubmit = async (ev) => {
  ev.preventDefault();
  try {
    await api("POST", mfaAtivo ? "/api/mfa/desativar" : "/api/mfa/confirmar", dados(ev.target));
    const feito = mfaAtivo ? "autenticador desativado" : "autenticador ativo";
    await carregarMfa(); msg("m-mfa", feito, true);
  } catch (x) { msg("m-mfa", x.message); }
};

// Historico: o admin ve todos; o membro, so as proprias conexoes (o painel
// filtra -- a tela so mostra o que veio).
function bytesLegiveis(n) {
  if (n === null || n === undefined) return "—";
  const u = ["B", "KiB", "MiB", "GiB", "TiB"]; let i = 0; let v = n;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return (i ? v.toFixed(1) : String(v)) + " " + u[i];
}
function duracaoLegivel(s) {
  if (s === null || s === undefined) return "—";
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
  return h ? `${h} h ${m} min` : m ? `${m} min ${x} s` : `${x} s`;
}
function quando(iso) { return iso ? new Date(iso).toLocaleString() : "—"; }
async function carregarHistorico() {
  const tb = $("t-historico"); tb.textContent = ""; msg("m-historico", ""); msg("m-retencao", "");
  let h;
  try { h = await api("GET", "/api/historico"); } catch (x) { return msg("m-historico", x.message); }
  $("h-escopo").textContent = (h.todos ? "Todas as conexões de todas as redes" : "Só as suas conexões")
    + ` · guardadas por ${h.retencao_dias} dias · as ${h.teto} mais recentes`;
  $("f-retencao").hidden = !h.todos;
  $("f-retencao").elements.dias.value = h.retencao_dias;
  if (!h.conexoes.length) { msg("m-historico", "Nenhuma conexão registrada ainda.", true); return; }
  for (const c of h.conexoes) {
    const tr = document.createElement("tr");
    const saiu = c.estado === "saiu" ? quando(c.saiu_em) : c.estado;
    // IPv6 entre colchetes (a porta no fim nao se confunde); quem caiu para o
    // TCP entrou pela ponte, e o IP e o de fora que ela lembra.
    const ip = (c.ip_real.includes(":") ? `[${c.ip_real}]` : c.ip_real) + `:${c.porta_real}` + (c.pela_ponte ? " · TCP" : "");
    for (const v of [c.login, c.rede, ip, c.ip_vpn || "—", quando(c.entrou_em), saiu,
                     duracaoLegivel(c.segundos), bytesLegiveis(c.bytes_do_membro), bytesLegiveis(c.bytes_ao_membro)]) {
      const td = document.createElement("td"); td.textContent = v; tr.appendChild(td);
    }
    tb.appendChild(tr);
  }
}
$("b-historico").onclick = () => { mostrar("tela-historico"); carregarHistorico(); };
$("b-historico-voltar").onclick = iniciar;
$("f-retencao").onsubmit = async (ev) => {
  ev.preventDefault();
  const d = dados(ev.target);
  try {
    const r = await api("POST", "/api/historico/retencao", { dias: Number(d.dias), codigo: d.codigo });
    await carregarHistorico();
    msg("m-retencao", `retenção de ${r.retencao_dias} dias` + (r.apagadas ? ` · ${r.apagadas} conexão(ões) mais antiga(s) apagada(s)` : ""), true);
  } catch (x) { msg("m-retencao", x.message); }
};

async function carregarAdmin() {
  const tu = $("t-usuarios"), ts = $("t-servidores"); tu.textContent = ""; ts.textContent = "";
  const linha = (corpo, valores) => { const tr = document.createElement("tr"); for (const v of valores) { const td = document.createElement("td"); td.textContent = v; tr.appendChild(td); } corpo.appendChild(tr); };
  try {
    for (const u of await api("GET", "/api/usuarios")) {
      linha(tu, [u.login, u.email, u.admin ? "sim" : "não", u.mfa ? "cadastrado" : "—", u.ativo ? "ativo" : "desativado"]);
      if (u.mfa) {
        const b = document.createElement("button"); b.className = "exclui"; b.textContent = "Zerar";
        b.onclick = async () => {
          if (!await confirmar("Zerar autenticador", `Zerar o autenticador de «${u.login}»? Ele cadastra de novo no próximo login.`, "Zerar")) return;
          try { await api("POST", "/api/usuarios/mfa-zerar", { login: u.login }); carregarAdmin(); } catch (x) { msg("m-usuario", x.message); }
        };
        tu.lastChild.children[3].append(" ", b);
      }
      // Desativar e vermelho (exclui o acesso, sem apagar a conta); reativar
      // e verde (inclui de volta) -- a cor de sempre da casa.
      const ba = document.createElement("button");
      ba.className = u.ativo ? "exclui" : "inclui";
      ba.textContent = u.ativo ? "Desativar" : "Reativar";
      ba.onclick = async () => {
        const acao = u.ativo ? "Desativar" : "Reativar";
        const frase = u.ativo
          ? `Desativar «${u.login}»? As sessões dele caem na hora e ele deixa de conseguir entrar.`
          : `Reativar «${u.login}»? Ele volta a conseguir entrar no painel e nas redes.`;
        if (!await confirmar(`${acao} usuário`, frase, acao)) return;
        try { await api("POST", "/api/usuarios/ativo", { login: u.login, ativo: !u.ativo }); carregarAdmin(); } catch (x) { msg("m-usuario", x.message); }
      };
      tu.lastChild.children[4].append(" ", ba);
    }
  } catch (x) { msg("m-usuario", x.message); }
  try { for (const s of await api("GET", "/api/servidores")) linha(ts, [s.nome, s.ip, s.dns, String(s.redes)]); } catch (x) { msg("m-servidor", x.message); }
}
$("f-usuario").onsubmit = async (ev) => { ev.preventDefault(); try { await api("POST", "/api/usuarios", dados(ev.target)); ev.target.reset(); msg("m-usuario", "usuário incluído", true); carregarAdmin(); } catch (x) { msg("m-usuario", x.message); } };
$("f-servidor").onsubmit = async (ev) => { ev.preventDefault(); try { await api("POST", "/api/servidores", dados(ev.target)); ev.target.reset(); msg("m-servidor", "servidor incluído", true); carregarAdmin(); } catch (x) { msg("m-servidor", x.message); } };
$("f-cofre").onsubmit = async (ev) => { ev.preventDefault(); try { await api("POST", "/api/destrancar", dados(ev.target)); iniciar(); } catch (x) { msg("m-cofre", x.message); } };
$("f-login").onsubmit = async (ev) => {
  ev.preventDefault();
  try { sessao = await api("POST", "/api/login", dados(ev.target)); try { sessionStorage.setItem("phxvpn", JSON.stringify(sessao)); } catch (_) {} iniciar(); }
  catch (x) { msg("m-login", x.message); }
};
$("f-instalar").onsubmit = async (ev) => {
  ev.preventDefault(); msg("m-instalar", "instalando… (gera a autoridade certificadora e deriva a chave mestre)", true);
  try { await api("POST", "/api/instalar", dados(ev.target)); msg("m-instalar", "instalado", true); iniciar(); }
  catch (x) { msg("m-instalar", x.message); }
};
iniciar();
