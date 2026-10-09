#!/usr/bin/env node
/* Prova real, no navegador, da fatia F9 do pedido 495: o painel das
 * ocorrencias e o «explicar» pela Claude, contra o `phxsqld` DE VERDADE e uma
 * Claude FALSA (`claude-falsa.mjs`, um servidor local que fala o contrato da
 * API).
 *
 *     node testes-web/prova-495-f9-ocorrencias.mjs [--binario <phxsqld>] [--porta <n>]
 *                                                  [--defeito <nome>] [--capturas <dir>]
 *
 * Sai 0 quando tudo vale, 1 quando algo falha.
 *
 * A ocorrencia NAO e plantada no arquivo: nasce pelo caminho real. O `.reg`
 * de uma tabela vira um DIRETORIO (o sistema operacional recusa a escrita,
 * como em `testes_da_saude_do_disco.rs`), um `inserir` com um LITERAL
 * sentinela leva um `PhxError::Io` de verdade, o sumidouro do servidor marca
 * `erro_de_disco`, e o carteiro grava a linha no `ocorrencias.log`. Depois o
 * `.reg` volta, e a tabela -- com uma LINHA sentinela dentro -- fica legivel.
 *
 * O que se mede, sempre pelo EFEITO:
 *   - SEM CLIQUE NADA SAI: os pedidos que CHEGAM a Claude falsa, contados
 *     com o painel de aprovacao aberto;
 *   - O QUE SOBE: o corpo que chegou e o corpo mostrado, byte a byte; sem o
 *     literal do `inserir`, sem a linha da tabela, sem o login e sem o IP;
 *   - A CHAVE: todo pedido que a pagina fez ao PhxSql (URL, cabecalhos e
 *     corpo), lido pelo `page.on('request')` -- a prova do 339(a) estendida
 *     ao painel novo. Controle: a chave CHEGA a Claude falsa.
 *
 * `--defeito` repoe um defeito SEM recompilar (o proxy reverso de
 * `claude-interceptar.mjs` reescreve a pagina servida) e a prova tem de sair
 * VERMELHA:
 *   chave-no-pedido   o painel manda a chave no pedido `ocorrencias`
 *   envio-sem-clique  o «explicar» envia sem esperar o clique
 *   corpo-com-pessoa  o corpo leva login e IP (a lista de permissao aberta)
 *   corpo-com-linha   o corpo leva as linhas da tabela da ocorrencia
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { renameSync, mkdirSync, rmdirSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir, USUARIO } from './servidor.mjs';
import { entrar, api } from './apoio.mjs';
import { definirIA } from './claude-apoio.mjs';
import { subirFalsa } from './claude-falsa.mjs';
import { subirCopiaComPatch } from './claude-interceptar.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const arg = (nome, padrao) => {
  const i = process.argv.indexOf(nome);
  return i >= 0 ? process.argv[i + 1] : padrao;
};
const BINARIO = resolve(arg('--binario', resolve(AQUI, '..', 'target', 'debug', 'phxsqld')));
const PORTA = Number(arg('--porta', '6495'));
const DEFEITO = arg('--defeito', '');
const CAPTURAS = arg('--capturas', '');

// Fabricada: a forma de uma chave, nunca o valor de uma real.
const CHAVE = 'sk-ant-prova-495-f9-FABRICADA-0123456789AB';
const LITERAL = 'LITERAL-495-f9-k2v';
const LINHA = 'LINHA-495-f9-p7m';
const DB = 'prova495';
const TAB = 'clientes';

/* Cada defeito e UMA troca de texto na pagina servida. O ponto de troca que
 * sumir e erro da prova, e nao verde: a prova que nao acha o que repor nao
 * mede nada. */
const DEFEITOS = {
  'chave-no-pedido': [
    'const r = await api("ocorrencias", pedido);',
    'const r = await api("ocorrencias", Object.assign(pedido, { _chave: PhxIA._cfg().chave }));',
  ],
  'envio-sem-clique': [
    'onde.querySelector("#iaOcAprovar").onclick = () => fimDaEspera(true);',
    'onde.querySelector("#iaOcAprovar").onclick = () => fimDaEspera(true); fimDaEspera(true);',
  ],
  'corpo-com-pessoa': [
    'const CAMPOS_DA_OCORRENCIA = ["quando",',
    'const CAMPOS_DA_OCORRENCIA = ["usuario", "ip", "quando",',
  ],
  // A base e a tabela vao escritas porque a ocorrencia do sumidouro chega
  // sem as duas (lacuna da F2, dita no relatorio): o defeito tem de LER a
  // linha de verdade, senao o vermelho seria do erro e nao do vazamento.
  'corpo-com-linha': [
    '+ JSON.stringify(eventoDaOcorrencia(o), null, 1);',
    `+ JSON.stringify(eventoDaOcorrencia(o), null, 1) + JSON.stringify(await api("sql", `
      + `{ database: "${DB}", texto: "SELECT * FROM ${TAB}" }));`,
  ],
};

const resultados = [];
function registrar(id, ok, detalhe) {
  resultados.push({ id, ok, detalhe });
  console.log(`${ok ? 'VERDE   ' : 'VERMELHO'} ${id} -- ${detalhe}`);
}
async function passo(id, fn) {
  try { await fn(); } catch (e) { registrar(id, false, `estourou: ${String(e.message || e).split('\n')[0]}`); }
}

const dormir = ms => new Promise(r => setTimeout(r, ms));

if (DEFEITO && !DEFEITOS[DEFEITO]) {
  console.error(`defeito desconhecido: ${DEFEITO}; os que existem: ${Object.keys(DEFEITOS).join(', ')}`);
  process.exit(2);
}

const navegador = await chromium.launch();
let srv = null, falsa = null, copia = null;
try {
  srv = await subir({ phxsqld: BINARIO, portaDados: PORTA, portaWeb: PORTA + 1,
                      log: m => console.log(`· ${m}`) });
  falsa = await subirFalsa({ porta: PORTA + 2 });
  // O `limpar` da falsa volta o padrao de fabrica: zera-se pelos dois juntos.
  const zerarFalsa = () => {
    falsa.limpar();
    falsa.definirPadrao({ resposta: 'sucesso', texto: 'O disco recusou a escrita. Isto é hipótese a CONFERIR, não diagnóstico.',
                          tokensEntrada: 7, tokensSaida: 9, pedacos: 3, atrasoMs: 5 });
  };
  zerarFalsa();
  let url = srv.url;
  if (DEFEITO) {
    const [de, para] = DEFEITOS[DEFEITO];
    copia = await subirCopiaComPatch({
      portaOuvir: PORTA + 3, portaReal: PORTA + 1,
      patchCorpo: html => {
        if (!html.includes(de)) throw new Error(`defeito ${DEFEITO}: ponto de troca nao achado`);
        return html.replace(de, para);
      },
    });
    url = copia.url;
    console.log(`· DEFEITO REPOSTO: ${DEFEITO} (pagina servida por ${url})`);
  }
  const origemPhx = new URL(url).origin;

  // bypassCSP: a Claude falsa mora em 127.0.0.1, fora do `connect-src` da
  // pagina. A CSP de verdade e provada pela bateria 3 da `claude-bateria.mjs`.
  const ctx = await navegador.newContext({ viewport: { width: 1500, height: 950 }, bypassCSP: true });
  const page = await ctx.newPage();
  const aoPhx = [];
  page.on('request', r => {
    if (r.url().startsWith(origemPhx)) {
      aoPhx.push({ url: r.url(), cab: JSON.stringify(r.headers()), corpo: r.postData() || '' });
    }
  });
  await entrar(page, url);

  // ------------------------------------------- a ocorrencia, pelo caminho real
  await api(page, 'criar_database', { database: DB }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: DB, tabela: TAB,
    colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }, { nome: 'nome', tipo: 'Str(40)' }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  });
  await api(page, 'inserir', { database: DB, tabela: TAB, valores: [1, LINHA] });
  const reg = join(srv.base, DB, `${TAB}.reg`);
  if (!existsSync(reg)) throw new Error(`nao achei ${reg}`);
  renameSync(reg, `${reg}.guardado`);
  mkdirSync(reg);
  const recusa = await api(page, 'inserir', { database: DB, tabela: TAB, valores: [2, LITERAL] })
    .then(() => null, e => String(e));
  rmdirSync(reg);
  renameSync(`${reg}.guardado`, reg);
  if (!recusa) throw new Error('o inserir com o .reg quebrado foi aceito: nao houve erro de disco');

  let ocorrencia = null, visto = null;
  for (let i = 0; i < 100 && !ocorrencia; i++) {
    visto = await api(page, 'ocorrencias', {});
    ocorrencia = (visto.linhas || []).find(l => l.alarme === 'erro_de_disco') || null;
    if (!ocorrencia) await dormir(100);
  }
  if (!ocorrencia) console.log(`  ocorrencias vistas: ${JSON.stringify(visto.linhas)}`);
  registrar('a_ocorrencia_nasce_pelo_caminho_real', !!ocorrencia,
    ocorrencia ? `erro_de_disco: ${JSON.stringify(ocorrencia)}`
               : `nenhuma linha erro_de_disco em ${TAB} (recusa: ${recusa})`);
  if (!ocorrencia) throw new Error('sem ocorrencia nao ha o que explicar');
  // O id da sessao e a credencial do `X-Sessao`. Ele nao pode sair na
  // ocorrencia (o `tarefa`), nem no aquario que quem so MONITORA le, nem na
  // telemetria: achado exercitando esta fatia, a chave da atividade web era
  // o id cru.
  const sid = await page.evaluate(() => est.sessao);
  const retrato = JSON.stringify(await api(page, 'aquario_retrato', {}));
  const telem = JSON.stringify(await api(page, 'telemetria', {}));
  registrar('o_id_da_sessao_nao_sai_na_ocorrencia_no_aquario_nem_na_telemetria',
    !!sid && !JSON.stringify(ocorrencia).includes(sid) && !retrato.includes(sid) && !telem.includes(sid),
    `sessao lida: ${!!sid}; na ocorrencia: ${!!sid && JSON.stringify(ocorrencia).includes(sid)}; `
    + `no aquario_retrato: ${!!sid && retrato.includes(sid)}; na telemetria: ${!!sid && telem.includes(sid)}`);
  registrar('a_op_nao_devolve_o_literal', !JSON.stringify(ocorrencia).includes(LITERAL),
    `literal na linha devolvida: ${JSON.stringify(ocorrencia).includes(LITERAL)}`);

  // ------------------------------------------------ o painel, sem a Claude
  await page.click('[data-admin="ocorrencias"]');
  await page.waitForSelector('#gradeOcorrencias', { timeout: 15000 });
  await page.waitForFunction(() => document.querySelector('#gradeOcorrencias')?.textContent.includes('erro_de_disco'),
    undefined, { timeout: 15000 });
  const semIa = await page.$('.bt-explicar-oc');
  registrar('sem_a_integracao_o_explicar_nao_aparece', !semIa, `botao explicar desenhado: ${!!semIa}`);

  // O filtro do SERVIDOR: um alarme que nao aconteceu esvazia a grade.
  await page.selectOption('#ocAlarme', 'forca_bruta');
  await page.click('#ocFiltrar');
  await page.waitForFunction(() => /^0\b/.test(document.querySelector('.ferramentas .conta')?.textContent || ''),
    undefined, { timeout: 15000 });
  await page.selectOption('#ocAlarme', '');
  await page.click('#ocFiltrar');
  await page.waitForFunction(() => document.querySelector('#gradeOcorrencias')?.textContent.includes('erro_de_disco'),
    undefined, { timeout: 15000 });
  registrar('o_filtro_por_alarme_vai_ao_servidor', true, 'forca_bruta esvaziou; todos trouxe de volta');

  // ---------------------------------------------------- com a Claude falsa
  await definirIA(page, { chave: CHAVE, ligado: true, modelo: 'claude-haiku-4-5',
                          endpoint: falsa.endpointMensagens });
  await page.click('#ocFiltrar');
  await page.waitForSelector('.bt-explicar-oc', { timeout: 15000 });

  await passo('nao_enviar_nao_envia', async () => {
    zerarFalsa();
    await page.click('.bt-explicar-oc');
    await page.waitForSelector('#iaOcNaoEnviar', { timeout: 10000 });
    await page.click('#iaOcNaoEnviar');
    await page.waitForSelector('#iaOcNaoEnviado', { timeout: 10000 });
    await dormir(500);
    registrar('nao_enviar_nao_envia', falsa.lerPedidos().length === 0,
      `pedidos que chegaram a Claude depois de «Nao enviar»: ${falsa.lerPedidos().length}`);
  });

  await passo('sem_clique_nada_sai', async () => {
    zerarFalsa();
    await page.click('.bt-explicar-oc');
    await page.waitForSelector('#iaOcCorpo', { timeout: 10000 });
    // O tempo de uma chamada inteira a falsa (3 pedacos de 5 ms): se o
    // envio sai sozinho, ja chegou.
    await dormir(1200);
    registrar('sem_clique_nada_sai', falsa.lerPedidos().length === 0,
      `pedidos que chegaram a Claude ANTES do clique: ${falsa.lerPedidos().length}`);
  });

  await passo('o_corpo_aprovado_e_o_que_sai_e_sai_limpo', async () => {
    const mostrado = await page.$eval('#iaOcCorpo', p => p.textContent);
    if (await page.$('#iaOcAprovar')) await page.click('#iaOcAprovar');
    await page.waitForFunction(() => document.querySelector('#iaOcTokens b')
      || document.querySelector('#iaOcSaida .aviso.mal'), undefined, { timeout: 15000 });
    const erro = await page.$eval('#iaOcSaida', s => s.querySelector('.aviso.mal')?.textContent || '');
    const chegou = falsa.lerPedidos();
    const corpo = chegou.length ? JSON.stringify(chegou[0].corpo) : '';
    registrar('aprovado_chega_uma_vez_a_claude', chegou.length === 1 && !erro,
      `pedidos: ${chegou.length}; erro na tela: ${erro || '-'}`);
    registrar('aprovado_sai_exatamente_o_que_foi_mostrado',
      chegou.length >= 1 && JSON.stringify(chegou[chegou.length - 1].corpo) === JSON.stringify(JSON.parse(mostrado)),
      'corpo da falsa comparado ao #iaOcCorpo');
    registrar('o_corpo_leva_a_ocorrencia', corpo.includes('erro_de_disco'),
      `erro_de_disco no corpo: ${corpo.includes('erro_de_disco')}`);
    registrar('o_corpo_sobe_sem_literal', corpo.length > 0 && !corpo.includes(LITERAL),
      `literal do inserir no corpo: ${corpo.includes(LITERAL)}`);
    registrar('o_corpo_sobe_sem_linha_de_dado', corpo.length > 0 && !corpo.includes(LINHA),
      `linha da tabela no corpo: ${corpo.includes(LINHA)}`);
    // O evento vai DENTRO de um texto (`content`), entao as aspas chegam
    // escapadas duas vezes no `corpo` serializado.
    const login = `\\"${USUARIO}\\"`, ip = '127.0.0.1';
    registrar('o_corpo_sobe_sem_login_e_sem_ip',
      corpo.length > 0 && !corpo.includes(login) && !corpo.includes(ip),
      `login no corpo: ${corpo.includes(login)}; ip no corpo: ${corpo.includes(ip)}`);
    registrar('controle_a_chave_chega_a_claude',
      chegou.length > 0 && chegou.every(p => p.chaveVista === CHAVE),
      `pedidos a Claude com a chave: ${chegou.filter(p => p.chaveVista === CHAVE).length}`);
    const texto = await page.$eval('#iaOcTexto', p => p.textContent).catch(() => '');
    registrar('a_resposta_aparece_na_tela', texto.includes('hipótese a CONFERIR'), `resposta: ${texto.slice(0, 60)}`);
  });

  // ------------------------------------- a chave nunca vai ao PhxSql (339(a))
  const comChave = aoPhx.filter(p => p.url.includes(CHAVE) || p.cab.includes(CHAVE) || p.corpo.includes(CHAVE));
  registrar('a_chave_nunca_vai_ao_phxsql', aoPhx.length > 0 && comChave.length === 0,
    `${aoPhx.length} pedidos ao PhxSql, ${comChave.length} com a chave${comChave.length ? ': ' + comChave[0].corpo.slice(0, 120) : ''}`);

  // Os dois temas: o painel e o «explicar» desenham sem erro de pagina.
  await passo('os_dois_temas', async () => {
    const erros = [];
    page.on('pageerror', e => erros.push(String(e)));
    for (const tema of ['claro', 'escuro']) {
      await page.evaluate(t => aplicarTema(t), tema);
      await page.click('[data-admin="ocorrencias"]');
      await page.waitForSelector('.bt-explicar-oc', { timeout: 10000 });
      if (CAPTURAS) {
        mkdirSync(CAPTURAS, { recursive: true });
        await page.screenshot({ path: join(CAPTURAS, `ocorrencias-lista-${tema}.png`) });
      }
      await page.click('.bt-explicar-oc');
      await page.waitForSelector('#iaOcCorpo', { timeout: 10000 });
      if (CAPTURAS) {
        mkdirSync(CAPTURAS, { recursive: true });
        await page.screenshot({ path: join(CAPTURAS, `ocorrencias-${tema}.png`), fullPage: true });
      }
      await page.click('#iaOcNaoEnviar');
    }
    registrar('os_dois_temas', erros.length === 0, `erros de pagina: ${erros.length}${erros.length ? ' ' + erros[0] : ''}`);
  });
  await ctx.close();
} catch (e) {
  registrar('prova_inteira', false, `estourou: ${String(e.message || e).split('\n')[0]}`);
} finally {
  await navegador.close();
  if (copia) await copia.derrubar();
  if (falsa) await falsa.derrubar();
  if (srv) await srv.derrubar();
}

const vermelhos = resultados.filter(r => !r.ok);
console.log(`\n${resultados.length - vermelhos.length}/${resultados.length} verdes${DEFEITO ? ` (defeito reposto: ${DEFEITO})` : ''}`);
process.exit(vermelhos.length ? 1 : 0);
