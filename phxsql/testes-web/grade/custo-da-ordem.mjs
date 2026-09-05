#!/usr/bin/env node
/* O que a grade ORDENADA custa NA TELA -- num navegador de verdade, contra o
 * `phxsqld` de verdade, em varias escalas de tabela.
 *
 *     node phxsql/testes-web/grade/custo-da-ordem.mjs [--linhas 10000,100000,1000000]
 *
 * POR QUE ELA EXISTE. O pedido 188 tem o custo do MOTOR medido -- a grade
 * ordenada percorre o indice INTEIRO antes de recortar, entao 50 linhas custam
 * o mesmo que 1.000. Numero de motor nao decide se ha frente: decide se o
 * custo APARECE para quem esta olhando. A ordem do dono foi essa, e esta
 * bancada e a resposta dela.
 *
 * O QUE ELA MEDE. O gesto inteiro, com o relogio do NAVEGADOR: trocar o
 * «Percorrer por» de «ordem de digitação» para um indice e esperar a grade
 * repintada. Entra tudo o que a pessoa espera -- o `fetch`, o servidor, o
 * JSON de volta, a montagem do DOM e o quadro pintado.
 *
 * O CRIVO -- e ele importa mais que o numero. «Sem ordem» devolve as 200
 * primeiras na ORDEM DE DIGITACAO; «ordenada» devolve as 200 primeiras na
 * ORDEM DA CHAVE. Sao 200 linhas dos dois lados e NAO e o mesmo trabalho: a
 * razao entre as duas mede o PRECO DE PEDIR ORDEM, e nao um motor contra
 * outro. Quem quiser a comparacao de trabalho igual olha o
 * `--example o-que-a-grade-ordenada-custa`, que poe a grade ordenada contra o
 * MINIMO que a mesma pergunta exige.
 *
 * ATENCAO AO BINARIO VELHO: a pagina esta embutida no `phxsqld` por
 * `include_str!`. Antes de rodar:
 *   flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server --bin phxsqld
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { connect } from 'node:net';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { statSync } from 'node:fs';

import { subir, USUARIO, SENHA, TOKEN } from '../servidor.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '../..');

/* Portas proprias: a bateria de ponta a ponta mora na 6200/6201 e nao se
 * encosta nela -- duas medicoes na mesma porta medem o servidor da outra. */
const PORTA_DADOS = 6300;
const PORTA_WEB = 6301;

const arg = (nome, padrao) => {
  const i = process.argv.indexOf(nome);
  return i > 0 ? process.argv[i + 1] : padrao;
};
const ESCALAS = arg('--linhas', '10000,100000,1000000').split(',').map(Number);
const RODADAS = Number(arg('--rodadas', '7'));
/* O teto da grade da aba Conteudo: e o que a tela pede sozinha. */
const PAGINA = 200;

const dormir = ms => new Promise(r => setTimeout(r, ms));
const mediana = v => [...v].sort((a, b) => a - b)[Math.floor(v.length / 2)];

/** UM soquete de dados, mantido aberto -- porque o `login` vive na SESSAO da
 *  conexao: um soquete por pedido perderia a ficha a cada linha e o servidor
 *  responderia «faca login antes» na segunda. */
class Fio {
  constructor(porta) {
    this.fila = [];
    this.buf = '';
    this.s = connect({ host: '127.0.0.1', port: porta });
    this.s.setNoDelay(true);
    this.s.on('data', d => {
      this.buf += d;
      let nl;
      while ((nl = this.buf.indexOf('\n')) >= 0) {
        const linha = this.buf.slice(0, nl);
        this.buf = this.buf.slice(nl + 1);
        const p = this.fila.shift();
        if (p) { try { p.ok(JSON.parse(linha)); } catch (e) { p.falha(e); } }
      }
    });
    this.s.on('error', e => { while (this.fila.length) this.fila.shift().falha(e); });
    this.pronto = new Promise((ok, falha) => { this.s.once('connect', ok); this.s.once('error', falha); });
  }
  async pedir(obj) {
    await this.pronto;
    return await new Promise((ok, falha) => {
      this.fila.push({ ok, falha });
      this.s.write(JSON.stringify(obj) + '\n');
    });
  }
  fechar() { this.s.destroy(); }
}

/** O fio autenticado da semeadura. Uma so ficha para a carga inteira. */
async function abrirFio(porta) {
  const f = new Fio(porta);
  const r = await f.pedir({ op: 'login', usuario: USUARIO, senha: SENHA, token: TOKEN });
  if (r.erro) throw new Error(`login na porta de dados: ${r.erro}`);
  return f;
}

/** Semeia `n` linhas em `tabela`, por CSV colado -- o formato mais magro que
 *  o `inserir_lote` aceita. Em blocos, senao um pedido de 30 MB atravessa o
 *  fio de uma vez e o que se mede depois e a memoria do servidor. */
async function semear(fio, db, tabela, n, log) {
  await fio.pedir({ op: 'criar_database', token: TOKEN, database: db }).catch(() => {});
  const r = await fio.pedir({
    op: 'criar_tabela', token: TOKEN, database: db, tabela,
    colunas: [
      { nome: 'id', tipo: 'Int8', obrigatoria: true },
      { nome: 'nome', tipo: 'Str(40)', obrigatoria: true },
      { nome: 'cidade', tipo: 'Str(20)' },
    ],
    indices: [
      { nome: 'porId', colunas: ['id'], unico: true, primario: true },
      { nome: 'porCidade', colunas: ['cidade'] },
    ],
  });
  if (r.erro) throw new Error(`criar_tabela ${tabela}: ${r.erro}`);

  const BLOCO = 20000;
  const t0 = Date.now();
  for (let i = 1; i <= n; i += BLOCO) {
    const ate = Math.min(i + BLOCO - 1, n);
    const linhas = ['id,nome,cidade'];
    for (let k = i; k <= ate; k++) {
      linhas.push(`${k},Cliente ${String(k).padStart(8, '0')},Cidade ${k % 500}`);
    }
    const v = await fio.pedir({
      op: 'inserir_lote', token: TOKEN, database: db, tabela,
      formato: 'csv', texto: linhas.join('\n'),
    });
    if (v.erro) throw new Error(`inserir_lote ${tabela}: ${v.erro}`);
  }
  log(`  semeadas ${n} linhas em ${tabela} — ${((Date.now() - t0) / 1000).toFixed(1)} s`);
}

/** Troca o «Percorrer por» e devolve os MILISSEGUNDOS ate a grade repintada.
 *
 *  O relogio e o do navegador (`performance.now`), e o fim nao e a resposta do
 *  servidor: e a grade NOVA com linha dentro, mais dois quadros. Parar na
 *  resposta mediria o servidor, e o que se pergunta aqui e o que a pessoa
 *  espera olhando a tela. */
async function trocarOrdem(page, valor) {
  return await page.evaluate(async (v) => {
    const marcaVelha = document.querySelector('#grade');
    const s = document.querySelector('#ord');
    if (!s) throw new Error('a aba Conteudo nao tem o seletor de ordem');
    const t0 = performance.now();
    s.value = v;
    s.dispatchEvent(new Event('change'));
    // Espera a grade NOVA -- outro elemento, e nao o mesmo com dado velho.
    const prazo = performance.now() + 120000;
    for (;;) {
      const g = document.querySelector('#grade');
      if (g && g !== marcaVelha && g.querySelector('tbody tr')) break;
      if (performance.now() > prazo) throw new Error('a grade nao repintou');
      await new Promise(r => requestAnimationFrame(r));
    }
    // Dois quadros: o primeiro entrega o DOM, o segundo garante o pintado.
    await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
    return performance.now() - t0;
  }, valor);
}

async function main() {
  const phxsqld = resolve(RAIZ, 'target/release/phxsqld');
  try { statSync(phxsqld); } catch {
    throw new Error(`nao achei ${phxsqld} — compile com:\n  flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server --bin phxsqld`);
  }
  const log = m => console.log(m);
  const srv = await subir({ phxsqld, portaDados: PORTA_DADOS, portaWeb: PORTA_WEB, log });
  const navegador = await chromium.launch();
  const linhasDoRelatorio = [];
  try {
    const db = 'gradeOrdem';
    const fio = await abrirFio(PORTA_DADOS);
    for (const n of ESCALAS) await semear(fio, db, `t${n}`, n, log);
    fio.fechar();

    const page = await navegador.newPage({ viewport: { width: 1440, height: 900 } });
    await page.goto(srv.url, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#btEntrar');
    await page.fill('#u', USUARIO); await page.fill('#s', SENHA); await page.fill('#t', TOKEN);
    await page.click('#btEntrar');
    await page.waitForSelector('#app.ativo[data-pronto="1"]', { timeout: 30000 });

    for (const n of ESCALAS) {
      const tab = `t${n}`;
      await page.evaluate(([d, t]) => abrirTabela(d, t), [db, tab]);
      await page.click('.aba[data-aba="conteudo"]');
      await page.waitForSelector('#grade tbody tr', { timeout: 120000 });
      await dormir(300);

      const sem = [], ord = [];
      for (let r = 0; r <= RODADAS; r++) {
        // Intercaladas: as duas em sequencia fariam a primeira pagar a
        // arvore fria da outra. E a rodada 0 e jogada fora pelo mesmo motivo.
        const a = await trocarOrdem(page, '');
        const b = await trocarOrdem(page, 'porId');
        if (r === 0) continue;
        sem.push(a); ord.push(b);
      }
      const ms = m => mediana(m).toFixed(1);
      const razao = (mediana(ord) / mediana(sem)).toFixed(1);
      linhasDoRelatorio.push({ n, sem: ms(sem), ord: ms(ord), razao });
      log(`  ${String(n).padStart(9)} linhas: sem ordem ${ms(sem)} ms | ORDENADA ${ms(ord)} ms | ${razao}x`);
    }
  } finally {
    await navegador.close();
    await srv.derrubar();
  }

  console.log('\n=== o custo da grade ordenada NA TELA ===');
  console.log(`    pagina de ${PAGINA} linhas | mediana de ${RODADAS} trocas intercaladas | relogio do navegador\n`);
  console.log(`  ${'linhas'.padStart(11)}  ${'sem ordem'.padStart(12)}  ${'ORDENADA'.padStart(12)}  ${'razao'.padStart(8)}`);
  console.log(`  ${'-'.repeat(49)}`);
  for (const l of linhasDoRelatorio) {
    console.log(`  ${String(l.n).padStart(11)}  ${(l.sem + ' ms').padStart(12)}  ${(l.ord + ' ms').padStart(12)}  ${(l.razao + 'x').padStart(8)}`);
  }
  console.log('\n  CRIVO: as duas devolvem ' + PAGINA + ' linhas e NAO e o mesmo trabalho -- uma na ordem');
  console.log('  de digitacao, a outra na ordem da chave. A razao mede o PRECO DE PEDIR');
  console.log('  ORDEM. O trabalho igual esta no `--example o-que-a-grade-ordenada-custa`.');
}

main().catch(e => { console.error(e); process.exit(1); });
