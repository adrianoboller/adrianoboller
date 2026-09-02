/* O que todo caso usa: entrar pela tela, afirmar, e guardar a captura.
 *
 * Nada de framework. A regra de zero dependencias vale aqui tambem -- so o
 * Playwright, que e o navegador, e a `std` do Node. */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';

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
 * a entrada, que e por onde todo mundo passa. */
export async function entrar(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  // Sem servidor a pagina cai em «modo demonstracao» com dados embutidos, e
  // a bateria passaria inteira sem tocar no motor. Este e o teste que impede
  // a bateria de se enganar sozinha.
  // `est` e um `const` de topo de script: existe no escopo global lexico, e
  // por isso se le pelo NOME e nao por `window.est`, que e undefined.
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 15000 })
    .catch(() => { throw new Falha('a pagina caiu em modo demonstracao: nao achou o servidor'); });
  await page.fill('#u', USUARIO);
  await page.fill('#s', SENHA);
  await page.fill('#t', TOKEN);
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo', { timeout: 20000 });
  await page.waitForSelector('#arvore .no', { timeout: 20000 });
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
