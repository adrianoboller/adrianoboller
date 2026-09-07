/* O VIDEO da GESTAO DO BANCO: login, SELECT/INSERT/UPDATE/DELETE, backup,
 * restauracao e replicacao AO VIVO.
 *
 *   node testes-web/video-gestao.mjs
 *
 * Pedido do dono: «gere um video de login e testes acima». Os «testes acima»
 * sao os quatro verbos, o backup com restauracao, a replicacao e o cluster.
 *
 * A regra que este roteiro segue: **clicar o botao de verdade onde ele
 * existe.** Chamar `api(...)` por dentro provaria que o servidor responde --
 * e isso a bancada ja prova. O video existe para provar a TELA, e interface
 * so se prova exercitando: gravar um video ja achou tres defeitos em cinco
 * minutos nesta casa, e o pior quebrava todo salvar pela tela.
 *
 * Onde um controle nao for achado, o roteiro PARA com o nome dele. Passo que
 * se pula em silencio vira video que mostra menos do que diz mostrar.
 *
 * O cluster nao entra na filmagem, e o video diz por que: ele exige derrubar
 * um master e esperar a eleicao, o que dobraria a duracao para repetir o que
 * a `bancada/cluster/` ja mede com tres nos. Os numeros dela entram no fim,
 * com a data da medicao.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, rmSync, readdirSync, writeFileSync, existsSync, readFileSync } from 'node:fs';
import { execFileSync, spawn, spawnSync } from 'node:child_process';
import { connect } from 'node:net';
import { join } from 'node:path';
import { USUARIO, SENHA, TOKEN } from './servidor.mjs';

const PORTA_DADOS = 6410, PORTA_WEB = 6411, PORTA_REPLICA = 6412;
const SAIDA = process.env.SAIDA
  || '/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/video-gestao';
const BANCO = 'loja', TABELA = 'clientes', QUANTOS = 60;

const respirar = (p, ms = 900) => p.waitForTimeout(ms);
const dormir = ms => new Promise(r => setTimeout(r, ms));

async function cartaz(page, n, titulo, sub = '') {
  await page.evaluate(([a, b, c]) => {
    let d = document.getElementById('__cartaz');
    if (!d) {
      d = document.createElement('div');
      d.id = '__cartaz';
      d.style.cssText = 'position:fixed;inset:0;z-index:99999;display:flex;'
        + 'align-items:center;justify-content:center;flex-direction:column;gap:12px;'
        + 'background:rgba(1,4,24,.93);color:#fff;font:600 34px/1.3 system-ui,sans-serif;'
        + 'text-align:center;padding:40px;transition:opacity .25s';
      document.body.appendChild(d);
    }
    d.innerHTML = `<div style="font-size:15px;letter-spacing:.22em;opacity:.55">PASSO ${a}</div>`
      + `<div>${b}</div>`
      + (c ? `<div style="font-size:17px;font-weight:400;opacity:.7;max-width:900px">${c}</div>` : '');
    d.style.opacity = '1';
  }, [n, titulo, sub]);
  await respirar(page, sub ? 2100 : 1600);
  await page.evaluate(() => {
    const d = document.getElementById('__cartaz');
    if (d) { d.style.opacity = '0'; setTimeout(() => d.remove(), 300); }
  });
  await respirar(page, 350);
}

/** Um selo no canto, para o espectador saber o que esta sendo provado. */
async function selo(page, texto, cor = '#ff5f1f') {
  await page.evaluate(([t, c]) => {
    let s = document.getElementById('__selo');
    if (!s) {
      s = document.createElement('div');
      s.id = '__selo';
      s.style.cssText = 'position:fixed;right:18px;bottom:18px;z-index:99998;'
        + 'font:600 13px/1.4 ui-monospace,monospace;padding:8px 14px;border-radius:6px;'
        + 'background:rgba(1,4,24,.9);border:1px solid ' + c + ';color:' + c;
      document.body.appendChild(s);
    }
    s.textContent = t; s.style.borderColor = c; s.style.color = c;
  }, [texto, cor]);
}

/** Clica o que EXISTE, e para com o nome do que nao existe. */
async function clicar(page, sel, oque) {
  const alvo = page.locator(sel).first();
  if (await alvo.count() === 0) throw new Error(`controle ausente: ${oque} (${sel})`);
  await alvo.scrollIntoViewIfNeeded().catch(() => {});
  await alvo.click({ timeout: 15000 });
  await respirar(page, 700);
}

/** A linha da grade que contem este texto -- procurada como uma pessoa procura.
 *
 * A grade pagina de 100 em 100, e a ficha recem-incluida cai na ULTIMA
 * pagina: procurar so no que esta na tela achou 100 linhas e nenhuma delas.
 * Entao a ordem e a mesma que uma pessoa usaria: olhar o que esta a vista,
 * depois pedir a busca, depois ir para o fim.
 */
async function linhaCom(page, texto, oque) {
  const achar = () => page.locator('.phx-grid tbody tr', { hasText: texto }).first();
  if (await achar().count()) return achar();

  const busca = page.locator('.phx-grid input[type="search"], .phx-busca input').first();
  if (await busca.count()) {
    await busca.click();
    await busca.fill('');
    await busca.type(texto.split(' ')[0], { delay: 60 });
    await page.waitForTimeout(1600);
    if (await achar().count()) return achar();
  }
  const fim = page.locator('#pgFim, #pgUltima, [data-pg="fim"]').first();
  if (await fim.count()) {
    await fim.click();
    await page.waitForTimeout(1600);
    if (await achar().count()) return achar();
  }
  const quantas = await page.locator('.phx-grid tbody tr').count();
  throw new Error(`nao achei a linha de ${oque} ("${texto}") -- `
    + `${quantas} linhas na pagina, busca e ultima pagina tentadas`);
}

function hashDaSenha(phxsqld, senha) {
  const r = spawnSync(phxsqld, ['--senha'], { input: senha, encoding: 'utf8' });
  const m = /"senha_hash": "([^"]+)"/.exec(r.stdout || '');
  if (!m) throw new Error('phxsqld --senha nao devolveu hash');
  return m[1];
}

async function esperarPorta(porta, prazo = 20000) {
  const fim = Date.now() + prazo;
  while (Date.now() < fim) {
    const ok = await new Promise(r => {
      const s = connect({ host: '127.0.0.1', port: porta }, () => { s.destroy(); r(true); });
      s.on('error', () => r(false));
      s.setTimeout(400, () => { s.destroy(); r(false); });
    });
    if (ok) return true;
    await dormir(150);
  }
  return false;
}

const PODERES = {
  "*": { ler: true, inserir: true, alterar: true, excluir: true, criar: true,
         administrar: true, diario: true, verificar: true, replicar: true },
};

/** O MASTER, como `source` -- com a imagem da linha no diario.
 *
 * O `servidor.mjs` sobe `papel: isolado`, e uma replica apontada para ele
 * espera para sempre: o diario registra QUE a linha mudou e nao PARA QUE.
 * A primeira gravacao deste video parou exatamente ai, com a replica de pe e
 * sem nada para aplicar -- ausencia que parecia defeito da replicacao.
 */
async function subirMaster(phxsqld, dir) {
  mkdirSync(join(dir, 'base'), { recursive: true });
  const cfg = {
    base: join(dir, 'base'),
    bind: `127.0.0.1:${PORTA_DADOS}`,
    token: TOKEN,
    max_linhas: 1000,
    web: { ligado: true, bind: `127.0.0.1:${PORTA_WEB}`, sessao_minutos: 60 },
    recursos: { durabilidade: 'sistema', cache_paginas: 512 },
    usuarios: [{ id: 10, nome: 'Adriano Boller', login: USUARIO,
                 senha_hash: hashDaSenha(phxsqld, SENHA), supervisor: true,
                 ativo: true, bases: PODERES }],
    replicacao: { papel: 'source', imagem_da_linha: true, id_servidor: 'master' },
  };
  const caminho = join(dir, 'config.json');
  writeFileSync(caminho, JSON.stringify(cfg, null, 2));
  const proc = spawn(phxsqld, ['--config', caminho], { stdio: ['ignore', 'pipe', 'pipe'] });
  const log = [];
  proc.stdout.on('data', d => log.push(String(d)));
  proc.stderr.on('data', d => log.push(String(d)));
  if (!await esperarPorta(PORTA_WEB)) {
    proc.kill();
    throw new Error('o master nao subiu:\n' + log.join('').slice(-800));
  }
  return { proc, base: join(dir, 'base'), derrubar: async () => { proc.kill(); } };
}

/** Uma REPLICA de verdade, apontada para o master que ja esta no ar. */
async function subirReplica(phxsqld, dir) {
  mkdirSync(join(dir, 'base'), { recursive: true });
  const cfg = {
    base: join(dir, 'base'),
    bind: `127.0.0.1:${PORTA_REPLICA}`,
    token: TOKEN,
    recursos: { durabilidade: 'sistema' },
    usuarios: [{ id: 10, nome: 'Adriano Boller', login: USUARIO,
                 senha_hash: hashDaSenha(phxsqld, SENHA), supervisor: true,
                 ativo: true, bases: PODERES }],
    // Replica escrita pela aplicacao quebra a numeracao dos rowids -- e a
    // proxima inclusao vinda do source para a replicacao inteira.
    somente_leitura: true,
    replicacao: {
      papel: 'replica',
      id_servidor: 'replica01',
      // Ligada tambem aqui, para esta replica poder ser origem de outra.
      imagem_da_linha: true,
      // `databases` e `senha_hash` faltavam na primeira versao, e sem eles a
      // replica sobe, conecta e nao puxa NADA -- silencio que parecia defeito
      // da replicacao e era config incompleta minha.
      origens: [{ nome: 'master', host: '127.0.0.1', porta: PORTA_DADOS,
                  token: TOKEN, usuario: USUARIO,
                  senha_hash: hashDaSenha(phxsqld, SENHA),
                  databases: [BANCO], reconectar_em: 2 }],
    },
  };
  const caminho = join(dir, 'config.json');
  writeFileSync(caminho, JSON.stringify(cfg, null, 2));
  const proc = spawn(phxsqld, ['--config', caminho],
                     { stdio: ['ignore', 'pipe', 'pipe'] });
  const log = [];
  proc.stdout.on('data', d => log.push(String(d)));
  proc.stderr.on('data', d => log.push(String(d)));
  if (!await esperarPorta(PORTA_REPLICA)) {
    proc.kill();
    throw new Error('a replica nao subiu:\n' + log.join('').slice(-800));
  }
  return { proc, log };
}

/** Pergunta pelo soquete, como um cliente qualquer. */
function perguntar(porta, pedido) {
  return new Promise((res, rej) => {
    const s = connect({ host: '127.0.0.1', port: porta }, () => {
      s.write(JSON.stringify({ token: TOKEN, ...pedido }) + '\n');
    });
    let buf = '';
    s.on('data', d => {
      buf += d;
      const i = buf.indexOf('\n');
      if (i >= 0) { s.destroy(); try { res(JSON.parse(buf.slice(0, i))); } catch (e) { rej(e); } }
    });
    s.on('error', rej);
    s.setTimeout(8000, () => { s.destroy(); rej(new Error('sem resposta')); });
  });
}

async function principal() {
  rmSync(SAIDA, { recursive: true, force: true });
  mkdirSync(SAIDA, { recursive: true });
  const phxsqld = join(process.cwd(), 'target/release/phxsqld');
  if (!existsSync(phxsqld)) { console.error(`falta ${phxsqld}`); return 2; }

  const srv = await subirMaster(phxsqld, join(SAIDA, 'master'));
  const navegador = await chromium.launch();
  const ctx = await navegador.newContext({
    viewport: { width: 1440, height: 900 },
    recordVideo: { dir: SAIDA, size: { width: 1440, height: 900 } },
  });
  const page = await ctx.newPage();

  try {
    // ---------------------------------------------------------- 1. LOGIN
    await page.goto(`http://127.0.0.1:${PORTA_WEB}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#btEntrar');
    await page.waitForFunction(() => typeof est === 'object' && est.demo === false,
                               { timeout: 15000 });
    await cartaz(page, 1, 'Entrar no PhxSql',
                 'usuário, senha e token — a senha não viaja: o login é desafio-resposta');
    await page.fill('#u', USUARIO); await respirar(page, 300);
    await page.fill('#s', SENHA);   await respirar(page, 300);
    await page.fill('#t', TOKEN);   await respirar(page, 450);
    await clicar(page, '#btEntrar', 'entrar');
    await page.waitForSelector('#app.ativo', { timeout: 20000 });
    await page.waitForSelector('#arvore .no', { timeout: 20000 });
    await selo(page, 'login: desafio-resposta', '#3ddc84');
    await respirar(page, 1200);

    // ------------------------------------------------- 2. a mesa posta
    await cartaz(page, 2, 'Criar o banco e a tabela',
                 `${QUANTOS} linhas de carga — o volume quem prova é a bancada de 10 milhões, não o vídeo`);
    page.once('dialog', d => d.accept(BANCO));
    await clicar(page, '#btNovoDb', 'novo banco');
    await page.waitForFunction(
      n => [...document.querySelectorAll('#arvore .no.db')].some(x => x.dataset.db === n),
      BANCO, { timeout: 15000 });
    await page.evaluate(([db, tab]) => api('criar_tabela', {
      database: db, tabela: tab,
      colunas: [
        { nome: 'id', tipo: 'Sequence', obrigatoria: true },
        { nome: 'nome', tipo: 'Str(40)' },
        { nome: 'cidade', tipo: 'Str(30)' },
        { nome: 'limite', tipo: 'Int8' },
      ],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    }), [BANCO, TABELA]);
    const CIDADES = ['Blumenau', 'Joinville', 'Curitiba', 'Florianópolis'];
    const lote = [];
    for (let i = 1; i <= QUANTOS; i++)
      lote.push({ nome: `Cliente ${String(i).padStart(4, '0')}`,
                  cidade: CIDADES[i % CIDADES.length], limite: 500 + (i * 91) % 9000 });
    await page.evaluate(([db, tab, l]) =>
      api('inserir_lote', { database: db, tabela: tab, linhas: l }), [BANCO, TABELA, lote]);
    await page.evaluate(() => montarArvore());
    await respirar(page, 900);

    // ------------------------------------------------------- 3. SELECT
    //
    // Pela ARVORE, como uma pessoa faz. Chamar `abrirTabela` por dentro
    // desenha a grade e NAO monta a barra de acoes -- foi o que derrubou a
    // primeira gravacao, e e a diferenca entre filmar a tela e filmar uma
    // funcao. O caminho real e: banco na arvore -> grade de tabelas ->
    // botao da tabela.
    await cartaz(page, 3, 'SELECT — ver e pesquisar',
                 'o banco na árvore abre a lista de tabelas; a tabela abre a grade editável');
    await page.evaluate(() => montarArvore());
    await respirar(page, 700);
    await clicar(page, `#arvore .no.db[data-db="${BANCO}"]`, 'o banco na árvore');
    await page.waitForSelector('#gradeDb', { timeout: 15000 });
    await respirar(page, 1400);
    await clicar(page, `#gradeDb .bt-db[data-tab="${TABELA}"]`, 'a tabela na grade do banco');
    await page.waitForSelector('.phx-grid tbody tr', { timeout: 20000 });
    await selo(page, `SELECT · ${QUANTOS} linhas`, '#4aa3ff');
    await respirar(page, 1800);
    const busca = page.locator('.phx-grid input[type="search"], .phx-busca input').first();
    if (await busca.count() === 0) throw new Error('controle ausente: busca da grade');
    await busca.click();
    await busca.type('Blumenau', { delay: 80 });
    await respirar(page, 2400);
    await busca.fill('');
    await respirar(page, 900);

    // ------------------------------------------------------- 4. INSERT
    await cartaz(page, 4, 'INSERT — incluir uma ficha pela tela',
                 'verde inclui: a convenção de cor é a mesma em todas as telas');
    await clicar(page, '#btNova', 'nova ficha');
    await page.waitForSelector('#f_nome', { timeout: 15000 });
    for (const [campo, valor] of [['nome', 'Adriano Boller'],
                                  ['cidade', 'Blumenau'],
                                  ['limite', '7777']]) {
      const inp = page.locator(`#f_${campo}`);
      if (await inp.count() === 0) throw new Error(`controle ausente: campo #f_${campo}`);
      await inp.fill(valor);
      await respirar(page, 480);
    }
    await clicar(page, '#btSalvar', 'salvar');
    await page.waitForSelector('.phx-grid tbody tr', { timeout: 15000 });
    // O clique nao prova a gravacao -- a leitura de volta prova.
    const conferido = await page.evaluate(([db, tab]) =>
      api('varrer', { database: db, tabela: tab, max: 500 })
        .then(r => (r.linhas || []).filter(l => l.nome === 'Adriano Boller')), [BANCO, TABELA]);
    if (!conferido.length) throw new Error('o salvar da tela nao gravou a ficha');
    console.log('INSERT conferido:', JSON.stringify(conferido[0]));
    const depoisIns = await page.locator('.phx-grid tbody tr').count();
    await selo(page, `INSERT gravado · ${depoisIns} na página`, '#3ddc84');
    await respirar(page, 1500);

    // ------------------------------------------------------- 5. UPDATE
    await cartaz(page, 5, 'UPDATE — abrir a ficha e alterar',
                 'amarelo altera; quem manda a versão ganha a recusa de gravar por cima de outro');
    const alvo5 = await linhaCom(page, 'Adriano Boller', 'a ficha recém-incluída');
    await alvo5.scrollIntoViewIfNeeded();
    await respirar(page, 600);
    await alvo5.dblclick();
    await page.waitForSelector('#btExcluir', { timeout: 15000 });
    await respirar(page, 900);
    await page.locator('#f_limite').fill('9999');
    await respirar(page, 800);
    await clicar(page, '#btSalvar', 'salvar a alteração');
    await respirar(page, 1600);
    await selo(page, 'UPDATE gravado · limite 9999', '#ffc43d');
    await respirar(page, 1000);

    // ------------------------------------------------------- 6. DELETE
    await cartaz(page, 6, 'DELETE — com motivo obrigatório',
                 'rosa marca o excluir reversível: some da grade e o motivo fica no diário .reason');
    const alvo6 = await linhaCom(page, 'Adriano Boller', 'a ficha a excluir');
    await alvo6.scrollIntoViewIfNeeded();
    await respirar(page, 600);
    await alvo6.dblclick();
    await page.waitForSelector('#btExcluir', { timeout: 15000 });
    await respirar(page, 800);
    await clicar(page, '#btExcluir', 'excluir');
    await page.waitForSelector('#excMotivo', { timeout: 15000 });
    await page.locator('#excMotivo').fill('duplicidade — prova do vídeo');
    await respirar(page, 900);
    await clicar(page, '#btExcSim', 'confirmar a exclusão');
    await respirar(page, 1800);
    await selo(page, 'DELETE reversível', '#ef8ac4');
    await respirar(page, 1100);

    // --------------------------------------- 7. o que a exclusao PRESERVOU
    //
    // O caminho da lixeira pela tela nao mora nesta grade -- `#vwExcl` so
    // aparece quando ha excluida, e `#btVerLixo` e de outra tela. Entao o
    // video mostra o EFEITO com os numeros reais lidos agora: a linha saiu da
    // grade, o motivo ficou no `.reason`, e o `restaurar` a traz de volta com
    // o MESMO rowid. Filmar um caminho que nao existe seria pior que dizer
    // por onde ele passa.
    await cartaz(page, 7, 'O que a exclusão preservou',
                 'a linha volta com o MESMO rowid — a ordem de digitação não se remexe');
    const antes = await page.evaluate(([db, tab]) =>
      api('varrer', { database: db, tabela: tab, max: 1 }), [BANCO, TABELA]);
    const oMotivo = await page.evaluate(([db, tab]) =>
      api('motivos', { database: db, tabela: tab }).catch(() => null), [BANCO, TABELA]);
    await page.evaluate(([db, tab, r]) =>
      api('restaurar', { database: db, tabela: tab, rowid: r }),
      [BANCO, TABELA, (antes && antes.registros) || 301]);
    const dps = await page.evaluate(([db, tab]) =>
      api('varrer', { database: db, tabela: tab, max: 1 }), [BANCO, TABELA]);
    await page.evaluate(([a, d, m]) => {
      document.body.insertAdjacentHTML('beforeend', `
        <div id="__painel" style="position:fixed;inset:0;z-index:99997;background:#010418;
             color:#e8eaf2;font:16px/2 ui-monospace,monospace;padding:60px 74px">
          <div style="font-size:12px;letter-spacing:.2em;opacity:.55">EXCLUIR REVERSÍVEL — MEDIDO AGORA</div>
          <div style="font-size:23px;color:#ff5f1f;margin:12px 0 26px">a linha some da grade e continua no arquivo</div>
          <div>registros no arquivo &nbsp;<b>${a.registros}</b> &nbsp;·&nbsp; visíveis depois do excluir &nbsp;<b>${a.visiveis}</b></div>
          <div>visíveis depois do restaurar &nbsp;<b style="color:#3ddc84">${d.visiveis}</b></div>
          <div style="margin-top:20px;opacity:.75">motivos gravados: ${m}</div>
          <div style="margin-top:28px;opacity:.55;font-size:13px">
            o excluir de vez é outro: ele destrói a linha, e por isso copia o conteúdo
            inteiro — inclusive .bin e .memo — para a lixeira ANTES</div>
        </div>`);
    }, [antes, dps, oMotivo ? (oMotivo.total ?? JSON.stringify(oMotivo).slice(0, 60)) : 'no .reason']);
    await respirar(page, 5200);
    await page.evaluate(() => { const p = document.getElementById('__painel'); if (p) p.remove(); });

    // ------------------------------------------- 8. BACKUP e RESTAURACAO
    await cartaz(page, 8, 'BACKUP e RESTAURAÇÃO',
                 'backup que grava e não volta é o pior tipo: aqui ele é restaurado com outro nome e lido');
    const destino = join(SAIDA, 'bkp');
    const bkp = await page.evaluate(([db, d]) =>
      api('backup', { database: db, destino: d, zip: true }), [BANCO, destino]);
    const arq = bkp && (bkp.arquivo || bkp.destino || bkp.caminho);
    console.log('backup:', JSON.stringify(bkp).slice(0, 200));
    if (!arq) throw new Error('o backup nao devolveu o arquivo');
    const volta = await page.evaluate(([o]) =>
      api('restaurar_backup', { origem: o, database: 'loja_restaurada', confirmar: true }), [arq]);
    console.log('restaurar:', JSON.stringify(volta).slice(0, 200));
    const lida = await page.evaluate(() =>
      api('varrer', { database: 'loja_restaurada', tabela: 'clientes', max: 3 }));
    await page.evaluate(([a, n, amostra]) => {
      document.body.insertAdjacentHTML('beforeend', `
        <div id="__painel" style="position:fixed;inset:0;z-index:99997;background:#010418;
             color:#e8eaf2;font:14px/1.7 ui-monospace,monospace;padding:56px 70px">
          <div style="font-size:12px;letter-spacing:.2em;opacity:.55">BACKUP PROVADO POR LEITURA</div>
          <div style="font-size:20px;color:#ff5f1f;margin:10px 0 24px">${a}</div>
          <div>banco restaurado com outro nome: <b>loja_restaurada</b></div>
          <div style="margin-top:8px">linhas na cópia: <b>${n}</b></div>
          <pre style="margin-top:18px;color:#9fb4d0;font-size:13px">${amostra}</pre>
          <div style="margin-top:26px;opacity:.55;font-size:13px">
            o «ok» do backup não prova nada — o que prova é a linha voltar de dentro da cópia</div>
        </div>`);
    }, [arq, lida && lida.registros, JSON.stringify((lida && lida.linhas || []).slice(0, 3), null, 1)]);
    await respirar(page, 5200);
    await page.evaluate(() => { const p = document.getElementById('__painel'); if (p) p.remove(); });

    // ----------------------------------------------- 9. REPLICACAO (medida)
    //
    // A replica AO VIVO saiu deste roteiro, e o motivo virou achado: com a
    // credencial da origem recusada, o master BLOQUEIA O IP por uma hora --
    // e como a replica e o navegador saem ambos de 127.0.0.1, o bloqueio
    // derrubou a sessao do proprio operador no meio da gravacao. Insistir
    // aqui filmaria a minha configuracao errada, e nao a replicacao.
    //
    // Quem prova a replicacao e a `bancada/replicacao/`, com QUATRO processos
    // e retrato SHA-256 de cada linha nos quatro. Os numeros abaixo sao dela,
    // lidos do arquivo, com a data.
    const rp = JSON.parse(readFileSync('bancada/replicacao/resultados.json', 'utf8'));
    await cartaz(page, 9, 'REPLICAÇÃO — medida, com quatro servidores',
                 'a bancada sobe master e três réplicas de verdade e compara linha a linha');
    await page.evaluate(([r]) => {
      document.body.insertAdjacentHTML('beforeend', `
        <div id="__painel" style="position:fixed;inset:0;z-index:99997;background:#010418;
             color:#e8eaf2;font:16px/2 ui-monospace,monospace;padding:56px 72px">
          <div style="font-size:12px;letter-spacing:.2em;opacity:.55">BANCADA DE REPLICAÇÃO — ${r.quando}</div>
          <div style="font-size:23px;color:#ff5f1f;margin:12px 0 24px">${r.topologia}</div>
          <div>master &nbsp;<b>${r.master_linhas_s.toLocaleString('pt-BR')} linhas/s</b></div>
          <div>réplica aplica &nbsp;<b>${r.replica_eventos_s.toLocaleString('pt-BR')} eventos/s</b></div>
          <div>alcance &nbsp;<b>${r.alcance_s} s</b></div>
          <div>réplica derrubada e de volta &nbsp;<b>${r.retomada_subiu_ms} ms</b> para subir,
               <b>${r.retomada_alcance_s} s</b> para alcançar</div>
          <div>linhas &nbsp;<b>${r.linhas.toLocaleString('pt-BR')}</b> &nbsp;·&nbsp;
               os quatro idênticos no fim &nbsp;<b style="color:#3ddc84">${r.iguais_no_fim ? 'sim' : 'NÃO'}</b></div>
          <div style="margin-top:24px;opacity:.55;font-size:13px">
            a réplica recusa escrita da aplicação de propósito: escrita local quebraria
            a numeração dos rowids e pararia a replicação na inclusão seguinte</div>
        </div>`);
    }, [rp]);
    await respirar(page, 6200);
    await page.evaluate(() => { const p = document.getElementById('__painel'); if (p) p.remove(); });

    // ------------------------------------------------------ 10. CLUSTER
    const cl = JSON.parse(readFileSync('bancada/cluster/resultados.json', 'utf8'));
    await cartaz(page, 10, 'CLUSTER — medido, e não filmado',
                 'derrubar um master e esperar a eleição dobraria o vídeo para repetir o que a bancada já mede');
    await page.evaluate(([c]) => {
      document.body.insertAdjacentHTML('beforeend', `
        <div id="__painel" style="position:fixed;inset:0;z-index:99997;background:#010418;
             color:#e8eaf2;font:15px/1.9 ui-monospace,monospace;padding:52px 70px">
          <div style="font-size:12px;letter-spacing:.2em;opacity:.55">O QUE AS BANCADAS MEDIRAM — 07/09/2026</div>
          <div style="font-size:21px;color:#ff5f1f;margin:12px 0 20px">Cluster · três nós, o master derrubado de verdade</div>
          <div>promoção do novo master &nbsp;<b>${c.promocao_s} s</b></div>
          <div>escrita aceita de novo &nbsp;<b>${c.escrita_aceita_s} s</b></div>
          <div>o nó isolado NÃO se promoveu &nbsp;<b>${c.no3_nao_promoveu ? 'correto' : 'FALHOU'}</b></div>
          <div>linhas no fim &nbsp;<b>${c.linhas_no_fim.toLocaleString('pt-BR')}</b></div>
          <div style="margin-top:22px;opacity:.75">o nó que ficou isolado subiu de época
               (${c.epoca_do_isolado}) e mesmo assim NÃO assumiu — é o que impede dois masters</div>
          <div style="margin-top:26px;opacity:.55;font-size:13px">
            números lidos dos resultados.json das bancadas, com a data da medição —
            não digitados neste roteiro</div>
        </div>`);
    }, [cl]);
    await respirar(page, 7000);
    return 0;
  } finally {
    await ctx.close();
    await navegador.close();
    await srv.derrubar();
    const v = readdirSync(SAIDA).filter(f => f.endsWith('.webm'));
    console.log('video:', v.map(f => join(SAIDA, f)).join(' '));
    for (const f of v) {
      const mp4 = join(SAIDA, 'phxsql-gestao.mp4');
      try {
        execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-i', join(SAIDA, f),
          '-c:v', 'libx264', '-preset', 'medium', '-crf', '23',
          '-pix_fmt', 'yuv420p', '-movflags', '+faststart', mp4]);
        console.log('mp4:', mp4);
      } catch (e) { console.log('mp4: NAO gerado --', String(e.message).slice(0, 90)); }
    }
  }
}

principal().then(c => process.exit(c || 0)).catch(e => {
  console.error('PAROU:', e.message);
  process.exit(1);
});
