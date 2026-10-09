/* Prova pelo navegador do TLS de saida nas telas (pedido 572, papel E,
 * 08/10/2026). Roda nos DOIS temas, contra o phxsqld de verdade.
 *
 *   node testes-web/prova-pino-tls-tela.mjs [caminho/do/phxsqld]   (da pasta phxsql/)
 *
 * Os campos `pino_tls`, `tls` e `tls_ca` entraram no servidor configuraveis
 * so pelo JSON. Esta prova confere que as telas que ja editam esses objetos
 * os levam -- com o comportamento do SERVIDOR, que e o que se mede, e nao o
 * que a tela diz de si:
 *
 *   DbLink    o modo, as ancoras e o pino; a ficha devolve so `tem_pino_tls`
 *             (o pino nao volta ao DOM); salvar sem mexer HERDA o pino; trocar
 *             modo, ancoras ou pino sem a senha e recusado antes de sair, e
 *             com ela grava; pino + verificar e recusado; o phxsql nao mostra
 *             modo nenhum; a grade das definicoes diz o fio de cada ligacao;
 *   e-mail    a tela de Configuracoes diz o modo e o FATO do pino, e nao diz
 *             mais «sem TLS»;
 *   replicacao  o assistente manda o `pino_tls` na sonda e tira dela o pino
 *             do tunel, que o TLS substitui;
 *   cluster   acrescentar no com pino TLS grava `pino_tls` em `cluster.nos`
 *             (lido do config.json, e nao da tela).
 *
 * E em todo campo de pino: `text-transform` nenhum. Base64 distingue caixa,
 * e um pino mostrado em caixa alta e mentira sobre o dado -- o «Blumenau».
 *
 * Sai com codigo 1 se qualquer medida reprovar. Capturas em $SAIDA. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import { subir } from './servidor.mjs';
import { entrar, api } from './apoio.mjs';

const PHXSQLD = resolve(process.argv[2] || 'target/debug/phxsqld');
const SAIDA = process.env.SAIDA || '/tmp/prova-pino-tls-tela';
mkdirSync(SAIDA, { recursive: true });

// Um pino bem formado (32 bytes em base64) que nao e de ninguem: a prova
// mede o que a tela MANDA e o que o servidor GUARDA, nao um aperto de mao.
const PINO = 'sha256//' + createHash('sha256').update('prova-pino-tls-tela').digest('base64');
const PINO2 = 'sha256//' + createHash('sha256').update('outro-no').digest('base64');

const reprovas = [];
const nota = (ok, oQue, valor = '') => {
  console.log(`${ok ? 'ok ' : 'XX '} ${oQue}${valor === '' ? '' : `: ${valor}`}`);
  if (!ok) reprovas.push(oQue);
};
/* Uma secao que cai (seletor que nao existe no binario de antes) reprova com
 * o motivo e deixa as outras rodarem: uma linha dizendo onde vale mais que um
 * timeout cru no meio da saida. */
async function secao(nome, f) {
  try { await f(); } catch (e) { nota(false, `${nome} caiu`, String(e.message || e).split('\n')[0]); }
}

const t = (page, chave) => page.evaluate(k => txt(k), chave);
const semMarca = s => String(s || '').replace(/\*\*|`/g, '');
const aviso = page => page.evaluate(() => {
  const a = document.querySelector('#aviso');
  // So o recado de ERRO: o de exito («ligacao gravada») nao e recusa.
  return a && !a.hidden && a.classList.contains('mal') ? a.textContent : '';
});
const limparAviso = page => page.evaluate(() => { const a = document.querySelector('#aviso'); if (a) { a.hidden = true; a.textContent = ''; } });
const ligacao = async (page, nome) => ((await api(page, 'dblink')).ligacoes || []).find(l => l.nome === nome);
const caixaDoPino = (page, sel) => page.$eval(sel, i => ({
  transform: getComputedStyle(i).textTransform,
  inteiro: i.scrollWidth <= i.clientWidth + 1,
}));
/* O formulario do DbLink, aberto pelo mesmo caminho do clique na grade. */
async function abrirFicha(page, nome) {
  await page.evaluate(n => editarDbLink(n), nome);
  await page.waitForSelector('#btGravar');
}
async function gravarFicha(page) {
  await limparAviso(page);
  const antes = await page.evaluate(() => document.querySelector('#gradeDbl') !== null);
  await page.click('#btGravar');
  // Ou o recado de erro aparece, ou a tela volta as definicoes.
  await page.waitForFunction(a => {
    const av = document.querySelector('#aviso');
    return (av && !av.hidden && av.classList.contains('mal')) || (!a && document.querySelector('#gradeDbl'));
  }, antes, { timeout: 10000 }).catch(() => {});
  return aviso(page);
}

// Dois servidores: o do cluster sobe com um no MORTO na lista, e a maioria
// que falta poderia recusar escrita das outras telas -- entao elas moram no
// servidor de sempre.
const A = await subir({
  phxsqld: PHXSQLD, portaDados: 6360, portaWeb: 6361, log: console.log,
  extra: {
    alertas: {
      email: {
        ligado: true, servidor: '127.0.0.1', porta: 2525, de: 'phx@local',
        para: ['ops@local'], tls: 'exigir', pino_tls: PINO,
      },
    },
  },
});
const B = await subir({
  phxsqld: PHXSQLD, portaDados: 6364, portaWeb: 6365, log: console.log,
  extra: {
    replicacao: { papel: 'source', id_servidor: 'no1', imagem_da_linha: true },
    cluster: {
      id: 'no1', prioridade: 2, janela_inatividade_s: 10, pulso_s: 1,
      nos: [{ id: 'no1', endereco: '127.0.0.1', porta: 6364 },
            { id: 'no2', endereco: '127.0.0.1', porta: 6369 }],
    },
  },
});
const nav = await chromium.launch();
try {
  for (const tema of ['escuro', 'claro']) {
    console.log(`\n== tema ${tema}`);
    const ctx = await nav.newContext({ viewport: { width: 1280, height: 900 } });
    await ctx.addInitScript(x => { try { localStorage.setItem('phxsql-tema', x); } catch {} }, tema);
    const page = await ctx.newPage();
    const erros = [];
    page.on('pageerror', e => erros.push(String(e)));
    await entrar(page, A.url);
    const pg = `erp${tema[0]}`, phx = `irmao${tema[0]}`;

    // ------------------------------------------------------------ DbLink
    await secao('DbLink', async () => {
      await api(page, 'dblink_salvar', { nome: pg, motor: 'postgres', host: '127.0.0.1', porta: 5432,
        usuario: 'leitor', senha: 'segredo-pg', database: 'erp' });
      await abrirFicha(page, pg);
      const vis = await page.evaluate(() => ({
        tls: !document.querySelector('#fTls').closest('label').hidden,
        ca: !document.querySelector('#fTlsCa').closest('label').hidden,
        pino: !document.querySelector('#fPinoTls').closest('label').hidden,
        valor: document.querySelector('#fTls').value,
      }));
      nota(vis.tls && vis.pino && !vis.ca && vis.valor === '',
        'DbLink postgres: modo e pino a vista, ancoras so no verificar', JSON.stringify(vis));
      const cx = await caixaDoPino(page, '#fPinoTls');
      nota(cx.transform === 'none', 'DbLink: o pino nao muda de caixa', cx.transform);

      // verificar sem a senha: recusado ANTES de sair, e o servidor nao muda.
      await page.selectOption('#fTls', 'verificar');
      const caVisivel = await page.$eval('#fTlsCa', i => !i.closest('label').hidden);
      nota(caVisivel, 'DbLink: escolher verificar mostra as ancoras');
      await page.fill('#fTlsCa', 'sistema');
      await page.screenshot({ path: join(SAIDA, `${tema}-dblink-verificar.png`), fullPage: true });
      const r1 = await gravarFicha(page);
      const falta = await t(page, 'tela.dbl_tls_falta_credencial');
      nota(r1 === falta, 'DbLink: trocar o modo sem a senha pede a senha', r1);
      nota(((await ligacao(page, pg)) || {}).tls === '', 'DbLink: o servidor nao mudou com a recusa');

      await page.fill('#fSenha', 'segredo-pg');
      const r2 = await gravarFicha(page);
      const l2 = await ligacao(page, pg);
      nota(!r2 && l2.tls === 'verificar' && l2.tls_ca === 'sistema',
        'DbLink: com a senha, o servidor grava verificar e as ancoras', r2 || `${l2.tls} ${l2.tls_ca}`);

      // pino + verificar se contradizem: recado, sem ida ao servidor.
      await abrirFicha(page, pg);
      nota(await page.$eval('#fTls', s => s.value) === 'verificar', 'DbLink: a ficha volta com o modo gravado');
      await page.fill('#fPinoTls', PINO);
      await page.fill('#fSenha', 'segredo-pg');
      const r3 = await gravarFicha(page);
      nota(r3 === await t(page, 'tela.dbl_tls_pino_contradiz'), 'DbLink: pino com verificar e recusado', r3);

      // pino torto: recado com a forma certa.
      await page.selectOption('#fTls', 'exigir');
      await page.fill('#fPinoTls', 'sha256//curto');
      const r4 = await gravarFicha(page);
      nota(r4 === await t(page, 'tela.pino_tls_torto'), 'DbLink: pino torto e recusado', r4);

      // exigir + pino, com a senha: grava; a ficha so diz que ha pino.
      await page.fill('#fPinoTls', ` ${PINO.slice(0, 20)} ${PINO.slice(20)} `);
      const r5 = await gravarFicha(page);
      const l5 = await ligacao(page, pg);
      nota(!r5 && l5.tls === 'exigir' && l5.tem_pino_tls === true && !JSON.stringify(l5).includes(PINO.slice(8)),
        'DbLink: exigir + pino gravados, e a ficha so diz tem_pino_tls', r5 || JSON.stringify({ tls: l5.tls, tem: l5.tem_pino_tls }));

      // A volta: o pino NAO volta ao DOM, e salvar sem mexer herda.
      await abrirFicha(page, pg);
      const dom = await page.evaluate(p => ({
        vazio: document.querySelector('#fPinoTls').value === '',
        tem: !!document.querySelector('#fPinoTlsTem'),
        vazou: document.documentElement.outerHTML.includes(p),
      }), PINO.slice(8));
      nota(dom.vazio && dom.tem && !dom.vazou, 'DbLink: o pino nao volta a tela, so o selo «guardado»', JSON.stringify(dom));
      await page.screenshot({ path: join(SAIDA, `${tema}-dblink-pino-guardado.png`), fullPage: true });
      await page.fill('#fDesc', 'so a descricao mudou');
      const r6 = await gravarFicha(page);
      const l6 = await ligacao(page, pg);
      nota(!r6 && l6.tem_pino_tls === true && l6.tls === 'exigir' && l6.descricao === 'so a descricao mudou',
        'DbLink: salvar sem mexer no TLS herda o pino e o modo, sem pedir senha', r6 || JSON.stringify({ tem: l6.tem_pino_tls, tls: l6.tls }));

      // Apagar o pino e troca de destino: sem a senha, recusa; com ela, apaga.
      await abrirFicha(page, pg);
      await page.check('#fPinoTlsApagar');
      const r7 = await gravarFicha(page);
      nota(r7 === falta && ((await ligacao(page, pg)) || {}).tem_pino_tls === true,
        'DbLink: apagar o pino sem a senha pede a senha', r7);
      await page.fill('#fSenha', 'segredo-pg');
      const r8 = await gravarFicha(page);
      nota(!r8 && ((await ligacao(page, pg)) || {}).tem_pino_tls === false, 'DbLink: com a senha, o pino sai', r8);

      // A grade das definicoes diz o fio, e o aviso nao diz mais «nao ha TLS».
      await page.evaluate(() => telaDbLinkDefinicoes());
      await page.waitForSelector('#gradeDbl .phx-grid tbody tr');
      const grade = await page.evaluate(n => {
        const ths = [...document.querySelectorAll('#gradeDbl .phx-th-titulo')].map(x => x.innerText.trim());
        const tr = [...document.querySelectorAll('#gradeDbl .phx-grid tbody tr')]
          .find(r => r.innerText.includes(n));
        return { ths, linha: tr ? tr.innerText : '', corpo: document.querySelector('#painel').innerText };
      }, pg);
      nota(grade.ths.some(x => x.startsWith('TLS')) && /exigir/.test(grade.linha),
        'DbLink: a grade das definicoes tem a coluna do TLS', grade.linha.replace(/\s+/g, ' ').slice(0, 120));
      const titulo = semMarca(await t(page, 'tela.dbl_aviso_tls_titulo'));
      nota(grade.corpo.includes(titulo) && !/Não há TLS/.test(grade.corpo),
        'DbLink: o aviso diz que o TLS e pedido, e nao que nao existe');
      await page.screenshot({ path: join(SAIDA, `${tema}-dblink-definicoes.png`), fullPage: true });

      // O phxsql: modo e ancoras escondidos (o servidor os recusa ali), o pino
      // a vista -- e o salvar com pino e token grava tem_pino_tls.
      await api(page, 'dblink_salvar', { nome: phx, motor: 'phxsql', host: '127.0.0.1', porta: 6360,
        token_remoto: 'bateria', cifra: false });
      await abrirFicha(page, phx);
      const vp = await page.evaluate(() => ({
        tls: !document.querySelector('#fTls').closest('label').hidden,
        pino: !document.querySelector('#fPinoTls').closest('label').hidden,
      }));
      nota(!vp.tls && vp.pino, 'DbLink phxsql: sem modo, com pino', JSON.stringify(vp));
      await page.fill('#fPinoTls', PINO);
      const r9 = await gravarFicha(page);
      nota(r9 === falta, 'DbLink phxsql: o pino sem o token pede a credencial', r9);
      await page.fill('#fToken', 'bateria');
      const r10 = await gravarFicha(page);
      const l10 = await ligacao(page, phx);
      nota(!r10 && l10.tem_pino_tls === true && !l10.tls, 'DbLink phxsql: pino gravado, nenhum modo mandado', r10);
    });

    // ------------------------------------------------- aviso por e-mail
    await secao('e-mail', async () => {
      await page.evaluate(() => verConfigServidor());
      await page.waitForFunction(() => [...document.querySelectorAll('#painel td code')]
        .some(c => c.textContent === 'alertas.email.ligado'), null, { timeout: 15000 });
      const linhas = await page.evaluate(() => Object.fromEntries([...document.querySelectorAll('#painel tr')]
        .map(tr => [tr.querySelector('td code')?.textContent, [...tr.querySelectorAll('td')].map(td => td.innerText.trim())])
        .filter(([k]) => k && k.startsWith('alertas.email'))));
      const tls = linhas['alertas.email.tls'] || [];
      const pino = linhas['alertas.email.pino_tls'] || [];
      nota(tls[1] === 'exigir', 'e-mail: o modo do TLS na tela', tls[1]);
      const fato = await t(page, 'tela.cfg_pino_tls_guardado');
      nota(!!fato && pino[1] === fato, 'e-mail: o pino aparece como FATO', pino[1]);
      nota(!(linhas['alertas.email.ligado'] || [])[2]?.startsWith('sem TLS'), 'e-mail: a dica nao diz mais «sem TLS»',
        (linhas['alertas.email.ligado'] || [])[2]);
      const vazou = await page.evaluate(p => document.documentElement.outerHTML.includes(p), PINO.slice(8));
      nota(!vazou, 'e-mail: o pino nao vai ao DOM');
      const corpo = await page.evaluate(() => document.querySelector('#painel').innerText);
      nota(!corpo.includes(semMarca(await t(page, 'tela.cfg_em_titulo'))),
        'e-mail: com TLS e pino, a nota de texto claro some');
      await page.evaluate(() => [...document.querySelectorAll('#painel td code')]
        .find(c => c.textContent === 'alertas.email.tls')?.scrollIntoView({ block: 'center' }));
      await page.screenshot({ path: join(SAIDA, `${tema}-email.png`) });
    });

    // ------------------------------------------- assistente de replicacao
    await secao('replicacao', async () => {
      const sondas = [];
      const ouvir = r => {
        if (r.method() !== 'POST' || !r.url().endsWith('/api')) return;
        try { const b = JSON.parse(r.postData() || '{}'); if (b.op === 'replicacao_testar') sondas.push(b); } catch {}
      };
      page.on('request', ouvir);
      page.evaluate(() => assistenteReplicacao());
      await page.waitForSelector('#rzIr1');
      await page.click('#rzIr1');
      await page.waitForSelector('#rzPinoTls');
      const cx = await caixaDoPino(page, '#rzPinoTls');
      nota(cx.transform === 'none', 'replicacao: o pino nao muda de caixa', cx.transform);
      await page.fill('#rzTokenR', 'bateria');
      await page.fill('#rzPino', 'ab'.repeat(32));
      await page.fill('#rzPinoTls', 'sha256//torto');
      await page.click('#rzIr2');
      const rec = await page.$eval('#rzRecado', e => e.innerText.trim());
      nota(rec === await t(page, 'tela.pino_tls_torto'), 'replicacao: pino torto e recusado', rec);
      await page.fill('#rzPinoTls', PINO);
      const fio = await page.evaluate(() => ({
        cifra: document.querySelector('#rzCifra').disabled,
        pino: document.querySelector('#rzPino').disabled,
        diz: document.querySelector('#rzFioDiz').innerText,
      }));
      nota(fio.cifra && fio.pino && fio.diz.includes('TLS 1.3'),
        'replicacao: com o pino TLS, o tunel se apaga e o fio diz TLS', JSON.stringify(fio));
      await page.screenshot({ path: join(SAIDA, `${tema}-replicacao.png`) });
      await page.click('#rzIr2');
      await page.waitForFunction(n => n > 0, sondas.length, { timeout: 1 }).catch(() => {});
      for (let i = 0; i < 50 && !sondas.length; i++) await page.waitForTimeout(100);
      const s = sondas[0] || {};
      nota(s.pino_tls === PINO && !('chave_do_fio' in s),
        'replicacao: a sonda leva o pino TLS, e nao o pino do tunel', JSON.stringify({ pino_tls: s.pino_tls, chave_do_fio: s.chave_do_fio }));
      page.off('request', ouvir);
      await page.evaluate(() => document.querySelector('.sobre')?.remove());
    });

    nota(!erros.length, `nenhum erro de pagina (${tema})`, erros.join(' | '));
    await ctx.close();

    // ---------------------------------------------------------- cluster
    await secao('cluster', async () => {
      const ctxB = await nav.newContext({ viewport: { width: 1280, height: 900 } });
      await ctxB.addInitScript(x => { try { localStorage.setItem('phxsql-tema', x); } catch {} }, tema);
      const pb = await ctxB.newPage();
      try {
        await entrar(pb, B.url);
        await pb.evaluate(() => verCluster());
        await pb.waitForSelector('#clNovoPinoTls', { timeout: 15000 });
        const cx = await caixaDoPino(pb, '#clNovoPinoTls');
        await pb.fill('#clNovoPinoTls', PINO2);
        const cx2 = await caixaDoPino(pb, '#clNovoPinoTls');
        nota(cx.transform === 'none' && cx2.inteiro, 'cluster: o pino cabe inteiro e sem caixa alta', JSON.stringify(cx2));
        const id = `no3${tema[0]}`;
        await pb.fill('#clNovoId', id);
        await pb.fill('#clNovoEnd', '127.0.0.1');
        await pb.fill('#clNovaPorta', '6368');
        await pb.screenshot({ path: join(SAIDA, `${tema}-cluster.png`), fullPage: true });
        await pb.click('#btClAdd');
        let no = null;
        for (let i = 0; i < 50 && !no; i++) {
          await pb.waitForTimeout(100);
          const c = JSON.parse(readFileSync(B.config, 'utf8'));
          no = ((c.cluster || {}).nos || []).find(n => n.id === id) || null;
        }
        nota(no && no.pino_tls === PINO2, 'cluster: o no novo entra em cluster.nos COM o pino TLS',
          JSON.stringify(no));
      } finally { await ctxB.close(); }
    });
  }
} finally {
  await nav.close();
  await A.derrubar();
  await B.derrubar();
}
console.log(reprovas.length ? `\nREPROVADO: ${reprovas.length}` : '\nAPROVADO');
process.exit(reprovas.length ? 1 : 0);
