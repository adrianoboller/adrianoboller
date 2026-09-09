#!/usr/bin/env node
/* As TRES baterias da integracao com a Claude, como roteiro VERSIONADO --
 * o pedido 231 de `docs/PENDENCIAS.md`.
 *
 * `docs/CLAUDE-IA.md` SS8 relatava tres baterias (43 + 31 + 5 provas) contra
 * um "servidor falso da API" que rodou numa sessao e morreu com ela: nenhum
 * roteiro ficou no repositorio, contra a lei da casa ("script que resolveu
 * algo nao pode morrer com a sessao"). Este arquivo, `testes-web/claude-falsa.mjs`
 * (o servidor falso) e `testes-web/claude-interceptar.mjs` (a reposicao de
 * defeito sem recompilar) sao o roteiro que faltava.
 *
 *     node testes-web/claude-bateria.mjs
 *
 * Chaves:
 *     --bateria 1|2|3   roda so uma bateria (o padrao roda as tres)
 *     --so <pedaco>     roda so as provas cuja CHAVE contem o pedaco
 *     --porta <n>       porta de dados do phxsqld (web = +1, falsa = +2)
 *     --capturas <dir>  screenshots de apoio (nao e' prova; e' registro)
 *     --binario <path>  outro `phxsqld` (padrao: o da arvore principal)
 *
 * O QUE ELA E: a prova de que o caminho inteiro da integracao -- da tela de
 * Configuracoes ao SSE pedaco a pedaco, da modelagem que cria tabela de
 * verdade a politica de seguranca da pagina -- funciona contra um servidor
 * que fala o CONTRATO da API (SS2 de `docs/CLAUDE-IA.md`), com o `phxsqld`
 * de VERDADE por tras (login, banco, protocolo -- nada de maquete).
 *
 * O QUE ELA NAO E: prova de que a Anthropic responde bem. Sem chave de
 * verdade nao ha como medir QUALIDADE de resposta -- so' o CAMINHO, que e'
 * exatamente o que a SS10 do documento ja dizia como limite.
 *
 * ATENCAO AO BINARIO: por ordem da rodada, este arquivo NUNCA compila nada.
 * Ele usa o `phxsqld` que ja existe em `target/release/` (por padrao, o da
 * arvore principal `/home/user/adrianoboller/phxsql`) -- se ele for mais
 * velho que `ui/claude.js` desta arvore, a bateria AVISA (nao pode recusar
 * como a `bateria.mjs` faz, porque o binario e' de OUTRA arvore de proposito
 * -- ver `docs/cognicao/` desta rodada). */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { existsSync, mkdirSync, statSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { subirFalsa } from './claude-falsa.mjs';
import { entrar, api, Falha, verdade, igual, contem } from './apoio.mjs';
import {
  definirIA, lerIA, abrirConfigClaude, abrirQuery, testarChave, abrirPainelIA,
  escolherReceita, definirDb, verEnvio, perguntarEEsperar, medirCrescimento, cenarioParaIA,
} from './claude-apoio.mjs';
import {
  interceptarPaginaPrincipal, csp_apenasSelf, corpo_criaSemConfirmar,
  corpo_chaveNoCorpoDoPedido, subirCopiaComPatch,
} from './claude-interceptar.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');
const ARVORE_PRINCIPAL = '/home/user/adrianoboller/phxsql';

// ------------------------------------------------------------------ chaves
const arg = (nome, padrao = null) => {
  const i = process.argv.indexOf(nome);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : padrao;
};
const opc = {
  bateria: arg('--bateria') ? Number(arg('--bateria')) : null,
  so: arg('--so'),
  porta: Number(arg('--porta', '6870')),
  capturas: arg('--capturas'),
  binario: arg('--binario', join(ARVORE_PRINCIPAL, 'target', 'release', 'phxsqld')),
};

const CORES = { ok: '\x1b[32m', mal: '\x1b[31m', fraco: '\x1b[90m', fim: '\x1b[0m' };
const diz = (...a) => console.log(...a);

// Uma chave de TESTE, nunca uma real -- tem o prefixo real da Anthropic (para
// exercitar exatamente o codigo que a tela usa em `fim()`, os quatro
// ultimos), mas o corpo e' obviamente fabricado. NUNCA aparece em nenhum
// artefato do repositorio fora deste arquivo, e o proprio arquivo nunca sobe
// para a Anthropic de verdade -- so' para o servidor FALSO, que roda em
// 127.0.0.1.
const CHAVE_DE_TESTE = 'sk-ant-teste-B1CDA9F03E7D1A6890FF2233';

// --------------------------------------------------------------- o registro
const provas = [];
let filtroSo = opc.so;

async function prova(bateria, chave, titulo, fn) {
  if (filtroSo && !chave.includes(filtroSo)) return;
  const t0 = Date.now();
  process.stdout.write(`  [b${bateria}] ${chave.padEnd(44)} `);
  try {
    const detalhe = await fn();
    const ms = Date.now() - t0;
    provas.push({ bateria, chave, titulo, veredito: 'ok', detalhe: detalhe ?? null, ms });
    diz(`${CORES.ok}ok${CORES.fim} (${ms} ms)`);
  } catch (e) {
    const ms = Date.now() - t0;
    const msg = e instanceof Falha ? e.message : (e && e.stack ? e.stack.split('\n').slice(0, 3).join(' | ') : String(e));
    provas.push({ bateria, chave, titulo, veredito: 'falhou', detalhe: msg, ms });
    diz(`${CORES.mal}FALHOU${CORES.fim} (${ms} ms)`);
    diz(`      ${CORES.mal}${msg}${CORES.fim}`);
  }
}

/** As DUAS pontas da prova real: falha com o defeito reposto, passa com o
 *  conserto. Cada chamada gera DUAS entradas em `provas`, uma por sentido --
 *  e' o formato que faz o `resultados.json` carregar as duas metades sem
 *  precisar de uma secao separada. */
async function provaDupla(bateria, chave, titulo, { comDefeito, semDefeito }) {
  await prova(bateria, `${chave}__com_defeito_reposto`, `${titulo} (defeito reposto: deve FALHAR)`, async () => {
    let falhouComoEsperado = false;
    let detalhe;
    try { detalhe = await comDefeito(); falhouComoEsperado = false; }
    catch (e) { falhouComoEsperado = true; detalhe = e instanceof Falha ? e.message : String(e && e.message || e); }
    verdade(falhouComoEsperado, 'com o defeito reposto a prova deveria FALHAR e passou -- teste que passa por engano e pior que teste que falta');
    return { comportamento_visto: detalhe };
  });
  await prova(bateria, `${chave}__com_o_conserto`, `${titulo} (conserto: deve PASSAR)`, semDefeito);
}

function conferirBinario(phxsqld) {
  if (!existsSync(phxsqld)) throw new Error(`nao achei ${phxsqld}`);
  const bin = statSync(phxsqld).mtimeMs;
  const uiRaiz = join(RAIZ, 'crates', 'phxsql-server', 'ui', 'claude.js');
  if (!existsSync(uiRaiz)) return { avisoBinario: null };
  const uiMt = statSync(uiRaiz).mtimeMs;
  if (uiMt > bin) {
    const aviso = `AVISO: ${phxsqld} e mais VELHO que ${uiRaiz} desta arvore -- `
      + `o binario pode nao servir o claude.js que esta sendo lido aqui. Esta `
      + `bateria roda mesmo assim (o binario vem de outra arvore, ver `
      + `docs/cognicao/), mas o achado abaixo pode nao valer para o codigo lido.`;
    diz(`${CORES.mal}${aviso}${CORES.fim}`);
    return { avisoBinario: aviso };
  }
  return { avisoBinario: null };
}

// =====================================================================
// BATERIA 1 -- o caminho inteiro
// =====================================================================
async function bateria1(navegador, servidor, falsa) {
  const contexto = await navegador.newContext({ viewport: { width: 1600, height: 950 }, bypassCSP: true });
  await contexto.addInitScript(() => { try { localStorage.setItem('phxsql-tema', 'escuro'); } catch { /* privada */ } });

  const pedidosAoPhxsql = [];
  contexto.on('request', req => {
    if (req.method() === 'POST' && req.url().endsWith('/api')) {
      pedidosAoPhxsql.push(req.postData() || '');
    }
  });

  const page = await contexto.newPage();
  const errosDePagina = [];
  page.on('pageerror', e => errosDePagina.push(String(e && e.message || e)));

  await entrar(page, servidor.url);
  const db = 'iaBat1';
  await cenarioParaIA(page, api, db);

  // -------------------------------------------------------- 1. guarda velha
  await prova(1, 'guarda_velha_sem_configuracao_nenhuma',
    'Sem chave nem ligado: a tela de Query fica exatamente como antes -- o botao da Claude nao e desenhado.',
    async () => {
      await abrirQuery(page);
      const temBotao = await page.$('#btIA');
      verdade(!temBotao, 'o botao "Perguntar a Claude" nao deveria existir sem configuracao');
      const temConsultar = await page.$('#btConsultar');
      verdade(!!temConsultar, 'a tela de Query precisa continuar com o Consultar de sempre');
    });

  await prova(1, 'guarda_velha_ligado_mas_sem_chave',
    '"ligado" sozinho nao basta -- ligada() exige chave E interruptor.',
    async () => {
      await definirIA(page, { ligado: true });
      await abrirQuery(page);
      const temBotao = await page.$('#btIA');
      verdade(!temBotao, 'ligado sem chave ainda nao deveria desenhar o botao');
      await definirIA(page, { ligado: false }); // devolve ao estado anterior
    });

  // ------------------------------------------------- 2. tela de configuracao
  await prova(1, 'config_tela_sem_chave_pino_oficial',
    'Sem endpoint configurado, a tela mostra o endereco OFICIAL com o pino verde.',
    async () => {
      await abrirConfigClaude(page);
      contem(await page.content(), 'Leia antes de ligar', 'falta o aviso de leitura');
      const pino = await page.$eval('.pino', el => ({ texto: el.textContent.trim(), classe: el.className }));
      contem(pino.classe, 'ok', 'sem endpoint configurado, o pino deveria ser o "ok" (oficial)');
      return pino;
    });

  await prova(1, 'config_salva_pela_tela_e_mascara',
    'Salvar pela FORMA de verdade grava a chave, o modelo e o interruptor; a tela volta so com os 4 ultimos digitos.',
    async () => {
      await abrirConfigClaude(page);
      await page.fill('#iaChave', CHAVE_DE_TESTE);
      await page.selectOption('#iaModelo', 'claude-haiku-4-5');
      await page.check('#iaLigado');
      await page.click('#iaSalvar');
      await page.waitForSelector('#aviso:not([hidden])', { timeout: 5000 });
      const avisoTopo = await page.$eval('#aviso', el => el.textContent);
      contem(avisoTopo, 'salva', 'o aviso do topo deveria confirmar o salvamento');
      // Reabre para conferir que so os 4 ultimos ficam visiveis -- e a chave
      // INTEIRA nao pode estar em lugar nenhum da pagina.
      await abrirConfigClaude(page);
      const legenda = await page.$eval('label:has(#iaChave) .leg', el => el.textContent);
      contem(legenda, CHAVE_DE_TESTE.slice(-4), 'a legenda deveria trazer os 4 ultimos digitos');
      const html = await page.content();
      verdade(!html.includes(CHAVE_DE_TESTE), 'a chave INTEIRA nao pode aparecer na pagina depois de salva');
      const cfg = await lerIA(page);
      igual(cfg.modelo, 'claude-haiku-4-5', 'o modelo salvo deveria ser o escolhido');
      igual(cfg.ligado, true, 'o interruptor salvo deveria estar ligado');
    });

  // O endpoint nao tem campo na tela (so' um <code> que MOSTRA) -- e' preciso
  // ir pela mesma gaveta que a tela le. Documentado no relatorio final desta
  // rodada.
  await definirIA(page, { endpoint: falsa.endpointMensagens });

  await prova(1, 'config_pino_fica_vermelho_com_endpoint_de_teste',
    'Apontado para o servidor falso, o pino muda para "NAO e o oficial".',
    async () => {
      await abrirConfigClaude(page);
      const pino = await page.$eval('.pino', el => ({ texto: el.textContent.trim(), classe: el.className }));
      contem(pino.classe, 'mal', 'com endpoint de teste o pino deveria ser o "mal" (nao oficial)');
      contem(pino.texto, 'NÃO', 'o pino deveria avisar que o endereco nao e o oficial');
      return pino;
    });

  await prova(1, 'config_remover_chave_desliga_tudo',
    'Remover apaga a chave e desliga o interruptor; o botao de remover fica desabilitado.',
    async () => {
      await abrirConfigClaude(page);
      await page.click('#iaRemover');
      await page.waitForSelector('#aviso:not([hidden])', { timeout: 5000 });
      const cfg = await lerIA(page);
      igual(cfg.chave, '', 'a chave deveria ter sido apagada');
      igual(cfg.ligado, false, 'o interruptor deveria ter desligado junto');
      const desabilitado = await page.$eval('#iaRemover', el => el.disabled);
      verdade(desabilitado, 'sem chave, o botao Remover deveria ficar desabilitado');
    });

  // Recompoe o estado para o resto da bateria 1.
  await definirIA(page, { chave: CHAVE_DE_TESTE, modelo: 'claude-haiku-4-5', ligado: true, endpoint: falsa.endpointMensagens });

  // --------------------------------------------------------- 3. Testar chave
  await prova(1, 'testar_chave_sucesso_mostra_resposta_e_tokens',
    '"Testar a chave" com sucesso mostra a resposta e os tokens de entrada/saida.',
    async () => {
      falsa.definirRoteiro({ resposta: 'sucesso', texto: 'ok', tokensEntrada: 4, tokensSaida: 1 });
      await abrirConfigClaude(page);
      const r = await testarChave(page);
      verdade(r.ok, `esperava sucesso, veio: ${r.texto}`);
      contem(r.texto, 'token(s)', 'a mensagem deveria trazer a contagem de tokens');
      return r;
    });

  for (const [codigo, pedaco] of [[401, 'não foi aceita'], [429, 'Limite de uso'], [400, 'recusou o pedido']]) {
    await prova(1, `testar_chave_erro_${codigo}`,
      `"Testar a chave" com ${codigo} mostra o recado certo, nao "erro" generico.`,
      async () => {
        falsa.definirRoteiro({ resposta: 'erro', codigo, mensagem: `mensagem de teste ${codigo}` });
        await abrirConfigClaude(page);
        const r = await testarChave(page);
        verdade(!r.ok, `esperava falha com ${codigo}`);
        contem(r.texto, pedaco, `o recado do ${codigo} deveria conter "${pedaco}"`);
        contem(r.texto, `mensagem de teste ${codigo}`, 'a mensagem da API deveria aparecer, analisada do JSON');
        return r;
      });
  }

  await prova(1, 'testar_chave_erro_529_sobrecarga',
    '529/5xx mostra "sobrecarregada", e diz que nao e culpa da chave nem do pedido.',
    async () => {
      falsa.definirRoteiro({ resposta: 'erro', codigo: 529, mensagem: 'overloaded de teste' });
      await abrirConfigClaude(page);
      const r = await testarChave(page);
      verdade(!r.ok, 'esperava falha com 529');
      contem(r.texto, 'sobrecarregada', 'deveria dizer que a API esta sobrecarregada');
    });

  await prova(1, 'testar_chave_rede_caida',
    'Endpoint que nao responde vira o recado de rede, citando o endereco configurado.',
    async () => {
      await definirIA(page, { endpoint: 'http://127.0.0.1:1/v1/messages' }); // porta fechada de proposito
      await abrirConfigClaude(page);
      const r = await testarChave(page, { timeout: 20000 });
      verdade(!r.ok, 'esperava falha de rede');
      contem(r.texto, 'alcançar a API', 'o recado de rede deveria citar a tentativa de alcancar a API');
      await definirIA(page, { endpoint: falsa.endpointMensagens }); // devolve
    });

  // ------------------------------------------------------- 4. painel de Query
  await prova(1, 'painel_query_botao_aparece_ligado',
    'Com chave e ligado, o botao "Perguntar a Claude" aparece na tela de Query.',
    async () => {
      await abrirQuery(page);
      await page.waitForSelector('#btIA', { timeout: 5000 });
      const leg = await page.$eval('.dbl-titulo .leg', el => el.textContent);
      contem(leg, 'não executa sozinho', 'a legenda deveria avisar que o SQL nao executa sozinho');
    });

  await prova(1, 'painel_o_que_vai_subir_mostra_corpo_e_chave_mascarada',
    'O painel "Ver o que vai subir" mostra o corpo exato do POST e os cabecalhos com a chave mascarada.',
    async () => {
      await abrirPainelIA(page);
      await escolherReceita(page, 'sql');
      await definirDb(page, db);
      await page.fill('#iaPergunta', 'os clientes de Blumenau');
      const envio = await verEnvio(page);
      contem(envio.corpo, '"model": "claude-haiku-4-5"', 'o corpo deveria trazer o modelo escolhido');
      contem(envio.corpo, '"stream": true', 'o corpo deveria pedir streaming');
      contem(envio.cabecalhos, '····' + CHAVE_DE_TESTE.slice(-4), 'os cabecalhos mostrados devem trazer so os 4 ultimos da chave');
      verdade(!envio.cabecalhos.includes(CHAVE_DE_TESTE), 'a chave INTEIRA nao pode aparecer no painel do que vai subir');
      return envio.resumo;
    });

  // ------------------------------------------------------- 5. receita SQL
  await prova(1, 'receita_sql_cai_no_editor_sem_executar',
    'O SQL que a Claude devolve cai no editor -- e NAO executa sozinho.',
    async () => {
      falsa.definirRoteiro({ resposta: 'sucesso', texto: 'SELECT nome, cidade FROM clientes', tokensEntrada: 30, tokensSaida: 8, pedacos: 3, atrasoMs: 20 });
      const crescimento = medirCrescimento(page, { janelaMs: 500 });
      const r = await perguntarEEsperar(page, 'os clientes cadastrados', { timeout: 15000 });
      await crescimento; // so' para nao deixar a promise solta
      verdade(!r.erro, `nao deveria ter dado erro: ${r.erro}`);
      igual(r.sql, 'SELECT nome, cidade FROM clientes', 'o SQL deveria cair no editor tal como veio');
      const semLinhas = await page.$('#iaResultado table, #iaResultado .vazio');
      verdade(!semLinhas, 'nada deveria ter executado ainda -- o editor so recebe o texto');
    });

  await prova(1, 'streaming_aparece_em_mais_de_um_pedaco',
    'O streaming pinta o texto aos pedacos -- mais de um tamanho distinto visto antes do final.',
    async () => {
      falsa.definirRoteiro({ resposta: 'sucesso', texto: 'SELECT id, nome FROM clientes ORDER BY id', tokensEntrada: 20, tokensSaida: 10, pedacos: 5, atrasoMs: 60 });
      await page.fill('#iaPergunta', 'todos os clientes em ordem');
      await page.click('#iaIr');
      const vistos = await medirCrescimento(page, { janelaMs: 1200, passoMs: 15 });
      await page.waitForFunction(() => document.querySelector('#iaTokens')?.querySelector('b'), undefined, { timeout: 10000 });
      const distintos = [...new Set(vistos)].filter(n => n >= 0);
      verdade(distintos.length >= 3, `esperava ver >=3 tamanhos distintos de texto crescendo, vi ${distintos.length}: ${distintos.join(',')}`);
      return { tamanhos_vistos: distintos };
    });

  await prova(1, 'executar_do_editor_traz_linhas_reais',
    'Clicar Executar roda o SQL de verdade pela operacao `sql` do protocolo, e traz linhas reais.',
    async () => {
      await page.click('#iaExecutar');
      await page.waitForSelector('#iaResultado .leg', { timeout: 10000 });
      const leg = await page.$eval('#iaResultado .leg', el => el.textContent);
      contem(leg, 'linha', 'a legenda deveria contar linhas devolvidas');
      const linhas = await page.$$eval('#iaGradeSql .phx-grid tbody tr:not(.phx-grupo)', trs => trs.length).catch(() => 0);
      verdade(linhas >= 2, `esperava pelo menos as 2 linhas do cenario, achei ${linhas}`);
      return { linhas };
    });

  await prova(1, 'recusa_do_motor_com_o_motivo_chega_a_tela',
    'SQL sem indice na coluna do WHERE volta com o motivo do MOTOR, nao um "erro" generico.',
    async () => {
      await page.fill('#iaSql', "SELECT * FROM clientes WHERE cidade = 'Blumenau'");
      await page.click('#iaExecutar');
      await page.waitForSelector('#iaResultado .aviso.mal', { timeout: 10000 });
      const msg = await page.$eval('#iaResultado .aviso.mal', el => el.textContent);
      const temMotivo = /indice|índice/i.test(msg);
      verdade(temMotivo, `a recusa deveria citar indice/índice como motivo, veio: ${msg}`);
      return msg;
    });

  // ------------------------------------------------------ 6. outras receitas
  await prova(1, 'receita_explicar_mostra_prosa_sem_editor',
    'A receita "Explicar o SQL" nao tem editor -- so mostra a explicacao.',
    async () => {
      await abrirPainelIA(page);
      await escolherReceita(page, 'explicar');
      await definirDb(page, db);
      falsa.definirRoteiro({ resposta: 'sucesso', texto: 'Esta consulta devolve nome e cidade, na ordem de digitação.', tokensEntrada: 15, tokensSaida: 12, pedacos: 2 });
      const r = await perguntarEEsperar(page, 'SELECT nome, cidade FROM clientes');
      verdade(!r.erro, `nao deveria ter dado erro: ${r.erro}`);
      contem(r.texto, 'ordem de digitação', 'a explicacao fake deveria ter chegado inteira');
      const temEditor = await page.$('#iaSql');
      verdade(!temEditor, 'a receita "explicar" nao tem editor de SQL');
    });

  await prova(1, 'receita_desempenho_instrui_o_modelo_a_medir',
    'O `system` da receita de indice OBRIGA a resposta a terminar mandando medir -- confere que a INSTRUCAO viaja no pedido.',
    async () => {
      await abrirPainelIA(page);
      await escolherReceita(page, 'desempenho');
      await definirDb(page, db);
      await page.fill('#iaPergunta', 'SELECT * FROM clientes WHERE cidade = \'Blumenau\'');
      const envio = await verEnvio(page);
      contem(envio.corpo, 'Isto é sugestão a MEDIR, não verdade', 'o system deveria trazer a instrucao obrigatoria');
      contem(envio.corpo, 'Não afirme ganho em número', 'o system deveria proibir afirmar ganho sem medir');
    });

  await prova(1, 'linhas_de_exemplo_saem_com_dado_pessoal_redigido',
    'Marcar "linhas de exemplo" manda dado real, mas a coluna de dado pessoal sai como "***".',
    async () => {
      await abrirPainelIA(page);
      await escolherReceita(page, 'sql');
      await definirDb(page, db);
      await page.check('#iaLinhas');
      await page.fill('#iaQtd', '5');
      await page.fill('#iaPergunta', 'liste os clientes');
      const envio = await verEnvio(page);
      contem(envio.resumo, 'valor(es) redigido', 'o resumo deveria contar os valores redigidos');
      verdade(!envio.resumo.includes('0 valor'), 'deveria ter havido pelo menos 1 valor redigido (a coluna email)');
      // A amostra vira `JSON.stringify(a)` (compacto, sem espaco apos ':')
      // DENTRO do campo `system`, que a serializacao de FORA escapa as aspas
      // dele -- por isso o texto exibido traz `\"email\":\"***\"` com a
      // barra invertida a mostra, e nao o JSON "bonito" de dois niveis.
      contem(envio.corpo, '\\"email\\":\\"***\\"', 'o corpo mandado deveria trazer o email redigido');
      verdade(!envio.corpo.includes('adriano@exemplo.org'), 'o email de verdade nao pode ter viajado no corpo');
      contem(envio.corpo, '\\"nome\\":\\"Adriano Boller\\"', 'colunas que nao sao dado pessoal continuam de verdade');
    });

  await prova(1, 'erro_no_meio_do_fluxo_e_detectado',
    'Um `event: error` no MEIO do streaming (HTTP 200) e tratado, e nao ignorado.',
    async () => {
      await abrirPainelIA(page);
      await escolherReceita(page, 'sql');
      await definirDb(page, db);
      falsa.definirRoteiro({ resposta: 'erro_no_meio', texto: 'SELECT nome F', pedacos: 2, atrasoMs: 30, tipoErro: 'overloaded_error', mensagemErro: 'parou no meio, de teste' });
      const r = await perguntarEEsperar(page, 'os clientes', { timeout: 10000 });
      verdade(!!r.erro, 'deveria ter reportado erro');
      contem(r.erro, 'interrompida', 'o recado deveria dizer que a resposta foi interrompida');
      contem(r.erro, 'parou no meio, de teste', 'a mensagem do event:error deveria aparecer');
    });

  // -------------------------------------------------------- 7. a chave nunca
  //    vaza para o PhxSql, e aparece de verdade so' para a Anthropic falsa
  await prova(1, 'chave_nunca_aparece_em_pedido_ao_phxsql',
    `Em ${pedidosAoPhxsql.length}+ pedidos /api desta corrida, a chave de teste nunca aparece.`,
    async () => {
      verdade(pedidosAoPhxsql.length > 5, `poucos pedidos /api capturados (${pedidosAoPhxsql.length}) -- a bateria mediu pouco`);
      const achou = pedidosAoPhxsql.some(c => c.includes(CHAVE_DE_TESTE));
      verdade(!achou, 'a chave apareceu num corpo de pedido ao servidor PhxSql');
      return { pedidos_conferidos: pedidosAoPhxsql.length };
    });

  await prova(1, 'chave_aparece_de_verdade_nos_pedidos_a_anthropic_falsa',
    'Sem isto a prova acima nao mediria nada: a chave TEM de aparecer, so que no lugar certo.',
    async () => {
      const vistos = falsa.lerPedidos();
      const achou = vistos.some(p => p.chaveVista === CHAVE_DE_TESTE);
      verdade(achou, 'a chave deveria ter aparecido no x-api-key de pelo menos um pedido a anthropic falsa');
      return { pedidos_na_falsa: vistos.length };
    });

  // --------------------------------- prova dupla: chave no corpo ao PhxSql
  //
  // O terceiro defeito da tabela da SS8: a `montarContexto` mandaria a chave
  // da Anthropic dentro do CORPO de um pedido `tabelas` ao PROPRIO PhxSql --
  // o oposto do que a SS3 promete ("a chave nao entra em nenhum pedido ao
  // PhxSql"). O patch entra pela mesma copia-por-proxy da bateria 2, e pela
  // mesma razao: o binario embute a tela, e um `route.fulfill()` no
  // documento quebraria a classificacao de endereco de loopback do Chromium
  // (ver a nota em `subirCopiaComPatch`).
  async function prepararQueryParaChaveNoCorpo({ patchCorpo, dbLocal }) {
    const ctx = await navegador.newContext({ viewport: { width: 1400, height: 900 }, bypassCSP: true });
    const capturados = [];
    ctx.on('request', req => {
      if (req.method() === 'POST' && req.url().endsWith('/api')) capturados.push(req.postData() || '');
    });
    let copia = null;
    let alvoUrl = servidor.url;
    if (patchCorpo) {
      copia = await subirCopiaComPatch({ portaOuvir: opc.porta + 4, portaReal: opc.porta + 1, patchCorpo });
      alvoUrl = copia.url;
    }
    const pg = await ctx.newPage();
    await entrar(pg, alvoUrl);
    // Base PROPRIA por lado da prova: os dois lados batem no MESMO `phxsqld`
    // real por tras (a copia so' reescreve o `/`, o `/api` e' repassado) --
    // reusar o mesmo nome faria o segundo lado colidir com "tabela ja existe".
    await cenarioParaIA(pg, api, dbLocal, 'clientes');
    await definirIA(pg, { chave: CHAVE_DE_TESTE, modelo: 'claude-haiku-4-5', ligado: true, endpoint: falsa.endpointMensagens });
    await abrirQuery(pg);
    await abrirPainelIA(pg);
    await escolherReceita(pg, 'sql');
    await definirDb(pg, dbLocal);
    await pg.fill('#iaPergunta', 'liste os clientes');
    await verEnvio(pg); // dispara `montarContexto`, que e' onde o defeito mora
    return { ctx, pg, copia, capturados };
  }

  function afirmarChaveNuncaApareceuNosPedidosApi(capturados) {
    verdade(capturados.length > 0, 'nenhum pedido /api foi capturado -- a prova nao mediu nada');
    const achou = capturados.some(c => c.includes(CHAVE_DE_TESTE));
    verdade(!achou, 'a chave apareceu num corpo de pedido POST /api ao servidor PhxSql');
  }

  await provaDupla(1, 'chave_no_corpo_de_pedido_ao_phxsql',
    'Defeito da SS8: a `montarContexto` mandava a chave da Anthropic dentro do corpo de um pedido `tabelas` ao PROPRIO PhxSql.',
    {
      comDefeito: async () => {
        const { ctx, pg, copia, capturados } = await prepararQueryParaChaveNoCorpo({
          patchCorpo: corpo_chaveNoCorpoDoPedido, dbLocal: 'iaBat1ChaveDefeito',
        });
        try { afirmarChaveNuncaApareceuNosPedidosApi(capturados); }
        finally { await pg.close(); await ctx.close(); if (copia) await copia.derrubar(); }
      },
      semDefeito: async () => {
        const { ctx, pg, capturados } = await prepararQueryParaChaveNoCorpo({
          patchCorpo: null, dbLocal: 'iaBat1ChaveConserto',
        });
        try { afirmarChaveNuncaApareceuNosPedidosApi(capturados); }
        finally { await pg.close(); await ctx.close(); }
      },
    });

  // ---------------------------------------------------------------- 8. temas
  for (const tema of ['escuro', 'claro']) {
    await prova(1, `tema_${tema}_sem_erro_de_pagina`,
      `O painel da Claude renderiza sem erro de pagina no tema ${tema}.`,
      async () => {
        await page.evaluate(t => { try { aplicarTema(t); } catch { /* enfeite */ } }, tema);
        await abrirPainelIA(page);
        await escolherReceita(page, 'sql');
        verdade(errosDePagina.length === 0, `erro(s) de pagina: ${errosDePagina.join(' | ')}`);
      });
  }

  if (opc.capturas) {
    mkdirSync(opc.capturas, { recursive: true });
    await page.screenshot({ path: join(opc.capturas, 'bateria1-painel.png'), fullPage: true }).catch(() => {});
  }

  await page.close();
  await contexto.close();
}

// =====================================================================
// BATERIA 2 -- a modelagem que cria
// =====================================================================

/** O plano de duas tabelas + um relacionamento que a maioria dos casos usa.
 *  `sufixo` evita colisao de nome entre o caso normal e o caso de defeito
 *  reposto, que corre num banco separado ao mesmo tempo. */
function planoValido(sufixo = '') {
  return JSON.stringify({
    tabelas: [
      { nome: `pedidosia${sufixo}`, porque: 'pedidos de teste da bateria',
        colunas: [
          { nome: 'id', tipo: 'Sequence', obrigatoria: true, caption: 'Código', dado_pessoal: 'nao' },
          { nome: 'cliente_id', tipo: 'Int4', obrigatoria: true, caption: 'Cliente', dado_pessoal: 'nao' },
        ],
        indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true, porque: 'chave primaria' }] },
      { nome: `itensia${sufixo}`, porque: 'itens de teste da bateria',
        colunas: [
          { nome: 'id', tipo: 'Sequence', obrigatoria: true, caption: 'Código', dado_pessoal: 'nao' },
          { nome: 'pedido_id', tipo: 'Int4', obrigatoria: true, caption: 'Pedido', dado_pessoal: 'nao' },
        ],
        indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true, porque: 'chave primaria' }],
        relacionamentos: [{ nome: `fk_pedido${sufixo}`, colunas: ['pedido_id'], tabela_ref: `pedidosia${sufixo}`,
          colunas_ref: ['id'], ao_excluir: 'restringir', ao_alterar: 'restringir', porque: 'item pertence a um pedido' }] },
    ],
    notas: ['plano de teste gerado pela bateria da Claude'],
  });
}

async function bateria2(navegador, servidor, falsa) {
  const contexto = await navegador.newContext({ viewport: { width: 1600, height: 950 }, bypassCSP: true });
  const page = await contexto.newPage();
  const errosDePagina = [];
  page.on('pageerror', e => errosDePagina.push(String(e && e.message || e)));
  await entrar(page, servidor.url);

  const db = 'iaBat2';
  await api(page, 'criar_database', { database: db }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: db, tabela: 'jaexiste',
    colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  }).catch(() => {});

  // ------------------------------------------------- 1. comportamento velho
  await prova(2, 'diagrama_er_sem_chave_funciona_como_antes',
    'Sem chave nenhuma configurada, o diagrama ER abre e desenha normalmente.',
    async () => {
      await page.evaluate(d => telaDiagramaER(d), db);
      await page.waitForSelector('.er-rolo, #painel svg, #painel .vazio', { timeout: 10000 }).catch(() => {});
      verdade(errosDePagina.length === 0, `erro(s) de pagina: ${errosDePagina.join(' | ')}`);
    });

  await prova(2, 'dicionario_sem_chave_funciona_como_antes',
    'Sem chave nenhuma configurada, o SysColumns (dicionario de dados) abre normalmente.',
    async () => {
      await page.evaluate(d => verSysColumns(d), db);
      await page.waitForSelector('#painel table, #painel .vazio', { timeout: 10000 }).catch(() => {});
      verdade(errosDePagina.length === 0, `erro(s) de pagina: ${errosDePagina.join(' | ')}`);
    });

  await definirIA(page, { chave: CHAVE_DE_TESTE, modelo: 'claude-haiku-4-5', ligado: true, endpoint: falsa.endpointMensagens });

  async function abrirModelar() {
    await abrirQuery(page);
    await abrirPainelIA(page);
    await escolherReceita(page, 'modelar');
    await definirDb(page, db);
  }

  async function perguntarModelarEEsperarRevisao(texto, { timeout = 20000 } = {}) {
    await page.fill('#iaPergunta', texto);
    await page.click('#iaIr');
    // NAO espera por qualquer `<h3>` -- `ir()` escreve um `<h3>Resposta</h3>`
    // de PLACEHOLDER antes mesmo do streaming comecar, e esse h3 casaria
    // primeiro, medindo a tela ainda vazia (a mesma licao do "relogio fixo
    // le texto cru" da SS9, por outro caminho). O efeito de verdade e' a
    // REVISAO (`.ia-item`) ou a recusa de formato (`.aviso.mal`).
    await page.waitForFunction(() => {
      const s = document.querySelector('#iaSaida');
      return !!(s && (s.querySelector('.ia-item') || s.querySelector('.aviso.mal')));
    }, undefined, { timeout });
  }

  // ------------------------------------------- 2. tipo inexistente recusado
  await prova(2, 'plano_com_tipo_inexistente_recusado_antes_de_criar',
    'Um tipo que o motor nao tem (ex: VARCHAR) trava o item na revisao, e nada e criado.',
    async () => {
      await abrirModelar();
      falsa.definirRoteiro({
        resposta: 'sucesso', tokensEntrada: 40, tokensSaida: 30, pedacos: 3,
        texto: JSON.stringify({ tabelas: [{ nome: 'comtipoerrado', porque: 'teste',
          colunas: [{ nome: 'id', tipo: 'Sequence', obrigatoria: true, dado_pessoal: 'nao' },
                    { nome: 'obs', tipo: 'VARCHAR(80)', obrigatoria: false, dado_pessoal: 'nao' }],
          indices: [] }], notas: [] }),
      });
      await perguntarModelarEEsperarRevisao('uma tabela com um campo de texto qualquer');
      const problema = await page.$eval('.ia-item .aviso.mal', el => el.textContent);
      contem(problema, 'não existe no PhxSql', 'o motivo deveria citar que o tipo nao existe');
      const check = await page.$eval('.ia-mt', el => ({ marcado: el.checked, desabilitado: el.disabled }));
      verdade(check.desabilitado && !check.marcado, 'o item com tipo invalido deveria estar travado e desmarcado');
      const t = await api(page, 'tabelas', { database: db });
      verdade(!(t.tabelas || []).map(x => x.toLowerCase()).includes('comtipoerrado'), 'nada deveria ter sido criado ainda');
    });

  // --------------------------------------------- 3. colide com tabela existente
  await prova(2, 'plano_que_colide_com_tabela_existente_e_recusado',
    'Propor de novo uma tabela que ja existe trava o item dizendo isso, sem propor ALTER.',
    async () => {
      await abrirModelar();
      falsa.definirRoteiro({
        resposta: 'sucesso', tokensEntrada: 20, tokensSaida: 20, pedacos: 2,
        texto: JSON.stringify({ tabelas: [{ nome: 'jaexiste', porque: 'teste de colisao',
          colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true, dado_pessoal: 'nao' }], indices: [] }], notas: [] }),
      });
      await perguntarModelarEEsperarRevisao('cria de novo a tabela jaexiste');
      const problema = await page.$eval('.ia-item .aviso.mal', el => el.textContent);
      contem(problema, 'JÁ EXISTE', 'o motivo deveria dizer que a tabela ja existe');
      contem(problema, 'não tem ALTER de coluna', 'o motivo deveria explicar por que nao da para alterar');
    });

  // --------------------------------------------------- 4. resposta sem plano
  await prova(2, 'resposta_que_nao_e_plano_json_e_recusada',
    'Se a Claude devolver prosa em vez de JSON, a tela avisa do formato e mostra o texto cru.',
    async () => {
      await abrirModelar();
      falsa.definirRoteiro({ resposta: 'sucesso', tokensEntrada: 15, tokensSaida: 15, pedacos: 2,
        texto: 'Claro, aqui está uma explicação em português sobre como modelar isso.' });
      await perguntarModelarEEsperarRevisao('modele alguma coisa');
      const aviso = await page.$eval('#iaSaida .aviso.mal', el => el.textContent);
      contem(aviso, 'formato de plano', 'deveria dizer que a resposta nao veio no formato esperado');
      contem(await page.textContent('#iaSaida'), 'O que a Claude respondeu', 'deveria mostrar o texto cru recebido');
    });

  // ------------------------- 5/6. contagem antes de escrever + criacao real
  let dbAntesDeCriar = null;
  await prova(2, 'revisao_mostra_contagem_antes_de_escrever_nada',
    'A revisao conta "2 tabela(s)" e "1 relacionamento(s)" ANTES de qualquer escrita no banco.',
    async () => {
      await abrirModelar();
      falsa.definirRoteiro({ resposta: 'sucesso', tokensEntrada: 60, tokensSaida: 80, pedacos: 4, texto: planoValido() });
      await perguntarModelarEEsperarRevisao('uma loja com pedidos e itens de pedido');
      const conta = await page.$eval('#iaConta', el => el.textContent);
      const contaFk = await page.$eval('#iaContaFk', el => el.textContent);
      contem(conta, '2 tabela', 'deveria contar as duas tabelas do plano');
      contem(contaFk, '1 relacionamento', 'deveria contar o relacionamento do plano');
      dbAntesDeCriar = await api(page, 'tabelas', { database: db });
      verdade(!(dbAntesDeCriar.tabelas || []).some(t => /^pedidosia$|^itensia$/i.test(t)),
        'nenhuma tabela do plano deveria existir antes do clique em Criar');
    });

  await prova(2, 'criacao_confirmada_cria_de_verdade',
    'Clicar "Criar o que esta marcado" grava as tabelas e o relacionamento, com o resultado por item.',
    async () => {
      await page.click('#iaCriar');
      await page.waitForSelector('#iaNascido h3', { timeout: 15000 });
      const resumo = await page.$eval('#iaNascido .aviso', el => el.textContent);
      contem(resumo, '3 de 3', 'deveria ter criado os 3 itens (2 tabelas + 1 fk)');
      const itens = await page.$$eval('#iaNascido li', els => els.map(e => e.textContent));
      verdade(itens.every(t => !t.includes('×')), `algum item falhou: ${itens.join(' | ')}`);
    });

  // ------------------------------------------ 7/8. verdade pelo `esquema`
  await prova(2, 'tabelas_existem_de_verdade_provado_pela_operacao_esquema',
    'A prova NAO e a tela: e a operacao `esquema`, direto no motor.',
    async () => {
      const e = await api(page, 'esquema', { database: db, tabela: 'pedidosia' });
      igual(e.tabela, 'pedidosia', 'o esquema deveria confirmar o nome da tabela');
      const nomes = e.colunas.map(c => c.nome);
      contem(nomes.join(','), 'cliente_id', 'a coluna cliente_id deveria ter sido criada de verdade');
    });

  await prova(2, 'indices_e_marcacao_de_dado_pessoal_gravados',
    'O indice primario e a marcacao dado_pessoal do plano chegam gravados no esquema.',
    async () => {
      const e = await api(page, 'esquema', { database: db, tabela: 'pedidosia' });
      verdade(e.indices.some(i => i.primario), 'deveria haver um indice primario gravado');
      const col = e.colunas.find(c => c.nome === 'id');
      igual(col.dado_pessoal, 'nao', 'a marcacao dado_pessoal deveria ter chegado no esquema (mesmo "nao")');
    });

  await prova(2, 'fk_declarada_recusa_filha_orfa_de_verdade',
    'A FK que a IA declarou ja e imposta pelo motor: filha sem pai e recusada, pai com filha nao se apaga.',
    async () => {
      let recusouInsercao = false;
      try { await api(page, 'inserir', { database: db, tabela: 'itensia', valores: [999, 12345] }); }
      catch { recusouInsercao = true; }
      verdade(recusouInsercao, 'inserir item apontando para um pedido inexistente deveria ser recusado');
    });

  // --------------------------------------------------- 9. diagrama mostra
  await prova(2, 'diagrama_e_dicionario_mostram_o_que_nasceu',
    'Depois de criar, o painel "o modelo agora" mostra as tabelas e o relacionamento recem-criados.',
    async () => {
      // O resumo e' `insertAdjacentHTML("afterend", ...)` DEPOIS de `#iaEr`
      // -- ele e' irmao dele, nao filho; o descendente `#iaEr p.leg` nunca
      // acha nada. O pai comum e' `#iaNascido`.
      await page.waitForSelector('#iaNascido p.leg', { timeout: 10000 });
      const resumo = await page.$eval('#iaNascido p.leg', el => el.textContent);
      verdade(/[1-9]\d* tabela/.test(resumo), `o resumo do diagrama deveria contar pelo menos 1 tabela: ${resumo}`);
      verdade(/[1-9]\d* relacionamento/.test(resumo), `o resumo deveria contar pelo menos 1 relacionamento: ${resumo}`);
    });

  // ---------------------------------------- 10. segunda rodada ja colide
  await prova(2, 'segunda_rodada_do_mesmo_plano_ja_colide',
    'Rodar o MESMO plano de novo trava as duas tabelas como "ja existe".',
    async () => {
      await abrirModelar();
      falsa.definirRoteiro({ resposta: 'sucesso', tokensEntrada: 60, tokensSaida: 80, pedacos: 3, texto: planoValido() });
      await perguntarModelarEEsperarRevisao('a mesma loja de novo');
      const problemas = await page.$$eval('.ia-item .aviso.mal', els => els.map(e => e.textContent));
      verdade(problemas.length >= 2, `esperava as 2 tabelas travadas por colisao, achei ${problemas.length}`);
      verdade(problemas.every(p => p.includes('JÁ EXISTE')), 'todas deveriam dizer que ja existem');
    });

  // -------------------------------------------------------- 11. desfazer
  await prova(2, 'desfazer_remove_o_que_esta_rodada_criou',
    'O botao Desfazer remove, pelas mesmas operacoes de excluir, o que a rodada criou.',
    async () => {
      // A revisao acima ficou parada na colisao -- refaz um plano SO com uma
      // tabela nova para o desfazer ter algo limpo (sem dado) para remover.
      await abrirModelar();
      falsa.definirRoteiro({ resposta: 'sucesso', tokensEntrada: 20, tokensSaida: 20, pedacos: 2,
        texto: JSON.stringify({ tabelas: [{ nome: 'paradesfazer', porque: 'teste do desfazer',
          colunas: [{ nome: 'id', tipo: 'Sequence', obrigatoria: true, dado_pessoal: 'nao' }],
          indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }] }], notas: [] }) });
      await perguntarModelarEEsperarRevisao('uma tabela qualquer soh para desfazer depois');
      await page.click('#iaCriar');
      await page.waitForSelector('#iaNascido h3', { timeout: 15000 });
      const antes = await api(page, 'tabelas', { database: db });
      verdade((antes.tabelas || []).some(t => t.toLowerCase() === 'paradesfazer'), 'a tabela deveria existir antes do desfazer');
      await page.click('#iaDesfazer');
      await page.waitForSelector('#iaDesfeito .aviso', { timeout: 10000 });
      const depois = await api(page, 'tabelas', { database: db });
      verdade(!(depois.tabelas || []).some(t => t.toLowerCase() === 'paradesfazer'), 'a tabela deveria ter sumido depois do desfazer');
    });

  await page.close();
  await contexto.close();

  // ------------------------------------- prova dupla: criar sem confirmacao
  async function afirmarNadaCriadoLogoAposRevisao(pagina, dbAlvo, tabelaAlvo) {
    // O mesmo cuidado de `perguntarModelarEEsperarRevisao`: o `<h3>Resposta</h3>`
    // e' um placeholder que `ir()` escreve ANTES do streaming comecar -- esperar
    // por ele mediria a tela vazia, antes mesmo de a revisao existir.
    await pagina.waitForFunction(() => {
      const s = document.querySelector('#iaSaida');
      return !!(s && (s.querySelector('.ia-item') || s.querySelector('.aviso.mal')));
    }, undefined, { timeout: 15000 });
    // Tempo para um clique AUTOMATICO (o defeito reposto) terminar de criar,
    // se ele existir -- sem isto o teste correria antes do `criarDoPlano`
    // assincrono ter tido chance de gravar.
    await pagina.waitForTimeout(1200);
    const t = await api(pagina, 'tabelas', { database: dbAlvo });
    const existe = (t.tabelas || []).some(n => n.toLowerCase() === tabelaAlvo.toLowerCase());
    verdade(!existe, `a tabela "${tabelaAlvo}" ja existe logo apos a revisao aparecer -- ninguem clicou em Criar ainda`);
  }

  async function prepararAteRevisao({ patchCorpo, dbAlvo, tabelaAlvo }) {
    const ctx = await navegador.newContext({ viewport: { width: 1400, height: 900 }, bypassCSP: true });
    // NAO usa `interceptarPaginaPrincipal` aqui -- medido nesta rodada:
    // `route.fetch()+fulfill()` no documento faz o Chromium classificar a
    // pagina como de "unknown address space" para Private Network Access, e
    // TODA chamada seguinte a outro endereco de loopback (a falsa da
    // Anthropic) e recusada por CORS -- mesmo os dois sendo 127.0.0.1. Um
    // proxy reverso de verdade (`subirCopiaComPatch`), acessado por conexao
    // DIRETA, nao sofre disso. Ver `docs/cognicao/` desta rodada.
    let copia = null;
    let alvoUrl = servidor.url;
    if (patchCorpo) {
      copia = await subirCopiaComPatch({ portaOuvir: opc.porta + 3, portaReal: opc.porta + 1, patchCorpo });
      alvoUrl = copia.url;
    }
    const pg = await ctx.newPage();
    await entrar(pg, alvoUrl);
    await api(pg, 'criar_database', { database: dbAlvo }).catch(() => {});
    await definirIA(pg, { chave: CHAVE_DE_TESTE, modelo: 'claude-haiku-4-5', ligado: true, endpoint: falsa.endpointMensagens });
    await abrirQuery(pg);
    await pg.click('#btIA');
    await pg.waitForSelector('#iaPainel .ia-rec');
    await pg.click('.ia-rec[data-r="modelar"]');
    await pg.waitForSelector('#iaPergunta');
    await pg.fill('#iaDb', dbAlvo);
    falsa.definirRoteiro({ resposta: 'sucesso', tokensEntrada: 20, tokensSaida: 20, pedacos: 2,
      texto: JSON.stringify({ tabelas: [{ nome: tabelaAlvo, porque: 'prova dupla da bateria 2',
        colunas: [{ nome: 'id', tipo: 'Sequence', obrigatoria: true, dado_pessoal: 'nao' }],
        indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }] }], notas: [] }) });
    await pg.fill('#iaPergunta', 'uma tabela qualquer para a prova dupla');
    await pg.click('#iaIr');
    return { ctx, pg, copia };
  }

  await provaDupla(2, 'criar_do_plano_sem_confirmacao',
    'Defeito historico da SS8: a tela criava a tabela sem esperar o clique em "Criar o que esta marcado".',
    {
      comDefeito: async () => {
        const { ctx, pg, copia } = await prepararAteRevisao({
          patchCorpo: corpo_criaSemConfirmar, dbAlvo: 'iaBat2Defeito', tabelaAlvo: 'semconfirmar',
        });
        try { await afirmarNadaCriadoLogoAposRevisao(pg, 'iaBat2Defeito', 'semconfirmar'); }
        finally { await ctx.close(); if (copia) await copia.derrubar(); }
      },
      semDefeito: async () => {
        const { ctx, pg, copia } = await prepararAteRevisao({
          dbAlvo: 'iaBat2Conserto', tabelaAlvo: 'semconfirmar',
        });
        try { await afirmarNadaCriadoLogoAposRevisao(pg, 'iaBat2Conserto', 'semconfirmar'); }
        finally { await ctx.close(); if (copia) await copia.derrubar(); }
      },
    });
}

// =====================================================================
// BATERIA 3 -- a politica real (o CSP da pagina)
// =====================================================================

const CORPO_OFICIAL_FALSO = JSON.stringify({
  id: 'msg_falsa_oficial', type: 'message', role: 'assistant', model: 'claude-falsa',
  content: [{ type: 'text', text: 'ok' }], stop_reason: 'end_turn', stop_sequence: null,
  usage: { input_tokens: 3, output_tokens: 1 },
});

function ehPedidoOficial(url) {
  try { return new URL(url).href === 'https://api.anthropic.com/v1/messages'; }
  catch { return false; }
}

async function bateria3(navegador, servidor) {
  // -------------------------------------------------- 1-3: sem bypassCSP
  const contexto = await navegador.newContext({ viewport: { width: 1400, height: 900 } }); // SEM bypassCSP: a CSP de verdade vale
  const capturados = [];
  await contexto.route(ehPedidoOficial, async rota => {
    capturados.push({ headers: rota.request().headers(), body: rota.request().postData() });
    await rota.fulfill({ status: 200, contentType: 'application/json', body: CORPO_OFICIAL_FALSO });
  });

  let cspDaPagina = null;
  const page = await contexto.newPage();
  page.on('response', r => {
    if (!cspDaPagina && r.request().resourceType() === 'document') {
      cspDaPagina = r.headers()['content-security-policy'] || null;
    }
  });
  await entrar(page, servidor.url);
  await definirIA(page, { chave: CHAVE_DE_TESTE, modelo: 'claude-haiku-4-5', ligado: true }); // endpoint fica no OFICIAL, de proposito

  await prova(3, 'csp_da_pagina_inclui_a_origem_oficial',
    'O Content-Security-Policy da pagina lista https://api.anthropic.com no connect-src.',
    async () => {
      verdade(!!cspDaPagina, 'nao consegui capturar o cabecalho CSP da navegacao principal');
      contem(cspDaPagina, "connect-src 'self' https://api.anthropic.com", 'o connect-src deveria trazer a origem oficial');
      return cspDaPagina;
    });

  await prova(3, 'sem_bypasscsp_a_chamada_sai_para_o_endereco_oficial',
    'Sem bypassCSP nenhum, "Testar a chave" alcanca https://api.anthropic.com/v1/messages -- a politica NAO barra.',
    async () => {
      await abrirConfigClaude(page);
      const r = await testarChave(page, { timeout: 15000 });
      verdade(r.ok, `esperava sucesso contra o endpoint oficial interceptado, veio: ${r.texto}`);
      verdade(capturados.length >= 1, 'a rota do endereco oficial deveria ter recebido o pedido');
    });

  await prova(3, 'a_chamada_leva_chave_cabecalho_de_navegador_e_versao',
    'O pedido capturado tem x-api-key, anthropic-version e o cabecalho de acesso direto do navegador.',
    async () => {
      const ultimo = capturados[capturados.length - 1];
      igual(ultimo.headers['x-api-key'], CHAVE_DE_TESTE, 'o x-api-key deveria ser a chave configurada');
      igual(ultimo.headers['anthropic-version'], '2023-06-01', 'a versao da API deveria ser a fixada no codigo');
      igual(ultimo.headers['anthropic-dangerous-direct-browser-access'], 'true', 'o cabecalho de acesso do navegador deveria ir "true"');
    });

  await page.close();
  await contexto.close();

  // --------------------------------- 4/5: prova dupla do connect-src 'self'
  async function prepararConfigSemBypass({ patchCsp }) {
    const ctx = await navegador.newContext({ viewport: { width: 1200, height: 800 } }); // sem bypassCSP
    if (patchCsp) await interceptarPaginaPrincipal(ctx, { patchCsp });
    const capt = [];
    await ctx.route(ehPedidoOficial, async rota => {
      capt.push(true);
      await rota.fulfill({ status: 200, contentType: 'application/json', body: CORPO_OFICIAL_FALSO });
    });
    const pg = await ctx.newPage();
    await entrar(pg, servidor.url);
    await definirIA(pg, { chave: CHAVE_DE_TESTE, modelo: 'claude-haiku-4-5', ligado: true });
    await abrirConfigClaude(pg);
    return { ctx, pg, capt };
  }

  /** A mesma afirmacao dos dois lados: "Testar a chave" da certo contra o
   *  oficial. Com o `connect-src 'self'` reposto, o `fetch` e barrado pela
   *  CSP e o `#iaRecado` nunca vira "bom" -- o `waitForFunction` estoura por
   *  TEMPO, que e' exatamente o sintoma que o comentario do teste Rust ja
   *  descrevia ("a tela so fica parada"). Nao ha um segundo caminho aqui: e'
   *  a MESMA funcao, rodada duas vezes. */
  async function afirmarTestarChaveDaCertoContraOOficial(pg, capt) {
    await pg.click('#iaTestar');
    await pg.waitForFunction(() => {
      const el = document.querySelector('#iaRecado .aviso');
      return !!el && el.classList.contains('bom');
    }, undefined, { timeout: 6000 });
    verdade(capt.length >= 1, 'a rota do endereco oficial deveria ter recebido o pedido');
  }

  await provaDupla(3, 'connect_src_apenas_self',
    'Defeito da SS8: `connect-src \'self\'` sozinho mata a chamada ANTES de ela sair, sem erro visivel ao script.',
    {
      comDefeito: async () => {
        const { ctx, pg, capt } = await prepararConfigSemBypass({ patchCsp: csp_apenasSelf });
        try { await afirmarTestarChaveDaCertoContraOOficial(pg, capt); }
        finally { await ctx.close(); }
      },
      semDefeito: async () => {
        const { ctx, pg, capt } = await prepararConfigSemBypass({ patchCsp: null });
        try { await afirmarTestarChaveDaCertoContraOOficial(pg, capt); }
        finally { await ctx.close(); }
      },
    });
}

// =====================================================================
// ORQUESTRACAO
// =====================================================================
async function main() {
  const t0Total = Date.now();
  const { avisoBinario } = conferirBinario(opc.binario);

  const versao = spawnSync(opc.binario, ['--version'], { encoding: 'utf8' });
  const versaoTexto = (versao.stdout || versao.stderr || '').trim() || '(nao consegui ler --version)';

  diz(`${CORES.fraco}phxsqld: ${opc.binario}${CORES.fim}`);
  diz(`${CORES.fraco}versao:  ${versaoTexto}${CORES.fim}`);

  const servidor = await subir({
    phxsqld: opc.binario, portaDados: opc.porta, portaWeb: opc.porta + 1,
    log: m => diz(`${CORES.fraco}· ${m}${CORES.fim}`),
  });
  const falsa = await subirFalsa({
    porta: opc.porta + 2,
    log: m => diz(`${CORES.fraco}· ${m}${CORES.fim}`),
  });

  const navegador = await chromium.launch({ headless: true });

  try {
    if (!opc.bateria || opc.bateria === 1) {
      diz(`\n${CORES.fraco}== bateria 1 -- o caminho inteiro ==${CORES.fim}`);
      falsa.limpar();
      await bateria1(navegador, servidor, falsa);
    }
    if (!opc.bateria || opc.bateria === 2) {
      diz(`\n${CORES.fraco}== bateria 2 -- a modelagem que cria ==${CORES.fim}`);
      falsa.limpar();
      await bateria2(navegador, servidor, falsa);
    }
    if (!opc.bateria || opc.bateria === 3) {
      diz(`\n${CORES.fraco}== bateria 3 -- a politica real ==${CORES.fim}`);
      falsa.limpar();
      await bateria3(navegador, servidor);
    }
  } finally {
    await navegador.close();
    await falsa.derrubar();
    await servidor.derrubar();
    diz(`${CORES.fraco}· servidor pid ${servidor.pid} e falsa derrubados${CORES.fim}`);
  }

  const porBateria = {};
  for (const p of provas) {
    (porBateria[p.bateria] ||= []).push(p);
  }
  const resumoBaterias = Object.entries(porBateria).map(([n, lista]) => ({
    numero: Number(n),
    total: lista.length,
    ok: lista.filter(p => p.veredito === 'ok').length,
    falharam: lista.filter(p => p.veredito === 'falhou').length,
  }));

  const resultado = {
    quando: new Date().toISOString(),
    pedido: 231,
    binario: { caminho: opc.binario, versao: versaoTexto, aviso: avisoBinario },
    falsa_api: { arquivo: 'testes-web/claude-falsa.mjs' },
    resumo_por_bateria: resumoBaterias,
    provas,
    tempo_total_ms: Date.now() - t0Total,
  };
  const saidaJson = join(AQUI, 'claude-resultados.json');
  writeFileSync(saidaJson, JSON.stringify(resultado, null, 2) + '\n');
  diz(`\n${CORES.fraco}· resultado gravado em ${saidaJson}${CORES.fim}`);

  const maus = provas.filter(p => p.veredito === 'falhou');
  diz(`\n${provas.length - maus.length}/${provas.length} provas passaram`);
  for (const r of resumoBaterias) diz(`  bateria ${r.numero}: ${r.ok}/${r.total}`);
  if (maus.length) {
    diz(`${CORES.mal}${maus.length} falharam:${CORES.fim}`);
    for (const m of maus) diz(`  [b${m.bateria}] ${m.chave} -- ${m.detalhe}`);
  }
  return maus.length ? 1 : 0;
}

main()
  .then(c => process.exit(c))
  .catch(e => { console.error(`${CORES.mal}${e.stack || e.message}${CORES.fim}`); process.exit(2); });

