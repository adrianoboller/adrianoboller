"use strict";
// A ficha da sessao vem no fragmento da URL (#f=...): o fragmento nao vai ao
// servidor nem vaza no Referer. Lida uma vez, some da barra de endereco.
const ficha = new URLSearchParams(location.hash.slice(1)).get("f") || "";
history.replaceState(null, "", location.pathname);

const $ = (id) => document.getElementById(id);
const el = (tag, cls, texto) => { const e = document.createElement(tag); if (cls) e.className = cls; if (texto !== undefined) e.textContent = texto; return e; };

async function api(metodo, rota, corpo) {
  const r = await fetch(rota, {
    method: metodo,
    headers: { "Content-Type": "application/json", "Authorization": "Bearer " + ficha },
    body: corpo ? JSON.stringify(corpo) : undefined,
  });
  const j = await r.json().catch(() => ({ erro: "resposta ilegível" }));
  if (!r.ok) throw new Error(j.erro || ("erro " + r.status));
  return j;
}

function aviso(texto, ok) { const r = $("rodape"); r.textContent = texto || ""; r.className = "rodape " + (ok ? "ok" : "erro"); }

async function copiar(texto, rotulo) {
  try { await navigator.clipboard.writeText(texto); aviso(rotulo + " copiado", true); }
  catch (_) { aviso("não consegui copiar: selecione e copie à mão"); }
}

let abertos = new Set();   // redes com os membros expandidos
let desenhoAnterior = "";

// So redesenha quando algo mudou de verdade. Redesenhar a lista inteira a
// cada 2 s trocava os botoes por baixo do mouse, e um clique que caisse no
// meio da troca se perdia (achado ao exercitar: «Ligar» com a senha lembrada
// nao fazia nada). O que muda todo segundo -- a idade da sessao -- se
// atualiza no proprio lugar.
function desenhar(redes) {
  const chave = JSON.stringify([redes.map((r) => ({ ...r, membros: r.membros.map((m) => ({ ...m, sessao: m.online })) })), [...abertos], naoLidas]);
  if (chave === desenhoAnterior) {
    for (const r of redes) for (const m of r.membros) {
      const c = document.querySelector(`.caminho[data-k="${CSS.escape(r.rede + "|" + m.ip)}"]`);
      if (c) c.textContent = m.caminho === "-" ? m.sessao : `${m.caminho} · ${m.sessao}`;
    }
    return;
  }
  desenhoAnterior = chave;
  desenharTudo(redes);
}

// Resumo: os mesmos dados da lista, contados -- nada aqui e digitado.
function desenharResumo(redes) {
  const ligadas = redes.filter((r) => r.ligada);
  const membros = redes.reduce((n, r) => n + r.membros.length, 0);
  const conectados = ligadas.reduce((n, r) => n + r.membros.filter((m) => m.online).length, 0);
  const caixa = $("resumo"); caixa.textContent = "";
  for (const [valor, rotulo, on] of [
    [redes.length, "redes neste computador", false],
    [ligadas.length, "ligadas agora", ligadas.length > 0],
    [membros, "membros conhecidos", false],
    [conectados, "conectados agora", conectados > 0],
  ]) {
    const i = el("div", "indicador" + (on ? " on" : ""));
    i.append(el("b", "", String(valor)), el("span", "", rotulo));
    caixa.appendChild(i);
  }
}

function desenharTudo(redes) {
  desenharResumo(redes);
  const lista = $("lista"); lista.textContent = "";
  if (!redes.length) {
    lista.appendChild(el("div", "vazio", "Nenhuma rede ainda. Crie uma, ou entre numa com o código de convite."));
    return;
  }
  for (const r of redes) {
    const bloco = el("div", "rede" + (r.ligada ? " ligada" : ""));
    const topo = el("div", "topo");
    const lamp = el("span", "lampada" + (r.ligada ? " on" : "")); lamp.title = r.ligada ? "ligada" : "desligada";
    const nome = el("span", "nome", r.rede); nome.style.cursor = "pointer";
    nome.onclick = () => { abertos.has(r.rede) ? abertos.delete(r.rede) : abertos.add(r.rede); atualizar(); };
    const acoes = el("span", "acoes");
    const conv = el("button", "", "Convidar"); conv.onclick = () => abrirConvidar(r.rede);
    const usb = el("button", "", "USB"); usb.title = "compartilhar e usar dispositivos USB nesta rede"; usb.onclick = () => abrirUsb(r);
    const liga = el("button", r.ligada ? "exclui" : "inclui", r.desligando ? "Desligando…" : (r.ligada ? "Desligar" : "Ligar"));
    liga.disabled = !!r.desligando;
    liga.onclick = () => r.ligada ? desligar(r.rede, liga) : (r.lembrada ? ligarLembrada(r.rede, liga) : abrirLigar(r));
    acoes.append(usb, conv, liga);
    if (r.lembrada && !r.ligada) {
      const esq = el("button", "", "Esquecer senha"); esq.title = "apaga a senha lembrada neste computador";
      esq.onclick = async () => { try { aviso((await api("POST", "/api/esquecer", { rede: r.rede })).ok, true); } catch (e) { aviso(e.message); } atualizar(); };
      acoes.insertBefore(esq, conv);
    }
    const online = r.membros.filter((m) => m.online).length;
    const info = el("span", "info", `meu IP ${r.ip} · ${r.membros.length} membro(s)` + (r.ligada ? `, ${online} conectado(s)` : "") + ` · modo ${r.modo}`);
    topo.append(lamp, nome, acoes, info);
    bloco.appendChild(topo);
    if (abertos.has(r.rede) || r.ligada) {
      for (const m of r.membros) {
        const linha = el("div", "membro");
        const p = el("span", "ponto" + (m.online ? " on" : "")); p.title = m.online ? "conectado" : "sem sessão";
        const ip = el("span", "ip", m.ip); ip.title = "clique para copiar"; ip.onclick = () => copiar(m.ip, "IP " + m.ip);
        const chave = el("span", "chave", m.chave);
        const cam = el("span", "caminho", m.caminho === "-" ? m.sessao : `${m.caminho} · ${m.sessao}`);
        cam.dataset.k = r.rede + "|" + m.ip;
        linha.append(p, ip, chave, cam);
        const grupo = el("span", "botoes");
        if (r.ligada && m.online) {
          const bp = el("button", "", "Ping"); bp.onclick = () => pingar(r.rede, m.ip, bp);
          const bc = el("button", "", "Chat" + (naoLidas[r.rede + "|" + m.ip] ? ` (${naoLidas[r.rede + "|" + m.ip]})` : ""));
          if (naoLidas[r.rede + "|" + m.ip]) bc.classList.add("novo");
          bc.onclick = () => abrirChat(r.rede, m.ip);
          grupo.append(bp, bc);
        }
        // So o DONO ve o botao (membro nao chega a pedir; o motor tambem
        // recusa -- a tela so evita o pedido inutil), e nunca para si mesmo:
        // remove independe de estar ligado, porque so reescreve o rol.
        if (r.sou_dono && !m.eu) {
          const br = el("button", "exclui", "Remover"); br.onclick = () => abrirRemover(r.rede, m.ip);
          grupo.append(br);
        }
        if (grupo.childNodes.length) linha.append(grupo);
        bloco.appendChild(linha);
      }
      if (!r.membros.length) bloco.appendChild(el("div", "membro", "sem membros ainda — use Convidar"));
    }
    lista.appendChild(bloco);
  }
}

// Mensagens de chat: o ultimo numero visto e as nao lidas, por rede e IP.
const vistas = {};         // rede -> ultimo numero recebido
const conversas = {};      // "rede|ip" -> [mensagens]
const naoLidas = {};       // "rede|ip" -> quantas
let chatAberto = null;     // "rede|ip" da janela de chat aberta

async function buscarMensagens(redes) {
  for (const r of redes.filter((x) => x.ligada)) {
    const novas = await api("POST", "/api/mensagens", { rede: r.rede, desde: vistas[r.rede] || 0 }).catch(() => []);
    for (const m of novas) {
      vistas[r.rede] = Math.max(vistas[r.rede] || 0, m.numero);
      const k = r.rede + "|" + m.ip;
      (conversas[k] = conversas[k] || []).push(m);
      if (!m.minha && k !== chatAberto) { naoLidas[k] = (naoLidas[k] || 0) + 1; aviso(`mensagem nova de ${m.ip} em ${r.rede}`, true); }
    }
  }
  if (chatAberto) desenharConversa();
}

async function atualizar() {
  try { const redes = await api("GET", "/api/redes"); await buscarMensagens(redes); desenhar(redes); }
  catch (e) { aviso(e.message); }
}

async function pingar(rede, ip, botao) {
  botao.disabled = true; aviso(`ping em ${ip}…`, true);
  try { aviso((await api("POST", "/api/ping", { rede, ip })).ok, true); } catch (e) { aviso(e.message); }
  botao.disabled = false;
}

function desenharConversa() {
  const caixa = $("conversa"); caixa.textContent = "";
  for (const m of conversas[chatAberto] || []) {
    const b = el("div", "bolha " + (m.minha ? "minha" : "dele"), m.texto);
    b.appendChild(el("small", "", new Date(m.quando * 1000).toLocaleTimeString()));
    caixa.appendChild(b);
  }
  caixa.scrollTop = caixa.scrollHeight;
}

function abrirChat(rede, ip) {
  const f = dialogo("d-chat");
  chatAberto = rede + "|" + ip; delete naoLidas[chatAberto];
  $("d-chat").querySelector("[data-ip]").textContent = ip;
  $("d-chat").onclose = () => { chatAberto = null; atualizar(); };
  desenharConversa();
  f.onsubmit = (ev) => { ev.preventDefault(); const texto = f.querySelector("[name=texto]").value;
    if (!texto.trim()) return;
    enviar(f, "/api/chat", { rede, ip, texto }, () => { f.querySelector("[name=texto]").value = ""; atualizar(); }); };
  f.querySelector("[name=texto]").focus();
}

async function ligarLembrada(rede, botao) {
  botao.disabled = true; aviso(`ligando ${rede} com a senha lembrada…`, true);
  try { aviso((await api("POST", "/api/ligar", { rede })).ok, true); } catch (e) { aviso(e.message); }
  atualizar();
}

function dialogo(id) {
  const d = $(id); const f = d.querySelector("form");
  f.reset(); f.querySelector(".msg").textContent = "";
  d.querySelectorAll("[data-fechar]").forEach((b) => b.onclick = () => d.close());
  d.showModal();
  return f;
}

async function enviar(form, rota, corpo, depois) {
  const botoes = form.querySelectorAll("button"); botoes.forEach((b) => b.disabled = true);
  try { const r = await api("POST", rota, corpo); depois(r); }
  catch (e) { form.querySelector(".msg").textContent = e.message; }
  finally { botoes.forEach((b) => b.disabled = false); }
}

$("b-criar").onclick = () => {
  const f = dialogo("d-criar");
  f.onsubmit = (ev) => { ev.preventDefault(); const d = Object.fromEntries(new FormData(f));
    enviar(f, "/api/criar", d, (r) => { $("d-criar").close(); aviso(r.ok, true); abertos.add(d.rede); atualizar(); }); };
};

$("b-entrar").onclick = () => {
  const f = dialogo("d-entrar");
  f.onsubmit = (ev) => { ev.preventDefault();
    enviar(f, "/api/entrar", Object.fromEntries(new FormData(f)), (r) => { $("d-entrar").close(); aviso(r.ok, true); atualizar(); }); };
};

function abrirConvidar(rede) {
  const f = dialogo("d-convidar");
  $("d-convidar").querySelector("[data-rede]").textContent = rede;
  const campo = f.querySelector("[name=codigo]").closest(".campo");
  campo.hidden = true; $("b-copiar").hidden = true; $("b-gerar").hidden = false;
  f.onsubmit = (ev) => { ev.preventDefault(); const d = Object.fromEntries(new FormData(f)); d.rede = rede;
    enviar(f, "/api/convidar", d, (r) => {
      f.querySelector("[name=codigo]").value = r.ok; campo.hidden = false;
      $("b-copiar").hidden = false; $("b-gerar").hidden = true;
      $("b-copiar").onclick = () => copiar(r.ok, "Convite");
    }); };
}

function abrirLigar(r) {
  const rede = r.rede;
  const f = dialogo("d-ligar");
  $("d-ligar").querySelector("[data-rede]").textContent = rede;
  // Usuario e senha do servidor intermediario so quando a rede passa por ele.
  const usa = r.modo !== "direto";
  $("repasse-conta").hidden = !usa;
  f.querySelector("[name=repasse_usuario]").value = r.repasse_usuario || "";
  f.onsubmit = (ev) => { ev.preventDefault(); const d = Object.fromEntries(new FormData(f)); d.rede = rede;
    d.lembrar = d.lembrar === "1";
    if (!usa) { delete d.repasse_usuario; delete d.repasse_senha; }
    enviar(f, "/api/ligar", d, (x) => { $("d-ligar").close(); aviso(x.ok, true); atualizar(); }); };
}

// Confirmacao DENTRO da pagina -- nada de confirm() do navegador.
function abrirRemover(rede, ip) {
  const f = dialogo("d-remover");
  $("d-remover").querySelector("[data-rede]").textContent = rede;
  $("d-remover").querySelector("[data-ip]").textContent = ip;
  f.onsubmit = (ev) => { ev.preventDefault();
    enviar(f, "/api/remover", { rede, ip }, (r) => { $("d-remover").close(); aviso(r.ok, true); atualizar(); }); };
}

// USB: o dialogo so desenha; quem decide e o motor (usb.rs), pela /api/usb.
async function usbApi(corpo) { return api("POST", "/api/usb", corpo); }

function linhaUsb(texto, marca, botoes) {
  const l = el("div", "usb-linha");
  l.append(el("span", "desc", texto));
  if (marca) l.append(el("span", "marca", marca));
  for (const b of botoes) l.append(b);
  return l;
}

function botaoUsb(rotulo, classe, acao) {
  const b = el("button", classe, rotulo); b.type = "button";
  b.onclick = async () => {
    const msg = $("f-usb").querySelector(".msg"); msg.textContent = ""; b.disabled = true;
    try { const r = await acao(); if (r && r.ok) aviso(r.ok, true); }
    catch (e) { msg.textContent = e.message; }
    finally { b.disabled = false; }
  };
  return b;
}

async function desenharUsbLocais(rede) {
  const caixa = $("usb-locais"); caixa.textContent = "";
  try {
    const v = await usbApi({ acao: "locais", rede });
    if (!v.length) { caixa.append(el("div", "usb-vazio", "Nenhum dispositivo USB neste computador.")); return; }
    for (const d of v) {
      const marca = d.nesta_rede ? "compartilhado nesta rede" : (d.preso ? "compartilhado em outra rede" : "");
      const b = d.nesta_rede
        ? botaoUsb("Parar", "altera", async () => { const r = await usbApi({ acao: "parar", rede, busid: d.busid }); desenharUsbLocais(rede); return r; })
        : botaoUsb("Compartilhar", "inclui", async () => { const r = await usbApi({ acao: "compartilhar", rede, busid: d.busid }); desenharUsbLocais(rede); return r; });
      caixa.append(linhaUsb(d.resumo, marca, [b]));
    }
  } catch (e) { caixa.append(el("div", "usb-vazio", e.message)); }
}

async function desenharUsbPortas() {
  const caixa = $("usb-portas"); caixa.textContent = "";
  try {
    const v = await usbApi({ acao: "portas" });
    if (v.sem_vhci) { caixa.append(el("div", "usb-vazio", "Para usar o USB de outro membro, carregue o módulo do kernel: sudo modprobe vhci-hcd")); return; }
    if (!v.length) { caixa.append(el("div", "usb-vazio", "Nenhum USB de outro membro em uso aqui.")); return; }
    for (const u of v) {
      caixa.append(linhaUsb(`porta ${u.porta} · ${u.texto}`, "", [
        botaoUsb("Soltar", "altera", async () => { const r = await usbApi({ acao: "soltar", porta: u.porta }); desenharUsbPortas(); return r; }),
      ]));
    }
  } catch (e) { caixa.append(el("div", "usb-vazio", e.message)); }
}

function desenharUsbMembros(r) {
  const caixa = $("usb-membros"); caixa.textContent = "";
  const online = r.ligada ? r.membros.filter((m) => m.online) : [];
  if (!online.length) {
    caixa.append(el("div", "usb-vazio", r.ligada ? "Nenhum membro conectado agora." : "Ligue a rede para ver o USB dos membros."));
    return;
  }
  for (const m of online) {
    const sub = el("div", "usb-sub");
    const ver = botaoUsb("Ver USB", "", async () => {
      sub.textContent = "";
      const v = await usbApi({ acao: "remotos", ip: m.ip });
      if (!v.length) sub.append(el("div", "usb-vazio", "Este membro não compartilha nada nesta rede."));
      for (const d of v) {
        sub.append(linhaUsb(d.resumo, "", [
          botaoUsb("Usar", "inclui", async () => { const x = await usbApi({ acao: "usar", ip: m.ip, busid: d.busid }); desenharUsbPortas(); return x; }),
        ]));
      }
    });
    caixa.append(linhaUsb(m.ip, "", [ver]), sub);
  }
}

function abrirUsb(r) {
  dialogo("d-usb");
  $("d-usb").querySelector("[data-rede]").textContent = r.rede;
  desenharUsbLocais(r.rede); desenharUsbMembros(r); desenharUsbPortas();
}

async function desligar(rede, botao) {
  botao.disabled = true;
  try { aviso((await api("POST", "/api/desligar", { rede })).ok, true); } catch (e) { aviso(e.message); }
  atualizar();
}

$("b-config").onclick = async () => {
  const f = dialogo("d-config");
  const caixa = f.querySelector("[name=inicia]");
  try {
    const s = await api("GET", "/api/sistema");
    caixa.checked = s.inicia;
    $("config-nota").textContent = s.bandeja
      ? "Abre direto na bandeja, ao lado do relógio; o duplo clique no ícone abre esta janela."
      : "No Linux, abre esta janela ao entrar. Ligar a rede exige permissão de administrador (root ou setcap).";
  } catch (e) { f.querySelector(".msg").textContent = e.message; }
  caixa.onchange = async () => {
    try { aviso((await api("POST", "/api/sistema", { inicia: caixa.checked })).ok, true); }
    catch (e) { f.querySelector(".msg").textContent = e.message; caixa.checked = !caixa.checked; }
  };
};

(async () => {
  if (!ficha) { aviso("abra esta janela pelo phxvpn (o endereço traz a ficha da sessão)"); return; }
  try { const c = (await api("GET", "/api/chave")).ok; const s = $("minha-chave"); s.textContent = c.slice(0, 12) + "…"; s.onclick = () => copiar(c, "Chave"); }
  catch (e) { aviso(e.message); }
  atualizar();
  setInterval(atualizar, 2000);
})();
