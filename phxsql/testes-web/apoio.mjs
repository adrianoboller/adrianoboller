/* O que todo caso usa: entrar pela tela, afirmar, e guardar a captura.
 *
 * Nada de framework. A regra de zero dependencias vale aqui tambem -- so o
 * Playwright, que e o navegador, e a `std` do Node. */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { connect } from 'node:net';

import { USUARIO, SENHA, TOKEN } from './servidor.mjs';

export class Falha extends Error {}

export function verdade(cond, oQue) {
  if (!cond) throw new Falha(oQue);
}

export function igual(achado, esperado, oQue) {
  if (achado !== esperado) {
    throw new Falha(`${oQue}: esperava ${JSON.stringify(esperado)}, achei ${JSON.stringify(achado)}`);
  }
}

export function contem(texto, pedaco, oQue) {
  if (!String(texto).includes(pedaco)) {
    throw new Falha(`${oQue}: nao achei ${JSON.stringify(pedaco)} em ${JSON.stringify(String(texto).slice(0, 400))}`);
  }
}

/** Entra pela TELA DE LOGIN -- e nao por um atalho que pula o formulario.
 *
 * O caminho da pessoa e o caminho do teste: o desafio-resposta, o
 * `crypto.subtle` e o `abrirApp()` inteiro so se exercitam clicando no
 * botao. Preencher `est.usuario` por dentro provaria o resto e nao provaria
 * a entrada, que e por onde todo mundo passa.
 *
 * `credenciais` e opcional -- por padrao entra como o supervisor que sobe
 * junto do servidor da bateria (`USUARIO`/`SENHA`/`TOKEN`). Um caso que
 * precisa provar o DIREITO POR COLUNA (que so existe para quem NAO e
 * supervisor) cria o usuario restrito pela API, como o supervisor, e chama
 * `entrar` de novo com `{ usuario, senha, token }` dele -- numa aba nova,
 * porque a sessao do supervisor nao pode ser a mesma que testa a restricao. */
export async function entrar(page, url, credenciais = CREDENCIAL) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  // Sem servidor a pagina cai em «modo demonstracao» com dados embutidos, e
  // a bateria passaria inteira sem tocar no motor. Este e o teste que impede
  // a bateria de se enganar sozinha.
  // `est` e um `const` de topo de script: existe no escopo global lexico, e
  // por isso se le pelo NOME e nao por `window.est`, que e undefined.
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 15000 })
    .catch(() => { throw new Falha('a pagina caiu em modo demonstracao: nao achou o servidor'); });
  await page.fill('#u', credenciais.usuario ?? credenciais.USUARIO);
  await page.fill('#s', credenciais.senha ?? credenciais.SENHA);
  await page.fill('#t', credenciais.token ?? credenciais.TOKEN);
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo', { timeout: 20000 });
  await page.waitForSelector('#arvore .no', { timeout: 20000 });
  // E ENTAO ESPERA A ENTRADA TERMINAR, que nao e a mesma coisa.
  //
  // `#arvore .no` aparece no meio do `abrirApp`: a arvore ja esta no DOM e a
  // primeira tela ainda esta sendo pedida ao servidor. Medido na SP000056:
  // sao 32 ms de mediana (min 29, max 35, n=12) entre uma coisa e a outra --
  // e a viagem do `page.evaluate` seguinte cai dentro dessa janela ou fora
  // dela conforme o humor da maquina. Quem pintava a propria tela ali via o
  // Painel chegar por cima: o caso `telemetria` reprovava em 4 de 40 corridas
  // isoladas e em 5 de 14 com a maquina carregada, sempre trocando de tema,
  // porque o que sorteava era o relogio e nao o tema.
  //
  // `data-pronto` e a marca que o `abrirApp` poe quando acaba de verdade --
  // arvore montada, primeira tela pintada, abas pinadas de volta. Esperar por
  // ela e o que a pessoa faz sem pensar: ninguem clica no menu 30 ms depois
  // de a tela abrir.
  await page.waitForSelector('#app.ativo[data-pronto="1"]', { timeout: 20000 });
}

/** Chama uma operacao do protocolo pela MESMA `api()` da pagina.
 *
 * Serve para montar o cenario (criar banco, criar tabela) sem fingir que
 * isso e o teste. O que se PROVA e sempre pelo clique. */
export async function api(page, op, params = {}) {
  return await page.evaluate(([o, p]) => api(o, p), [op, params]);
}

/** Uma captura com nome estavel, para a avaliacao de design comparar rodadas. */
export async function capturar(ctx, nome, opc = {}) {
  if (!ctx.capturas) return null;
  mkdirSync(ctx.capturas, { recursive: true });
  const arq = join(ctx.capturas, `${nome}.png`);
  await ctx.page.screenshot({ path: arq, fullPage: !!opc.inteira });
  return arq;
}

/** Espera o painel parar de mudar -- desenho assincrono termina depois do clique. */
export async function assentar(page, ms = 250) {
  await page.waitForTimeout(ms);
}

export const CREDENCIAL = { USUARIO, SENHA, TOKEN };

/** Um banco so deste caso e deste tema.
 *
 * Todos os casos falam com o MESMO servidor -- um banco por caso e o que
 * impede o «excluir tabela» de um de estragar a grade do outro, e o que
 * deixa a bateria rodar os dois temas sem uma rodada sujar a outra. */
export function bancoDoCaso(ctx, apelido) {
  return `bat${apelido}${ctx.tema === 'claro' ? 'C' : 'E'}`;
}

/** O cenario padrao: um banco, uma tabela com os sete tipos e tres linhas.
 *
 * «Blumenau» esta aqui de proposito: e o dado com que a armadilha do
 * `label{text-transform:uppercase}` se pega. Um dado todo em maiuscula na
 * origem nao provaria nada. */
export async function cenario(page, db, tab = 'clientes') {
  await api(page, 'criar_database', { database: db }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: db, tabela: tab,
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true },
      { nome: 'nome', tipo: 'Str(40)', obrigatoria: true },
      { nome: 'cidade', tipo: 'Str(30)' },
      { nome: 'uf', tipo: 'Str(2)' },
      { nome: 'limite', tipo: 'Decimal(12,2)' },
      { nome: 'cadastro', tipo: 'Date' },
      { nome: 'ficha', tipo: 'Memo' },
    ],
    indices: [
      { nome: 'porId', colunas: ['id'], unico: true, primario: true },
      { nome: 'porNome', colunas: ['nome'], nocase: true },
    ],
  }).catch(() => {});
  const linhas = [
    [1, 'Adriano Boller', 'Blumenau', 'SC', '15000.00', '2024-03-11', 'cliente antigo'],
    [2, 'Maria Souza', 'Joinville', 'SC', '2500.50', '2025-01-02', ''],
    [3, 'Carlos Lima', 'Curitiba', 'PR', '900.00', '2025-06-30', ''],
  ];
  for (const l of linhas) {
    await api(page, 'inserir', { database: db, tabela: tab, valores: l }).catch(() => {});
  }
  return { db, tab };
}

/** Abre a linha de uma PhxGrid -- por DUPLO clique, e nao por um clique.
 *
 * A tabela montada a mao abria a ficha com um clique simples, e os casos
 * clicavam em `.linha-dado`. Quando a tela virou PhxGrid, duas coisas mudaram
 * ao mesmo tempo, e so a segunda e visivel lendo o codigo:
 *
 * 1. a classe `.linha-dado` deixou de existir (a grade poe as suas);
 * 2. o gesto passou a ser o DUPLO clique, porque o simples e da SELECAO.
 *
 * O segundo nao e defeito a consertar: e a convencao do DevExpress e do
 * Janus, que sao os dois moldes que o dono nomeou, e esta escrita no LEIAME
 * da grade. Quem muda e o teste, que passa a fazer o gesto da pessoa.
 *
 * O ajudante existe para o gesto morar num lugar so. Espalhado por quatro
 * casos, o dia em que a grade mudar de gesto de novo deixa tres deles
 * quebrados e um consertado -- e o quarto vira o que ninguem acha.
 *
 * `rowid` opcional: sem ele abre a primeira linha de dado; com ele procura a
 * linha cuja celula `rowid` bate, que e como o caso da ficha precisa. Linha
 * de GRUPO (`.phx-grupo`) nunca conta: ela nao e linha de dado, e a propria
 * grade a recusa no `aoAbrirLinha`. */
/** Clica -- e se nao conseguir, DIZ POR QUE em vez de dizer «timeout».
 *
 *  Nasceu da SP000056. O caso da telemetria estourava com
 *  `page.click: Timeout 30000ms exceeded. waiting for locator(...)`, e essa
 *  frase nao distingue as cinco coisas diferentes que o Playwright espera
 *  antes de clicar: existir, estar visivel, estar PARADO, receber o evento, e
 *  estar habilitado. Sem saber qual delas falhou, o diagnostico vira palpite
 *  -- e eu errei quatro seguidos por causa disso: chamei de flake, depois de
 *  regressao, depois de layout, depois de timeout curto.
 *
 *  O que ela mede no instante da falha, que e o unico instante que importa:
 *  onde o alvo esta, onde a janela esta, se ele rolou para dentro, e SOBRETUDO
 *  **quem esta no ponto do clique** -- porque «elemento coberto por outro» e a
 *  causa que mais se parece com «timeout» e menos se parece com ela mesma.
 *
 *  Nao muda o veredito: o que falhava continua falhando. Muda o que a falha
 *  CONTA. */
export async function clicarOuExplicar(page, seletor, opc = {}) {
  try {
    await page.click(seletor, opc);
    return;
  } catch (erro) {
    const diag = await page.evaluate(sel => {
      const alvo = document.querySelector(sel);
      if (!alvo) return { achou: false };
      alvo.scrollIntoView({ block: 'center' });
      const r = alvo.getBoundingClientRect();
      const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
      const noPonto = document.elementFromPoint(cx, cy);
      const quem = e => !e ? 'NADA (ponto fora da janela)'
        : `${e.tagName}${e.id ? '#' + e.id : ''}${e.className && typeof e.className === 'string'
            ? '.' + e.className.trim().split(/\s+/).join('.') : ''}`;
      const cs = getComputedStyle(alvo);
      return {
        achou: true,
        alvo: quem(alvo),
        caixa: `${Math.round(r.width)}x${Math.round(r.height)} em (${Math.round(r.left)},${Math.round(r.top)})`,
        janela: `${innerWidth}x${innerHeight}`,
        dentroDaJanela: r.top >= 0 && r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth,
        display: cs.display, visibility: cs.visibility, opacidade: cs.opacity,
        ponteiro: cs.pointerEvents,
        desabilitado: !!alvo.disabled,
        // A pergunta decisiva: quem responde no ponto do clique? Se nao for o
        // alvo nem filho dele, o Playwright fica tentando para sempre.
        noPontoDoClique: quem(noPonto),
        ehOAlvoOuFilho: !!noPonto && (noPonto === alvo || alvo.contains(noPonto)),
      };
    }, seletor).catch(e => ({ achou: 'a sonda nao rodou: ' + e.message }));
    throw new Falha(
      `nao consegui clicar em ${seletor} -- e o estado no instante da falha:\n`
      + `      ${JSON.stringify(diag, null, 2).replace(/\n/g, '\n      ')}\n`
      + `      (erro original: ${String(erro.message).split('\n')[0]})`);
  }
}

export async function abrirLinhaDaGrade(page, { em = '#painel', rowid = null } = {}) {
  const linha = `${em} .phx-grid tbody tr:not(.phx-grupo)`;
  await page.waitForSelector(linha, { timeout: 15000 });
  if (rowid == null) {
    await page.locator(linha).first().dblclick();
    return;
  }
  // A celula do CORPO nao carrega `data-campo` -- so o cabecalho carrega --,
  // entao a coluna do `rowid` se acha pela POSICAO, e a posicao sai do
  // cabecalho servido, medida na hora. Filtrar por `td[data-campo="rowid"]`
  // nao acha nada e falha dizendo «nao achei a linha», que e a mensagem certa
  // para a causa errada: parece dado faltando e e seletor furado.
  const i = await page.$$eval(`${em} .phx-grid thead tr:not(.phx-frow)`, trs => {
    const leaf = trs[trs.length - 1];
    return [...leaf.querySelectorAll('th')].map(t => t.getAttribute('data-campo'));
  }).then(cs => cs.indexOf('rowid') + 1);
  if (i === 0) throw new Falha(`a grade de ${em} nao tem coluna rowid`);
  const alvo = page.locator(linha)
    .filter({ has: page.locator(`td:nth-child(${i}):text-is("${rowid}")`) }).first();
  if (await alvo.count() === 0) {
    throw new Falha(`nao achei a linha rowid=${rowid} na grade de ${em}`);
  }
  await alvo.dblclick();
}

/** Abre a tabela pela ARVORE, clicando -- que e como a pessoa chega la. */
export async function abrirPelaArvore(page, db, tab) {
  await page.evaluate(() => montarArvore(false));
  await page.locator(`.no.tab[data-db="${db}"][data-tab="${tab}"]`).first().click();
  await page.waitForSelector('#painel table, #painel .vazio', { timeout: 10000 }).catch(() => {});
}

/** Abre uma tela pelo MENU, achando o item pela CHAVE da fabrica de idiomas.
 *
 * Nasceu de um defeito que ficou dois dias verde e depois reprovou cinco
 * passos de uma vez. A prova dos idiomas abria a tela de Configuracoes pela
 * BARRA DE FERRAMENTAS, assim:
 *
 *     .fer[title^="Config"], .fer[title^="Konfig"], #ferramentas .fer >> nth=13
 *
 * Duas coisas erradas na mesma linha, e a segunda escondeu a primeira. O
 * `title^=` compara a REDACAO -- e ela muda com o idioma, que e justamente o
 * que esta prova exercita. E o `nth=13` e POSICAO: quando o botao «Config»
 * saiu da barra (c153d71, «Config sai da barra», com o caminho do menu
 * conferido no lugar), o seletor nao ficou vazio -- ele passou a acertar o
 * 14o botao, que hoje e «Restaurar». A prova clicava, a tela abria, e o
 * `#idiomasAqui` nunca aparecia: cinco passos reprovando por um seletor que
 * mirava a peca errada com toda a confianca do mundo.
 *
 * A chave (`tela.mi_gerais_servidor`) nao muda com o idioma nem com a ordem
 * dos itens, e e a MESMA que a fabrica usa para traduzir o rotulo: quem
 * renomear o item ou o mover de menu continua sendo achado, e quem o APAGAR
 * faz esta funcao falhar dizendo o nome da chave -- em vez de clicar noutra
 * coisa. */
export async function abrirPeloMenu(page, chave) {
  // `MENUS` e const de topo de script, como o `est`: le-se pelo NOME.
  const onde = await page.evaluate(c => {
    for (let m = 0; m < MENUS.length; m++) {
      const itens = MENUS[m][3];
      for (let i = 0; i < itens.length; i++) {
        if (itens[i] !== 'sep' && itens[i].txt === c) return { m, i };
      }
    }
    return null;
  }, chave);
  if (!onde) throw new Falha(`nao achei item de menu com a chave «${chave}»`);
  await page.click(`.menubar .titulo[data-m="${onde.m}"]`);
  await page.click(`.menubar .item[data-m="${onde.m}"][data-i="${onde.i}"]`);
}

/** Uma CONEXAO VIVA na porta de dados, aberta de fora do navegador.
 *
 * E o "segundo cliente" que as telas de sessoes e de telemetria precisam para
 * ter o que encerrar: sem ela o unico cliente do servidor da bateria e a
 * propria pagina, e derruba-lo derrubaria o caso. Fala o protocolo de verdade
 * (um `ping` com o token e a linha de resposta) -- nao e um soquete mudo, e
 * por isso a sessao aparece na lista com origem e porta.
 *
 * `fechou` resolve quando o SERVIDOR fecha o soquete (`close`), que e a unica
 * prova de que o botao de encerrar fez o que diz: a tela mudar nao e a
 * conexao cair. `local` e a porta de origem, a mesma que a tela mostra. */
export async function conexaoViva(ctx, { de = '127.0.0.1', login = false } = {}) {
  const socket = connect({ host: '127.0.0.1', port: ctx.portaDados, localAddress: de });
  socket.setEncoding('utf8');
  let fechado = false;
  const fechou = new Promise(res => socket.on('close', () => { fechado = true; res(true); }));
  socket.on('error', () => {});
  await new Promise((res, rej) => {
    socket.once('connect', res);
    socket.once('error', rej);
  });
  // Respostas em fila: o protocolo e uma linha de JSON por resposta, e uma
  // resposta pode chegar em varios pedacos de rede.
  let buf = '';
  const fila = [];
  const espera = [];
  socket.on('data', d => {
    buf += d;
    let i;
    while ((i = buf.indexOf('\n')) >= 0) {
      const linha = buf.slice(0, i); buf = buf.slice(i + 1);
      if (!linha.trim()) continue;
      const r = JSON.parse(linha);
      if (espera.length) espera.shift()(r); else fila.push(r);
    }
  });
  const proxima = (ms = 30000) => new Promise((res, rej) => {
    if (fila.length) return res(fila.shift());
    const t = setTimeout(() => rej(new Falha('a conexao viva nao recebeu resposta a tempo')), ms);
    espera.push(r => { clearTimeout(t); res(r); });
  });
  const enviar = o => socket.write(JSON.stringify({ token: TOKEN, ...o }) + '\n');
  const perguntar = async o => { enviar(o); return await proxima(); };

  const pong = await perguntar({ op: 'ping' });
  if (!pong.ok) throw new Falha(`a conexao viva nao recebeu o ping: ${JSON.stringify(pong)}`);
  if (login) {
    const r = await perguntar({ op: 'login', usuario: USUARIO, senha: SENHA });
    if (!r.ok) throw new Falha(`a conexao viva nao conseguiu entrar: ${JSON.stringify(r)}`);
  }
  return {
    socket, local: socket.localPort, fechou, enviar, proxima, perguntar,
    aberta: () => !fechado && !socket.destroyed,
    derrubar: () => { if (!socket.destroyed) socket.destroy(); },
  };
}

/** Faz o servidor BLOQUEAR um IP de verdade: seis pedidos com token errado,
 * de dentro do proprio loopback (127.0.0.2 chega em 127.0.0.1 no Linux, e
 * assim o IP bloqueado nunca e o que o navegador usa).
 *
 * Devolve a recusa do ultimo pedido, que ja diz «bloqueado». O limite e o da
 * politica de fabrica (`tentativas_ate_bloquear` = 5): se um dia ela mudar, o
 * caso reprova aqui dizendo que o IP nao bloqueou, e nao 40 linhas adiante
 * dizendo que a lista esta vazia. */
export async function bloquearIp(ctx, ip = '127.0.0.2') {
  let ultimo = '';
  for (let i = 0; i < 6; i++) {
    ultimo = await new Promise(res => {
      const c = connect({ host: '127.0.0.1', port: ctx.portaDados, localAddress: ip });
      let b = '';
      c.setEncoding('utf8');
      c.on('data', d => { b += d; if (b.includes('\n')) { c.destroy(); res(b); } });
      c.on('error', () => res(b));
      c.on('close', () => res(b));
      c.on('connect', () => c.write(JSON.stringify({ op: 'ping', token: 'errado-de-proposito' }) + '\n'));
    });
  }
  if (!/bloqueado/.test(ultimo)) throw new Falha(`o IP ${ip} nao ficou bloqueado depois de 6 tokens errados: ${ultimo.slice(0, 200)}`);
  return ultimo;
}
