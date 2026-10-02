#!/usr/bin/env node
/* PROVA REAL dos casos de botao do pedido 190, nos DOIS sentidos.
 *
 *     node phxsql/testes-web/prova-real-botoes.mjs [--so <pedaco>]
 *
 * Teste que passa por engano e pior que teste que falta. Para cada caso novo,
 * esta prova SERVE a pagina com UM defeito reposto -- o botao morto, o
 * confirm ignorado, a chave inteira no painel -- e exige que o caso REPROVE;
 * e serve a pagina sem defeito e exige que ele PASSE (o controle: um portao
 * que recusasse tudo tambem "reprovaria" o defeito).
 *
 * Sem recompilar o `phxsqld`: a pagina vem de `include_str!`, entao o defeito
 * e reposto num proxy reverso (`subirCopiaComPatch`, o mesmo precedente da
 * `claude-interceptar.mjs`) que reescreve so o documento `/` e repassa o resto
 * ao servidor de verdade. Cada reposicao CONFERE que o trecho existe -- se a
 * tela mudar de forma, a prova grita em vez de passar sem repor nada.
 *
 * Sai com 0 se todos os defeitos foram pegos e todos os controles passaram.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { subirCopiaComPatch } from './claude-interceptar.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');
const PORTA = 6230;                                   // real: +0/+1, proxy: +2
const so = process.argv.includes('--so') ? process.argv[process.argv.indexOf('--so') + 1] : null;

/** Troca EXATAMENTE uma ocorrencia, ou grita. */
const trocar = (de, para) => html => {
  const n = html.split(de).length - 1;
  if (n !== 1) throw new Error(`reposicao: esperava 1 ocorrencia de ${JSON.stringify(de.slice(0, 70))}, achei ${n}`);
  return html.replace(de, () => para);
};

// ------------------------------------------------------------ os defeitos
const DEFEITOS = [
  { caso: 'botoes-de-nova-tabela', nome: 'o «+ campo» do cartao nao faz nada',
    patch: trocar('c.corpo.querySelector("#ntMais").onclick = () => {', 'c.corpo.querySelector("#ntMais").onclick = () => { return;') },
  { caso: 'botoes-de-nova-tabela', nome: 'o «Cadastro completo…» leva um rascunho sem `indices_texto` (o defeito achado clicando)',
    patch: trocar('        indices_texto: [],\n      };\n      c.fechar();', '      };\n      c.fechar();') },
  { caso: 'botoes-de-nova-tabela', nome: 'o «+ indice» da tela cheia nao faz nada',
    patch: trocar('$("#nt_addIdx").onclick = () => {', '$("#nt_addIdx").onclick = () => { return;') },
  { caso: 'botoes-de-nova-tabela', nome: 'o «Voltar» da tela cheia nao volta',
    patch: trocar('$("#nt_voltar").onclick = ev => { ev.preventDefault(); gerirTabelas(db); };', '$("#nt_voltar").onclick = ev => { ev.preventDefault(); };') },
  { caso: 'botoes-do-job', nome: 'o «Rodar agora» da ficha nao roda',
    patch: trocar('$("#btJbRodar").onclick = () => rodarJob(x.nome);', '$("#btJbRodar").onclick = () => {};') },
  { caso: 'botoes-do-job', nome: 'o «Excluir job» ignora o confirm e apaga assim mesmo',
    patch: html => trocar('      if (!confirm(preencher(txt("tela.g_confirmar_excluir_job","Excluir o job {nome}? O histórico das corridas dele fica."), {nome: x.nome}))) return;',
                          '      confirm("x");')(html) },
  { caso: 'botoes-da-telemetria', nome: 'o «Atualizar agora» nao atualiza',
    patch: trocar('$("#tlmAgora").onclick = () => volta();', '$("#tlmAgora").onclick = () => {};') },
  { caso: 'botoes-da-telemetria', nome: 'a pausa nao para o relogio',
    patch: trocar('setInterval(() => { if (!estado.pausado) volta(); }, estado.periodo)', 'setInterval(() => { volta(); }, estado.periodo)') },
  { caso: 'botoes-da-telemetria', nome: 'o «ocultar legenda» nao oculta',
    patch: trocar('estado.legendaAberta = !estado.legendaAberta;', '') },
  { caso: 'botoes-da-claude', nome: 'o painel «o que vai subir» mostra a CHAVE INTEIRA',
    patch: trocar('cab["x-api-key"] = fim(c.chave) ||', 'cab["x-api-key"] = c.chave ||') },
  { caso: 'botoes-da-claude', nome: 'o «Desfazer esta rodada» nao desfaz',
    patch: trocar('onde.querySelector("#iaDesfazer").onclick = () => desfazer(onde, db);', 'onde.querySelector("#iaDesfazer").onclick = () => {};') },
  { caso: 'botoes-da-claude', nome: 'o «Diagrama ER em tela cheia» nao abre o diagrama',
    patch: trocar('onde.querySelector("#iaVerEr").onclick = () => telaDiagramaER(db);', 'onde.querySelector("#iaVerEr").onclick = () => {};') },
  // Pedido 636: a pintura tardia. Cada defeito tira UMA conferencia de posse.
  { caso: 'pintura-tardia', nome: 'o «Criar» do cartao repinta o diagrama sem conferir a posse (o defeito do pedido)',
    patch: trocar('        await montarArvore(false);\n        telaDiagramaER(db, vez);', '        await montarArvore(false);\n        telaDiagramaER(db, vezDoPainel());') },
  { caso: 'pintura-tardia', nome: 'o diagrama pinta o corpo final sem conferir a posse',
    patch: trocar('  if (!aindaNoPainel(minha)) return;\n  ER.db = db;', '  ER.db = db;') },
  { caso: 'pintura-tardia', nome: 'verSequencias (duas fases) pinta sem conferir',
    patch: trocar('  if (!aindaNoPainel(vez)) return;\n  folha(preencher(txt("tela.tt_sequencias_de"', '  folha(preencher(txt("tela.tt_sequencias_de"') },
  { caso: 'pintura-tardia', nome: 'verSessoes (duas fases) pinta sem conferir',
    patch: trocar('  if (!aindaNoPainel(vez)) return;\n  folha(txt("tela.se_titulo", "Sessões"), txt("tela.se_sub"', '  folha(txt("tela.se_titulo", "Sessões"), txt("tela.se_sub"') },
  { caso: 'pintura-tardia', nome: 'telaDbLink (duas fases, ramo «nenhuma ligacao») pinta sem conferir',
    patch: trocar('    if (!aindaNoPainel(vez)) return;\n    folha("DbLink", txt("tela.st_nenhuma_ligacao"', '    folha("DbLink", txt("tela.st_nenhuma_ligacao"') },
  { caso: 'pintura-tardia', nome: 'a grade de conteudo (escreve direto no painel) nao confere a posse',
    patch: trocar('  if (!aindaNoPainel(vez)) return;\n  est.esquemaAtual = e;', '  est.esquemaAtual = e;') },
  { caso: 'pintura-tardia', nome: 'verQuemSou (uma fase) pinta sem conferir',
    patch: trocar('  const q = await api("quem_sou");\n  if (!aindaNoPainel(vez)) return;', '  const q = await api("quem_sou");') },
  { caso: 'pintura-tardia', nome: 'telaMensagens (uma fase, direto no painel) pinta sem conferir',
    patch: trocar('    const m = await api("mensagens");\n    if (!aindaNoPainel(vez)) return;', '    const m = await api("mensagens");') },
  { caso: 'botoes-da-claude', nome: 'o «Remover a chave» nao remove',
    patch: trocar('$("#iaRemover").onclick = () => {', '$("#iaRemover").onclick = () => { return;') },
];

const CONTROLES = [...new Set(DEFEITOS.map(d => d.caso))];

// ------------------------------------------------------------- a maquina
async function carregarCaso(nome) {
  const dir = join(AQUI, 'casos');
  for (const f of readdirSync(dir).filter(f => f.endsWith('.mjs')).sort()) {
    const mod = await import(join(dir, f));
    if (mod.caso.nome === nome) return mod.caso;
  }
  throw new Error(`nao achei o caso ${nome}`);
}

/** Roda UM caso, num servidor novo, com ou sem defeito. Devolve a falha ou null. */
async function rodarUm(navegador, caso, patch, tema) {
  const phxsqld = join(RAIZ, 'target', 'release', 'phxsqld');
  const servidor = await subir({ phxsqld, portaDados: PORTA, portaWeb: PORTA + 1, log: () => {} });
  const copia = await subirCopiaComPatch({ portaOuvir: PORTA + 2, portaReal: PORTA + 1, patchCorpo: patch });
  const contexto = await navegador.newContext({ viewport: { width: 1600, height: 950 } });
  await contexto.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch { /* privada */ } }, tema);
  await contexto.route(u => /fonts\.(googleapis|gstatic)\.com/.test(typeof u === 'string' ? u : u.href), r => r.abort());
  const page = await contexto.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(e.message || String(e)));
  const ctx = { page, url: copia.url, tema, base: servidor.base, portaDados: PORTA, portaWeb: PORTA + 1,
                capturas: null, notas: [], nomeCaptura: n => `${caso.nome}-${tema}-${n}` };
  let falha = null;
  try {
    await caso.rodar(ctx);
    if (erros.length) throw new Error(`${erros.length} erro(s) de pagina: ${erros[0]}`);
  } catch (e) { falha = e; }
  await contexto.close();
  await copia.derrubar();
  await servidor.derrubar();
  return falha;
}

async function principal() {
  const navegador = await chromium.launch({ headless: true });
  let ruim = 0;
  const diz = (...a) => console.log(...a);
  try {
    for (const nome of CONTROLES) {
      if (so && !nome.includes(so)) continue;
      const caso = await carregarCaso(nome);
      const f = await rodarUm(navegador, caso, h => h, 'escuro');
      diz(`${f ? 'VERMELHO' : 'verde   '}  controle   ${nome}${f ? `\n      ${String(f.message).split('\n')[0]}` : ''}`);
      if (f) ruim++;
    }
    for (const d of DEFEITOS) {
      if (so && !d.caso.includes(so)) continue;
      const caso = await carregarCaso(d.caso);
      const f = await rodarUm(navegador, caso, d.patch, 'escuro');
      const pegou = !!f;
      diz(`${pegou ? 'RED pego ' : 'PASSOU!! '}  ${d.caso}: ${d.nome}${pegou ? `\n      -> ${String(f.message).split('\n')[0].slice(0, 200)}` : ''}`);
      if (!pegou) ruim++;
    }
  } finally {
    await navegador.close();
  }
  diz(ruim ? `\n${ruim} problema(s): defeito que o caso nao pegou, ou controle que nao passou.` : '\nTodos os defeitos pegos e todos os controles verdes.');
  return ruim ? 1 : 0;
}

principal().then(c => process.exit(c)).catch(e => { console.error(e.stack || e.message); process.exit(2); });
