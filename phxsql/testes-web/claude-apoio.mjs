/* O que todo caso da bateria da Claude usa: ligar a integracao pela MESMA
 * gaveta que a tela le, abrir as telas envolvidas, e esperar o EFEITO (nunca
 * o relogio -- a licao da SS9 de `docs/CLAUDE-IA.md`: "relogio fixo le texto
 * cru como se fosse tela pronta").
 *
 * Reusa `api()`, `entrar()` e `abrirPeloMenu()` de `apoio.mjs` -- nao ha um
 * segundo caminho de login nem uma segunda `api()` aqui. */
import { abrirPeloMenu } from './apoio.mjs';

/** Le/escreve a MESMA gaveta que `claude.js` usa (`phxsql.ia`), pelo
 *  `localStorage` do navegador -- e' o unico lugar onde a chave mora, e nao
 *  ha campo na tela para editar o `endpoint` (so' um `<code>` que MOSTRA
 *  ele). Escrever aqui e' o caminho que a propria tela usaria se tivesse um
 *  formulario para isso -- nunca um atalho por dentro do modulo. */
export async function definirIA(page, parcial) {
  await page.evaluate(p => {
    const atual = JSON.parse(localStorage.getItem('phxsql.ia') || '{}');
    localStorage.setItem('phxsql.ia', JSON.stringify(Object.assign(atual, p)));
  }, parcial);
}

export async function lerIA(page) {
  return page.evaluate(() => JSON.parse(localStorage.getItem('phxsql.ia') || '{}'));
}

export async function abrirConfigClaude(page) {
  await abrirPeloMenu(page, 'tela.mi_claude');
  await page.waitForSelector('#iaSalvar', { timeout: 10000 });
}

export async function abrirQuery(page) {
  await abrirPeloMenu(page, 'tela.mi_consulta_sql');
  await page.waitForSelector('#btConsultar', { timeout: 10000 });
}

/** Testa a chave pela tela de Configuracoes e devolve o veredito -- espera o
 *  RECADO ficar `bom` ou `mal` (nunca o estado transitorio "testando…"). */
export async function testarChave(page, { timeout = 15000 } = {}) {
  await page.click('#iaTestar');
  // O `undefined` do meio e' o `arg` da assinatura `waitForFunction(fn, arg,
  // options)` -- sem ele, `{timeout}` vira o ARGUMENTO da funcao de pagina, e
  // o timeout de verdade cai no padrao do Playwright (30 s). Achado medido
  // nesta rodada: uma prova dupla que devia estourar em 6 s estourava em 30.
  await page.waitForFunction(() => {
    const el = document.querySelector('#iaRecado .aviso');
    return !!el && (el.classList.contains('bom') || el.classList.contains('mal'));
  }, undefined, { timeout });
  return page.evaluate(() => {
    const el = document.querySelector('#iaRecado .aviso');
    return { ok: el.classList.contains('bom'), texto: el.textContent.trim() };
  });
}

export async function abrirPainelIA(page) {
  await page.click('#btIA');
  await page.waitForSelector('#iaPainel .ia-rec', { timeout: 10000 });
}

export async function escolherReceita(page, chave) {
  await page.click(`.ia-rec[data-r="${chave}"]`);
  await page.waitForSelector('#iaPergunta', { timeout: 10000 });
}

export async function definirDb(page, db) {
  await page.fill('#iaDb', db);
}

/** "Ver o que vai subir" e devolve o corpo/cabecalhos mostrados no painel,
 *  lidos do PROPRIO DOM (e nao reconstruidos aqui) -- e' o que a pessoa ve. */
export async function verEnvio(page, { timeout = 15000 } = {}) {
  await page.click('#iaVer');
  await page.waitForSelector('#iaEnvio details.nota', { timeout });
  return page.evaluate(() => {
    const pres = [...document.querySelectorAll('#iaEnvio pre.dado')];
    const [cab, corpo] = pres.map(p => p.textContent);
    const resumo = document.querySelector('#iaEnvio summary').textContent;
    return { resumo, cabecalhos: cab, corpo };
  });
}

/** Preenche a pergunta, clica Perguntar, e espera o EFEITO: ou a resposta
 *  terminou (o `#iaTokens` ganha `<b>`, so' na mensagem FINAL de `ir()` --
 *  as atualizacoes por pedaco usam texto plano), ou deu erro. Nunca conta
 *  segundo: e' a mesma licao do soquete, por outro caminho. */
export async function perguntarEEsperar(page, texto, { timeout = 20000 } = {}) {
  await page.fill('#iaPergunta', texto);
  await page.click('#iaIr');
  await page.waitForFunction(() => {
    const saida = document.querySelector('#iaSaida');
    if (saida && saida.querySelector('.aviso.mal')) return true;
    const tok = document.querySelector('#iaTokens');
    return !!(tok && tok.querySelector('b'));
  }, undefined, { timeout });
  return page.evaluate(() => {
    const saida = document.querySelector('#iaSaida');
    const erro = saida.querySelector('.aviso.mal');
    const iaTexto = document.querySelector('#iaTexto');
    const iaSql = document.querySelector('#iaSql');
    return {
      erro: erro ? erro.textContent.trim() : null,
      texto: iaTexto ? iaTexto.textContent : null,
      sql: iaSql ? iaSql.value : null,
      tokens: document.querySelector('#iaTokens').textContent.trim(),
    };
  });
}

/** Mede o CRESCIMENTO do texto que chega por streaming, por EFEITO (o
 *  tamanho do texto na tela), nao pelo relogio: junta os tamanhos distintos
 *  vistos ao longo de uma janela curta. Chame logo depois de clicar
 *  Perguntar -- concorrente com `perguntarEEsperar`, entao rode-o ANTES e
 *  deixe o chamador esperar a resposta terminar depois. */
export async function medirCrescimento(page, { janelaMs = 1500, passoMs = 20 } = {}) {
  const vistos = [];
  const fim = Date.now() + janelaMs;
  let ultimo = -1;
  while (Date.now() < fim) {
    const len = await page.evaluate(() => (document.querySelector('#iaTexto')?.textContent || '').length)
      .catch(() => -1);
    if (len !== ultimo) { vistos.push(len); ultimo = len; }
    await page.waitForTimeout(passoMs);
  }
  return vistos;
}

/** Um cenario minimo: um database com uma tabela que tem coluna de dado
 *  pessoal -- o que a redacao da SS4 do documento precisa para ter algo para
 *  redigir. Pelas OPERACOES do protocolo, nao pela tela -- o que se PROVA
 *  pelo clique e' sempre o comportamento da Claude, nao a criacao do
 *  cenario. */
export async function cenarioParaIA(page, api, db, tab = 'clientes') {
  await api(page, 'criar_database', { database: db }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: db, tabela: tab,
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true },
      { nome: 'nome', tipo: 'Str(40)', obrigatoria: true },
      { nome: 'email', tipo: 'Str(60)', dado_pessoal: 'pessoal' },
      { nome: 'cidade', tipo: 'Str(30)' },
    ],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  });
  for (const l of [[1, 'Adriano Boller', 'adriano@exemplo.org', 'Blumenau'],
    [2, 'Maria Souza', 'maria@exemplo.org', 'Joinville']]) {
    await api(page, 'inserir', { database: db, tabela: tab, valores: l });
  }
  return { db, tab };
}
