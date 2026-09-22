/* Exercita as DUAS telas dos pedidos 379 e 374 contra motor vivo.
 *
 * Por que existe: «interface só se prova exercitando». Ler o código não mostra
 * que o CSS global morde o componente novo, nem que a sonda do assistente fala
 * um fio e o bloco gerado escreve outro. Aqui há dois phxsqld de verdade — um
 * SOURCE e uma RÉPLICA —, o assistente é percorrido CLICANDO, e o bloco que
 * ele entrega é lido da tela e conferido campo a campo.
 *
 * O que ele prova, e nos dois sentidos:
 *
 *  - 379/a: o bloco gerado traz `"cifra"` ESCRITO, e traz `chave_do_fio`
 *    quando o operador soube o pino. Com a cifra desmarcada, traz
 *    `"cifra": false` e NÃO traz o pino.
 *  - 379/b: a sonda do passo 3 fala o mesmo fio que o laço vai falar. A prova
 *    é o sentido negativo: contra um source com `cifra_fio.exigir: true`, a
 *    sonda EM CLARO tem de falhar. Antes desta rodada ela passava, porque
 *    caía no padrão do motor e nunca via a escolha da tela.
 *  - 379/c: pino torto é recusado ANTES de sair da tela (o motor o recusaria
 *    na declaração, com o servidor já reiniciado).
 *  - 374: a tela de Configurações diz que o fio do relé vai em claro, e o
 *    parágrafo do `AUTH LOGIN` só aparece no servidor que manda credencial —
 *    é por isso que há dois servidores com o `alertas.email` diferente.
 *
 * Como roda (o binário NÃO se compila aqui — usa o de target/release/):
 *
 *     node testes-web/exercitar-fio-e-rele.mjs
 *
 * Grava as capturas em /tmp/claude-0/.../fio-e-rele/ (ou no diretório dado no
 * primeiro argumento). Derruba os dois servidores pelo PID guardado — nunca
 * por `pkill -f`, que pegaria o phxsqld do vizinho. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { connect } from 'node:net';
import { join, resolve } from 'node:path';

const RAIZ = resolve(new URL('.', import.meta.url).pathname, '..');
const SAIDA = process.argv[2] || join(tmpdir(), 'fio-e-rele');
const phxsqld = join(RAIZ, 'target/release/phxsqld');

/* A faixa desta prova. Fora dela não se encosta. */
const SRC_DADOS = 6860, SRC_WEB = 6861;
const REP_DADOS = 6862, REP_WEB = 6863;
const VELHO_DADOS = 6864, VELHO_WEB = 6865;
/* Portas que ninguém atende: são as origens mortas que enchem a grade com os
 * outros dois estados do fio sem precisar de mais dois servidores. */
const MORTA_A = 6898, MORTA_B = 6899;

const USUARIO = 'adm', SENHA = 'segredo1';
const TOKEN_SRC = 'fio-source', TOKEN_REP = 'fio-replica', TOKEN_VELHO = 'fio-velho';
const SENHA_RELE = 'MARCA-SEGREDO-RELE-379';

const diz = (...a) => console.log(...a);
const dormir = ms => new Promise(r => setTimeout(r, ms));

const falhas = [];
function verdade(cond, oQue) {
  if (cond) { diz(`  ✓ ${oQue}`); return true; }
  diz(`  ✗ ${oQue}`);
  falhas.push(oQue);
  return false;
}

/* ------------------------------------------------------------ os servidores */

function hashDaSenha(senha) {
  const r = spawnSync(phxsqld, ['--senha'], { input: senha, encoding: 'utf8' });
  const m = /"senha_hash": "([^"]+)"/.exec(r.stdout || '');
  if (!m) throw new Error(`phxsqld --senha nao devolveu o hash: ${r.stdout}${r.stderr}`);
  return m[1];
}

async function esperarPorta(porta, prazoMs = 25000) {
  const fim = Date.now() + prazoMs;
  while (Date.now() < fim) {
    const abriu = await new Promise(r => {
      const s = connect({ host: '127.0.0.1', port: porta }, () => { s.destroy(); r(true); });
      s.on('error', () => r(false));
      s.setTimeout(500, () => { s.destroy(); r(false); });
    });
    if (abriu) return true;
    await dormir(150);
  }
  return false;
}

async function subir(nome, dir, conf, portaWeb, portaDados) {
  const caminho = join(dir, 'config.json');
  writeFileSync(caminho, JSON.stringify(conf, null, 2));
  const proc = spawn(phxsqld, ['--config', caminho], { cwd: dir, stdio: ['ignore', 'pipe', 'pipe'] });
  const saida = [];
  proc.stdout.on('data', d => saida.push(String(d)));
  proc.stderr.on('data', d => saida.push(String(d)));
  let morreu = null;
  proc.on('exit', c => { morreu = c; });
  const matar = () => {
    try { process.kill(proc.pid, 'SIGTERM'); } catch { /* ja morreu */ }
    setTimeout(() => { try { process.kill(proc.pid, 'SIGKILL'); } catch { /* ok */ } }, 4000).unref();
  };
  if (!(await esperarPorta(portaWeb)) || !(await esperarPorta(portaDados))) {
    matar();
    throw new Error(`${nome}: as portas nao abriram (saida=${morreu}):\n${saida.join('')}`);
  }
  diz(`${nome}: pid ${proc.pid} — dados ${portaDados}, web ${portaWeb}`);
  return {
    caminho, url: `http://127.0.0.1:${portaWeb}/`, arranque: () => saida.join(''),
    async derrubar() { matar(); for (let i = 0; i < 60 && morreu === null; i++) await dormir(100); },
  };
}

function configBase(base, portaDados, portaWeb, token, hash) {
  return {
    base, bind: `127.0.0.1:${portaDados}`, token, max_linhas: 1000,
    // `atras_de_proxy` é o escape ESCRITO do portão de rede HTTP: desde que
    // `cifra_fio.exigir` nasceu ligado (18/09/2026), a porta web recusa todo
    // pedido com 403 enquanto ninguém declarar o proxy — e a tela nem chega a
    // pintar o botão de entrar. Aqui o proxy é o próprio localhost.
    web: { ligado: true, bind: `127.0.0.1:${portaWeb}`, sessao_minutos: 60,
           atras_de_proxy: true },
    recursos: { durabilidade: 'sistema', cache_paginas: 256 },
    usuarios: [{
      id: 10, nome: 'Adriano Boller', login: USUARIO,
      senha_hash: hash, supervisor: true, ativo: true, bases: {},
    }],
  };
}

/* --------------------------------------------------------------- a entrada */

async function entrar(page, url, token) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false,
    { timeout: 25000 }).catch(() => { throw new Error('a pagina caiu em modo demonstracao'); });
  await page.fill('#u', USUARIO);
  await page.fill('#s', SENHA);
  await page.fill('#t', token);
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo[data-pronto="1"]', { timeout: 25000 });
}

/** Um banco com uma tabela: sem isso a sonda TRAVA em «o outro servidor não
 * tem banco nenhum ainda», e o assistente nunca chega ao passo do bloco. */
async function popular(page) {
  await page.evaluate(() => api('criar_database', { database: 'Comercial' })).catch(() => {});
  await page.evaluate(() => api('criar_tabela', {
    database: 'Comercial', tabela: 'clientes',
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true },
      { nome: 'nome', tipo: 'Str(40)', obrigatoria: true },
      { nome: 'cidade', tipo: 'Str(30)' },
    ],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  })).catch(e => diz('  criar_tabela:', String(e)));
  await page.evaluate(() => api('inserir',
    { database: 'Comercial', tabela: 'clientes', valores: [1, 'Adriano Boller', 'Blumenau'] })).catch(() => {});
}

/** Captura UM elemento — e não a página: o que se julga aqui é o componente. */
async function retratar(page, seletor, arquivo) {
  const el = await page.$(seletor);
  if (!el) { diz(`  ⚠ nao achei ${seletor} para ${arquivo}`); return; }
  await el.screenshot({ path: join(SAIDA, arquivo) });
  diz(`  ✓ ${arquivo}`);
}

/* ------------------------------------------------------------------- main */

mkdirSync(SAIDA, { recursive: true });
const dirSrc = mkdtempSync(join(tmpdir(), `phx-fio-src-${process.pid}-`));
const dirRep = mkdtempSync(join(tmpdir(), `phx-fio-rep-${process.pid}-`));
const dirVelho = mkdtempSync(join(tmpdir(), `phx-fio-velho-${process.pid}-`));
const hash = hashDaSenha(SENHA);

/* O SOURCE: relé de e-mail COM usuário — é o que põe credencial no fio, e o
 * parágrafo do AUTH LOGIN tem de aparecer só aqui. */
const confSrc = {
  ...configBase(join(dirSrc, 'dados'), SRC_DADOS, SRC_WEB, TOKEN_SRC, hash),
  replicacao: { papel: 'source', id_servidor: 'source-1', imagem_da_linha: true },
  alertas: {
    ligado: true, livre_minimo_percentual: 10, livre_minimo_mb: 512,
    email: {
      ligado: true, servidor: '127.0.0.1', porta: 25, de: 'phxsql@local.test',
      para: ['dba@local.test'], usuario: 'phxsql', senha: SENHA_RELE,
    },
  },
};
const src = await subir('source', dirSrc, confSrc, SRC_WEB, SRC_DADOS);

/* O pino do source sai do PRÓPRIO binário, com o config dele: pinar uma chave
 * calculada de outro jeito seria pinar uma coisa e o servidor apresentar
 * outra. É o mesmo caminho que a tela manda o operador seguir. */
const pino = (spawnSync(phxsqld, ['--config', src.caminho, '--chave-do-fio'],
  { encoding: 'utf8' }).stdout || '').trim();
diz(`pino do source: ${pino.slice(0, 16)}… (${pino.length} dígitos)`);

/* A RÉPLICA: relé de e-mail SEM usuário — o parágrafo do AUTH LOGIN não pode
 * aparecer. E três origens, uma de cada estado de fio, para a coluna nova da
 * grade mostrar os três de uma vez. */
const confRep = {
  ...configBase(join(dirRep, 'dados'), REP_DADOS, REP_WEB, TOKEN_REP, hash),
  replicacao: {
    papel: 'replica', id_servidor: 'replica-1',
    origens: [
      { nome: 'matriz', host: '127.0.0.1', porta: SRC_DADOS, token: TOKEN_SRC,
        reconectar_em: 30, cifra: true, chave_do_fio: pino },
      { nome: 'filial', host: '127.0.0.1', porta: MORTA_A, token: TOKEN_SRC,
        reconectar_em: 300, cifra: true },
      { nome: 'legado', host: '127.0.0.1', porta: MORTA_B, token: TOKEN_SRC,
        reconectar_em: 300, cifra: false },
    ],
  },
  alertas: {
    ligado: true, livre_minimo_percentual: 10, livre_minimo_mb: 512,
    email: {
      ligado: true, servidor: '127.0.0.1', porta: 25, de: 'phxsql@local.test',
      para: ['dba@local.test'],
    },
  },
};
const rep = await subir('replica', dirRep, confRep, REP_WEB, REP_DADOS);

/* O SOURCE VELHO: `cifra_fio.ligada: false`. Ele é o «source anterior ao
 * aperto» que a tela nomeia, e existe para os dois sentidos serem provados
 * contra motor vivo em vez de por leitura: contra ele a sonda CIFRADA falha
 * (e a tela tem de dizer por quê) e a sonda EM CLARO passa, que é o único
 * caminho até o bloco com `"cifra": false`. */
const confVelho = {
  ...configBase(join(dirVelho, 'dados'), VELHO_DADOS, VELHO_WEB, TOKEN_VELHO, hash),
  replicacao: { papel: 'source', id_servidor: 'velho-1', imagem_da_linha: true },
  cifra_fio: { ligada: false, exigir: false },
};
const velho = await subir('source velho', dirVelho, confVelho, VELHO_WEB, VELHO_DADOS);

const navegador = await chromium.launch();
try {
  for (const tema of ['escuro', 'claro']) {
    diz(`\n══ tema ${tema} ══`);
    const ctx = await navegador.newContext({ viewport: { width: 1280, height: 900 } });
    await ctx.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch { /* privado */ } }, tema);
    const page = await ctx.newPage();
    const erros = [];
    page.on('pageerror', e => erros.push(e.message || String(e)));

    /* ---------------------------------------- 374: o rodapé do relé (source) */
    diz('— 374: tela de Configurações do SOURCE (relé com usuário)');
    await entrar(page, src.url, TOKEN_SRC);
    if (tema === 'escuro') { diz('  populando o source…'); await popular(page); }
    await page.evaluate(() => verConfigServidor());
    await dormir(500);
    await page.evaluate(() => {
      const h = [...document.querySelectorAll('#painel h3.secao')]
        .find(x => x.textContent.includes('Alerta de espaço'));
      if (h) h.scrollIntoView({ block: 'start' });
    });
    await dormir(250);
    /* A nota do relé é a que fala de SMTP — e não «a primeira `.nota`», que
     * em 18/09/2026 pegou a da replicação aberta e publicou a captura errada.
     * O índice sai da PÁGINA, não de um palpite sobre a ordem dos blocos. */
    const iNota = await page.evaluate(() =>
      [...document.querySelectorAll('#painel .nota')].findIndex(x => x.textContent.includes('SMTP')));
    if (iNota >= 0) {
      await page.locator('#painel .nota').nth(iNota)
        .screenshot({ path: join(SAIDA, `374-rodape-rele-${tema}.png`) });
      diz(`  ✓ 374-rodape-rele-${tema}.png`);
    } else {
      diz('  ⚠ nao achei a nota do relé para capturar');
    }
    const notaRele = await page.evaluate(() => {
      const n = [...document.querySelectorAll('#painel .nota')]
        .find(x => x.textContent.includes('SMTP'));
      return n ? { html: n.innerHTML, texto: n.textContent, topo: n.getBoundingClientRect().top } : null;
    });
    if (verdade(!!notaRele, 'a nota do relé existe na tela do source')) {
      verdade(notaRele.texto.includes('sem TLS'), 'ela diz «sem TLS»');
      verdade(/base64/.test(notaRele.texto), 'ela nomeia o base64 (o servidor manda credencial)');
      verdade(/codificação e não cifra/.test(notaRele.texto),
        'ela diz que base64 é codificação e não cifra');
      verdade(/postfix|exim/.test(notaRele.texto), 'ela dá o conserto realista (relé interno)');
      verdade(/libere o IP/.test(notaRele.texto), 'ela prefere liberar o IP a mandar senha');
      verdade(!notaRele.texto.includes('**') && !notaRele.texto.includes('`'),
        'a marcação virou etiqueta — nenhum ** ou crase sobrou à mostra');
      verdade(!notaRele.texto.includes(SENHA_RELE), 'a senha do relé não vaza para a tela');
      verdade(!notaRele.texto.includes('undefined'), 'nenhum «undefined» no texto da nota');
    }
    /* A linha nova da tabela só de leitura. */
    const linhaUsuario = await page.evaluate(() => {
      const tr = [...document.querySelectorAll('#painel tr')]
        .find(x => x.textContent.includes('alertas.email.usuario'));
      return tr ? tr.textContent.replace(/\s+/g, ' ').trim() : null;
    });
    verdade(!!linhaUsuario && linhaUsuario.includes('phxsql'),
      `a linha alertas.email.usuario aparece com o valor: ${linhaUsuario}`);

    if (tema === 'escuro') {
      diz('  populando o source velho…');
      await entrar(page, velho.url, TOKEN_VELHO);
      await popular(page);
    }

    /* ------------------------------- 374: o mesmo rodapé sem credencial */
    diz('— 374: tela de Configurações da RÉPLICA (relé sem usuário)');
    await entrar(page, rep.url, TOKEN_REP);
    await page.evaluate(() => verConfigServidor());
    await dormir(500);
    const notaSem = await page.evaluate(() => {
      const n = [...document.querySelectorAll('#painel .nota')]
        .find(x => x.textContent.includes('SMTP'));
      return n ? n.textContent : null;
    });
    if (verdade(!!notaSem, 'a nota do relé existe também sem credencial')) {
      verdade(notaSem.includes('sem TLS'), 'ela continua dizendo «sem TLS»');
      verdade(!/base64/.test(notaSem),
        'e NÃO fala de base64 — quem não manda senha não recebe alarme de senha');
    }

    /* ------------------------------ 379: a coluna do fio na tela Replicação */
    diz('— 379: a grade das origens com o estado do fio');
    await page.evaluate(() => verReplicacao());
    await dormir(700);
    await retratar(page, '#gradeOrigensRep', `379-grade-do-fio-${tema}.png`);
    const celulas = await page.evaluate(() =>
      [...document.querySelectorAll('#gradeOrigensRep tbody tr')]
        .map(tr => [...tr.children].map(td => td.textContent.trim())));
    const juntas = JSON.stringify(celulas);
    verdade(/cifrado · com pino/.test(juntas), 'a origem com pino aparece como «cifrado · com pino»');
    verdade(/cifrado · sem pino/.test(juntas), 'a origem sem pino aparece como «cifrado · sem pino»');
    verdade(/em claro/.test(juntas), 'a origem em claro aparece como «em claro»');

    /* ------------------------------------ 379: o assistente, passo a passo */
    diz('— 379: o assistente de replicação');
    await page.click('#btAssistRep');
    await page.waitForSelector('#rzModos');
    await page.click('[data-m="read_replica"]');
    await page.click('#rzIr1');
    await page.waitForSelector('#rzCifra');

    await page.fill('#rzNome', 'matriz-nova');
    await page.fill('#rzHost', '127.0.0.1');
    await page.fill('#rzPorta', String(SRC_DADOS));
    await page.fill('#rzTokenR', TOKEN_SRC);
    // Source COM cadastro de usuários exige login: o token sozinho não abre
    // `bancos` (`erro.faca_login`). O campo diz «opcional» porque há source
    // sem cadastro nenhum — aqui há, e é o caso da vida real.
    await page.fill('#rzUsu', USUARIO);
    await page.fill('#rzSen', SENHA);

    /* (1) de fábrica: cifra ligada, pino vazio */
    let diz1 = await page.textContent('#rzFioDiz');
    verdade(/Sem pino/.test(diz1), 'de fábrica a tela avisa que sem pino não há defesa contra o meio');
    verdade(!diz1.includes('**'), 'o aviso do fio saiu com a marcação já virada etiqueta');
    await retratar(page, '.caixa.larga', `379-passo2-sem-pino-${tema}.png`);
    /* O CSS global já transformou um `input` de marcar numa bolinha do tamanho
     * da célula. Mede-se, não se confia. */
    const caixa = await page.$eval('#rzCifra', el => {
      const r = el.getBoundingClientRect();
      return { w: Math.round(r.width), h: Math.round(r.height) };
    });
    verdade(caixa.w > 8 && caixa.w < 40 && caixa.h > 8 && caixa.h < 40,
      `a caixa de marcar tem tamanho de caixa de marcar: ${caixa.w}×${caixa.h}px`);

    /* (2) pino torto: a tela recusa antes de sair do passo */
    await page.fill('#rzPino', 'abacaxi');
    await page.click('#rzIr2');
    await dormir(200);
    const recado = await page.textContent('#rzRecado');
    verdade(/64 dígitos hexadecimais/.test(recado), 'pino torto é recusado na própria tela');
    verdade(!!(await page.$('#rzCifra')), 'e a tela NÃO avançou de passo');
    await retratar(page, '.caixa.larga', `379-passo2-pino-torto-${tema}.png`);

    /* (3) com o pino de verdade */
    await page.fill('#rzPino', pino);
    await dormir(150);
    const diz3 = await page.textContent('#rzFioDiz');
    verdade(/com pino/.test(diz3), 'com o pino preenchido a tela diz que há âncora');
    // A recusa anterior era sobre um valor que não existe mais: deixá-la ali
    // punha dois vereditos contraditórios na mesma tela.
    verdade(!(await page.textContent('#rzRecado')).trim(),
      'e a recusa do pino torto saiu da tela quando o valor mudou');
    // O pino são 64 dígitos conferidos a olho: o campo tem de mostrá-los.
    // A pergunta certa não é «tem X pixels» (número inventado é número que
    // não se mede): é se o valor CABE. Campo que rola esconde o começo da
    // chave, e conferir chave é o que o pino existe para que se faça.
    const cabePino = await page.$eval('#rzPino', el => ({
      rola: el.scrollWidth > el.clientWidth + 1,
      largura: Math.round(el.getBoundingClientRect().width),
    }));
    verdade(!cabePino.rola,
      `os 64 dígitos cabem sem rolar no campo de ${cabePino.largura}px`);
    verdade(await page.$eval('#rzPino', el => /Mono|mono/.test(getComputedStyle(el).fontFamily)),
      'e em fonte monoespaçada, que é o que faz 1 e l se distinguirem');
    verdade(await page.$eval('#rzPino', el => el.value === el.value.toLowerCase()),
      'o DADO do pino não é maiúsculo na tela: o CSS de rótulo não mordeu o valor');
    await retratar(page, '.caixa.larga', `379-passo2-com-pino-${tema}.png`);

    /* (4) cifra desmarcada: o pino sai de cena e a tela diz o que viaja */
    await page.uncheck('#rzCifra');
    await dormir(150);
    const diz4 = await page.textContent('#rzFioDiz');
    verdade(/em claro/.test(diz4), 'sem cifra a tela diz que a conversa vai em claro');
    verdade(await page.$eval('#rzPino', el => el.disabled),
      'e o campo do pino fica inerte — pino sem túnel não compra nada');
    await retratar(page, '.caixa.larga', `379-passo2-em-claro-${tema}.png`);

    /* (5) a sonda fala o fio da TELA — prova pelo sentido NEGATIVO: o source
     * exige o aperto, então a sonda em claro tem de falhar. Antes desta
     * rodada ela passava, porque caía no padrão do motor e nunca via a
     * escolha da tela — e «ligação boa» para uma configuração que não sobe é
     * pior que não sondar. */
    await page.click('#rzIr2');
    await page.waitForFunction(() => {
      const h = document.querySelector('#rzCorpo h3');
      return h && h.textContent !== 'Testando a conexão';
    }, { timeout: 30000 });
    const tituloClaro = await page.textContent('#rzCorpo h3');
    verdade(tituloClaro === 'Não conectou',
      `a sonda EM CLARO falha contra um source que exige o aperto (título: «${tituloClaro}»)`);
    const corpoClaro = await page.textContent('#rzCorpo');
    verdade(!/anterior a 18\/09\/2026/.test(corpoClaro),
      'e a dica do aperto NÃO aparece quando a cifra está desmarcada');
    await retratar(page, '.caixa.larga', `379-sonda-em-claro-${tema}.png`);

    /* (6) o mesmo caminho com o túnel ligado e o pino: tem de conectar.
     *
     * E este passo é TAMBÉM a prova do terceiro defeito, achado exercitando:
     * a senha NÃO é redigitada aqui de propósito. O campo `#rzSen` nasce sem
     * `value` (credencial não volta para o DOM), e o passo 2 fazia
     * `rz.senha = $("#rzSen").value` sem condição — então voltar de «Corrigir
     * a conexão» apagava a senha certa, e a sonda seguinte falhava com
     * «usuário ou senha inválidos», mandando procurar defeito na credencial.
     * Reponha o `=` sem condição e esta linha volta a falhar. */
    await page.click('#rzVolta');
    await page.waitForSelector('#rzCifra');
    verdade(!(await page.$eval('#rzSen', el => el.value)),
      'o campo da senha volta VAZIO — a credencial não é reescrita no DOM');
    verdade(/deixe em branco para manter/.test(await page.$eval('#rzSen', el => el.placeholder)),
      'e o marcador d\'água diz que a senha digitada continua guardada');
    await page.check('#rzCifra');
    await page.fill('#rzPino', pino);
    await page.click('#rzIr2');
    await page.waitForFunction(() => {
      const h = document.querySelector('#rzCorpo h3');
      return h && h.textContent !== 'Testando a conexão';
    }, { timeout: 30000 });
    const tituloCifra = await page.textContent('#rzCorpo h3');
    verdade(tituloCifra === 'Conectou',
      `a mesma sonda CIFRADA e com pino conecta (título: «${tituloCifra}»)`);
    await retratar(page, '.caixa.larga', `379-sonda-cifrada-${tema}.png`);

    /* (7) até o bloco do config.json */
    await page.click('#rzIr3');
    await page.waitForSelector('#rzIr4');
    await page.click('#rzIr4');
    await page.waitForSelector('#rzIr5');
    await page.click('#rzIr5');
    await page.waitForSelector('#rzAplicar');
    await retratar(page, '.caixa.larga', `379-passo6-resumo-${tema}.png`);
    const resumo = await page.textContent('#rzCorpo');
    verdade(/túnel cifrado, com pino/.test(resumo), 'o resumo do plano diz o estado do fio');
    verdade(!resumo.includes('**'), 'e sem marcação à mostra no resumo');

    await page.click('#rzAplicar');
    await page.waitForSelector('pre.dado', { timeout: 25000 });
    await dormir(300);
    await retratar(page, '.caixa.larga', `379-bloco-gerado-${tema}.png`);
    const bloco = await page.$eval('pre.dado', el => el.textContent);
    verdade(/"cifra":\s*true/.test(bloco), 'o bloco gerado traz "cifra": true ESCRITO');
    verdade(bloco.includes(pino), 'o bloco gerado traz o chave_do_fio que o operador digitou');
    const notaBloco = await page.evaluate(() => {
      const n = [...document.querySelectorAll('#rzCorpo .nota')]
        .find(x => x.textContent.includes('cifra'));
      return n ? n.textContent : null;
    });
    verdade(!!notaBloco && /arranque avisa todo dia/.test(notaBloco),
      'e a nota explica por que o campo escrito cala o aviso de arranque');
    verdade(!!notaBloco && !notaBloco.includes('undefined'),
      'nenhum «undefined» na nota do bloco');

    /* (8) o SOURCE VELHO, que não fala o aperto: a sonda cifrada falha E a
     * tela diz a razão certa; em claro ela passa, e o bloco sai com o
     * `"cifra": false` escrito e SEM pino. */
    await page.evaluate(() => document.querySelector('.sobre').remove());
    await page.evaluate(() => verReplicacao());
    await dormir(400);
    await page.click('#btAssistRep');
    await page.waitForSelector('#rzModos');
    await page.click('[data-m="read_replica"]');
    await page.click('#rzIr1');
    await page.waitForSelector('#rzCifra');
    await page.fill('#rzNome', 'antigo');
    await page.fill('#rzHost', '127.0.0.1');
    await page.fill('#rzPorta', String(VELHO_DADOS));
    await page.fill('#rzTokenR', TOKEN_VELHO);
    await page.fill('#rzUsu', USUARIO);
    await page.fill('#rzSen', SENHA);
    await page.click('#rzIr2');
    await page.waitForFunction(() => {
      const h = document.querySelector('#rzCorpo h3');
      return h && h.textContent !== 'Testando a conexão';
    }, { timeout: 30000 });
    const tituloVelho = await page.textContent('#rzCorpo h3');
    verdade(tituloVelho === 'Não conectou',
      `a sonda cifrada falha contra um source que não fala o aperto (título: «${tituloVelho}»)`);
    const corpoVelho = await page.textContent('#rzCorpo');
    verdade(/anterior a 18\/09\/2026/.test(corpoVelho),
      'e AQUI a tela dá a dica do aperto, porque a cifra está ligada');
    await retratar(page, '.caixa.larga', `379-source-sem-aperto-${tema}.png`);

    await page.click('#rzVolta');
    await page.waitForSelector('#rzCifra');
    await page.uncheck('#rzCifra');
    await page.click('#rzIr2');
    await page.waitForFunction(() => {
      const h = document.querySelector('#rzCorpo h3');
      return h && h.textContent !== 'Testando a conexão';
    }, { timeout: 30000 });
    verdade((await page.textContent('#rzCorpo h3')) === 'Conectou',
      'e em claro a mesma sonda conecta — a escolha da tela atravessa até o fio');
    await page.click('#rzIr3');
    await page.waitForSelector('#rzIr4');
    await page.click('#rzIr4');
    await page.waitForSelector('#rzIr5');
    await page.click('#rzIr5');
    await page.waitForSelector('#rzAplicar');
    const resumoClaro = await page.textContent('#rzCorpo');
    verdade(/em claro/.test(resumoClaro), 'o resumo diz que este plano vai em claro');
    await page.click('#rzAplicar');
    await page.waitForSelector('pre.dado', { timeout: 25000 });
    await dormir(300);
    await retratar(page, '.caixa.larga', `379-bloco-em-claro-${tema}.png`);
    const blocoClaro = await page.$eval('pre.dado', el => el.textContent);
    verdade(/"cifra":\s*false/.test(blocoClaro),
      'o bloco em claro traz "cifra": false ESCRITO — decisão registrada, não omissão');
    verdade(!/chave_do_fio/.test(blocoClaro),
      'e NÃO traz chave_do_fio: pino sem túnel seria campo que não compra nada');
    const notaClaro = await page.evaluate(() => {
      const n = [...document.querySelectorAll('#rzCorpo .nota')]
        .find(x => x.textContent.includes('cifra'));
      return n ? n.textContent : null;
    });
    verdade(!!notaClaro && /nasceria CIFRADA/.test(notaClaro),
      'e a nota explica a outra metade: sem o campo a origem nasceria cifrada');
    verdade(!!notaClaro && !/herdou a virada/.test(notaClaro),
      'sem repetir a explicação do caso contrário, que aqui não se aplica');

    await page.evaluate(() => { const s = document.querySelector('.sobre'); if (s) s.remove(); });

    if (erros.length) {
      diz(`  ⚠ pageerror: ${erros.slice(0, 4).join(' | ')}`);
      falhas.push(`pageerror no tema ${tema}: ${erros[0]}`);
    } else {
      diz('  ✓ nenhum erro de página nesta volta');
    }
    await ctx.close();
  }
} finally {
  await navegador.close();
  await src.derrubar();
  await rep.derrubar();
  await velho.derrubar();
}

diz(`\ncapturas em ${SAIDA}`);
if (falhas.length) {
  diz(`\n${falhas.length} FALHA(S):`);
  for (const f of falhas) diz(`  ✗ ${f}`);
  process.exit(1);
}
diz('\ntudo verde');
