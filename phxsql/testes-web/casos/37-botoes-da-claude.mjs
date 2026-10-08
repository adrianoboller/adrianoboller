/* Os botoes da integracao com a CLAUDE -- sem chave de API de verdade.
 *
 * Doze botoes que pediam chave (pedido 190). A pergunta em aberto era
 * «interceptar a rota ou dispensa registrada?», e a resposta e INTERCEPTAR:
 * o `claude.js` fala com `https://api.anthropic.com/v1/messages` direto do
 * navegador, e o Playwright fica no meio dessa chamada (`page.route`) e
 * responde por ela com o fluxo SSE do contrato. Nada sai da maquina e o
 * `phxsqld` de verdade continua por tras (login, banco, `sql`, `criar_tabela`).
 * A chave daqui e FABRICADA e so existe neste arquivo -- nunca uma real, nunca
 * em log nem em arquivo.
 *
 * Ao contrario da `claude-bateria.mjs` (servidor falso, ponta a ponta,
 * separada), este caso roda DENTRO da bateria, e por isso e o que alimenta
 * `botoes-exercitados.txt`:
 *
 *   telaConfig         #iaSalvar #iaTestar #iaRemover
 *   botaoDaConsulta    #btIA
 *   painelDaConsulta   [data-r]
 *   desenharReceita    #iaVer #iaIr #iaExecutar
 *   desenharRevisao    #iaCriar
 *   criarDoPlano       #iaVerDic #iaVerEr #iaDesfazer
 *
 * Cada clique e conferido pelo EFEITO: o que ficou na gaveta da chave, o que
 * subiu pelo fio (a chave vai para a Anthropic e NUNCA para o `phxsqld`), as
 * linhas que o motor devolveu, a tabela que existe -- ou deixou de existir --
 * no servidor.
 */
import { entrar, api, verdade, igual, contem, capturar, bancoDoCaso } from '../apoio.mjs';
import { definirIA, lerGavetas, abrirConfigClaude, abrirQuery, testarChave,
         abrirPainelIA, escolherReceita, definirDb, verEnvio, cenarioParaIA } from '../claude-apoio.mjs';

// Fabricada: tem a forma da real (o codigo da tela corta os quatro ultimos)
// e o corpo e obviamente de mentira.
const CHAVE = 'sk-ant-teste-FABRICADA-37A1B2C3D4E5';
const URL_API = 'https://api.anthropic.com/v1/messages';

/** O fluxo SSE do contrato, de uma vez so (o navegador o le do mesmo jeito). */
function sse(texto, { entrada = 12, saida = 7 } = {}) {
  const ev = (tipo, dados) => `event: ${tipo}\ndata: ${JSON.stringify(dados)}\n\n`;
  const meio = Math.max(1, Math.ceil(texto.length / 2));
  return ev('message_start', { type: 'message_start', message: {
      id: 'msg_roteiro', type: 'message', role: 'assistant', model: 'roteiro', content: [],
      stop_reason: null, stop_sequence: null, usage: { input_tokens: entrada, output_tokens: 0 } } })
    + ev('content_block_start', { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } })
    + ev('content_block_delta', { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: texto.slice(0, meio) } })
    + ev('content_block_delta', { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: texto.slice(meio) } })
    + ev('content_block_stop', { type: 'content_block_stop', index: 0 })
    + ev('message_delta', { type: 'message_delta', delta: { stop_reason: 'end_turn', stop_sequence: null }, usage: { output_tokens: saida } })
    + ev('message_stop', { type: 'message_stop' });
}

const plano = (...nomes) => JSON.stringify({
  tabelas: nomes.map(nome => ({
    nome, porque: 'tabela do caso',
    colunas: [{ nome: 'id', tipo: 'Sequence', obrigatoria: true, caption: 'Codigo', dado_pessoal: 'nao' }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  })),
  notas: [],
});

export const caso = {
  nome: 'botoes-da-claude',
  async rodar(ctx) {
    const { page } = ctx;
    const db = bancoDoCaso(ctx, 'ia');
    const sufixo = ctx.tema === 'claro' ? 'c' : 'e';

    // ------------------------------------------------ a rota interceptada
    const fila = [];                       // respostas roteirizadas, em ordem
    const subiram = [];                    // o que CHEGOU na «Anthropic»
    const aoPhxsql = [];                   // o que chegou no phxsqld
    const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*',
                   'access-control-allow-methods': 'POST, OPTIONS' };
    await page.route(URL_API, async rota => {
      const req = rota.request();
      if (req.method() === 'OPTIONS') { await rota.fulfill({ status: 204, headers: CORS }); return; }
      subiram.push({ cabecalhos: req.headers(), corpo: req.postData() || '' });
      const r = fila.shift() || { texto: 'ok' };
      if (r.status && r.status !== 200) {
        await rota.fulfill({ status: r.status, headers: { ...CORS, 'content-type': 'application/json' },
          body: JSON.stringify({ type: 'error', error: { type: r.tipo || 'api_error', message: r.mensagem || 'erro' } }) });
        return;
      }
      await rota.fulfill({ status: 200, headers: { ...CORS, 'content-type': 'text/event-stream; charset=utf-8',
        'cache-control': 'no-cache' }, body: sse(r.texto) });
    });
    page.on('request', req => {
      if (req.method() === 'POST' && req.url().endsWith('/api')) aoPhxsql.push(req.postData() || '');
    });

    await entrar(page, ctx.url);
    await cenarioParaIA(page, api, db);

    // ---------------------------------------------------------- configuracao
    await abrirConfigClaude(page);
    verdade(await page.$eval('#iaRemover', el => el.disabled), 'sem chave, o Remover devia estar desabilitado');

    // Salvar PELA FORMA: a chave fica em MEMORIA, e em nenhum armazenamento
    // do navegador (pedido 339(a), refeito em 08/10/2026).
    await page.fill('#iaChave', CHAVE);
    await page.check('#iaLigado');
    await page.click('#iaSalvar');
    await page.waitForSelector('#iaRemover:not([disabled])', { timeout: 8000 });
    let g = await lerGavetas(page);
    igual(g.memoria.chave, CHAVE, 'a chave devia estar na memoria do modulo');
    verdade(!g.discoCru.includes('sk-ant'), 'a chave NUNCA pode estar no localStorage');
    verdade(!g.abaCru.includes('sk-ant'), 'a chave NUNCA pode estar no sessionStorage');
    verdade(!(await page.content()).includes(CHAVE), 'a chave inteira nao pode aparecer na pagina depois de salva');
    await capturar(ctx, ctx.nomeCaptura('claude-configurada'));

    // Testar a chave: a rota responde «ok» e a tela diz que funciona.
    fila.push({ texto: 'ok' });
    const bom = await testarChave(page, { timeout: 10000 });
    verdade(bom.ok, `a chave devia funcionar contra a rota: ${bom.texto}`);
    igual(subiram.at(-1).cabecalhos['x-api-key'], CHAVE, 'a chave devia ter ido no cabecalho para a Anthropic');
    // E a chave recusada: o recado vem do erro da API, e nao e verde.
    fila.push({ status: 401, tipo: 'authentication_error', mensagem: 'invalid x-api-key' });
    const mau = await testarChave(page, { timeout: 10000 });
    verdade(!mau.ok, 'chave recusada pela API nao pode aparecer como funcionando');

    // Remover: tira das duas gavetas, desliga, e o proprio botao se desabilita.
    await page.click('#iaRemover');
    await page.waitForSelector('#iaRemover[disabled]', { timeout: 8000 });
    g = await lerGavetas(page);
    verdade(!g.memoria.chave, 'depois de Remover a chave nao pode sobrar na memoria');
    igual(!!g.disco.ligado, false, 'Remover desliga o interruptor');

    // ---------------------------------------------- a tela de Query, ligada
    await definirIA(page, { chave: CHAVE, ligado: true });
    await abrirQuery(page);
    await page.click('#btIA');
    await page.waitForSelector('#iaPainel .ia-rec', { timeout: 10000 });
    const receitas = await page.$$eval('.ia-rec', bs => bs.map(b => b.dataset.r));
    verdade(receitas.length >= 3, `o painel devia oferecer as receitas; vi ${receitas.join(',')}`);

    // Cada botao de receita troca o painel: a de explicar nao tem editor.
    await escolherReceita(page, 'explicar');
    verdade(!(await page.$('#iaExecutar')), 'a receita «explicar» nao tem editor nem Executar');
    await escolherReceita(page, 'sql');
    verdade(!!(await page.$('#iaExecutar')), 'a receita «sql» tem editor e Executar');
    await definirDb(page, db);

    // Ver o que vai subir: o corpo e os cabecalhos, com a chave MASCARADA.
    await page.fill('#iaPergunta', 'os clientes cadastrados');
    const antes = subiram.length;
    const envio = await verEnvio(page);
    verdade(!envio.cabecalhos.includes(CHAVE), 'o painel «o que vai subir» nao pode mostrar a chave inteira');
    contem(envio.cabecalhos, CHAVE.slice(-4), 'o painel devia mostrar os quatro ultimos da chave');
    contem(envio.corpo, 'os clientes cadastrados', 'o corpo mostrado devia trazer a pergunta');
    igual(subiram.length, antes, 'Ver o que vai subir nao pode subir nada');

    // Perguntar NAO envia: mostra o corpo e pede aprovacao (pedido 339(a)).
    // «Nao enviar» fecha sem subir nada.
    await page.click('#iaIr');
    await page.waitForSelector('#iaAprovar', { timeout: 15000 });
    igual(subiram.length, antes, 'Perguntar sem aprovar nao pode subir nada');
    await page.click('#iaNaoEnviar');
    await page.waitForSelector('#iaNaoEnviado', { timeout: 8000 });
    igual(subiram.length, antes, '«Nao enviar» nao pode subir nada');

    // Perguntar e APROVAR: o SQL cai no editor e NAO executa sozinho.
    fila.push({ texto: 'SELECT nome, cidade FROM clientes' });
    await page.click('#iaIr');
    await page.waitForSelector('#iaAprovar', { timeout: 15000 });
    const mostrado = await page.$$eval('#iaEnvio pre.dado', p => p[1].textContent);
    await page.click('#iaAprovar');
    await page.waitForFunction(() => document.querySelector('#iaTokens')?.querySelector('b'), undefined, { timeout: 15000 });
    igual(await page.inputValue('#iaSql'), 'SELECT nome, cidade FROM clientes', 'o SQL devia cair no editor');
    verdade(!(await page.$('#iaResultado table')) && !(await page.$('#iaGradeSql')), 'nada executa antes do clique em Executar');
    const doCorpo = JSON.parse(subiram.at(-1).corpo);
    contem(JSON.stringify(doCorpo), 'os clientes cadastrados', 'a pergunta devia ter subido no corpo');
    igual(JSON.stringify(doCorpo), JSON.stringify(JSON.parse(mostrado)),
      'o que subiu tem de ser EXATAMENTE o corpo que o painel mostrou para aprovar');

    // Executar: o motor de verdade responde com as linhas do cenario.
    await page.click('#iaExecutar');
    await page.waitForSelector('#iaGradeSql .phx-grid tbody tr', { timeout: 10000 });
    const linhas = await page.$$eval('#iaGradeSql .phx-grid tbody tr:not(.phx-grupo)', t => t.length);
    igual(linhas, 2, 'as linhas do cenario');
    // O dado vem como esta no banco: «Blumenau» nao vira «BLUMENAU».
    const grade = await page.$eval('#iaGradeSql', el => el.innerText);
    contem(grade, 'Blumenau', 'o dado devia aparecer como esta gravado');
    await capturar(ctx, ctx.nomeCaptura('claude-sql-executado'));

    // ---------------------------------------------------------- modelagem
    const modelar = async (...nomes) => {
      fila.push({ texto: plano(...nomes) });
      await abrirQuery(page);
      await abrirPainelIA(page);
      await escolherReceita(page, 'modelar');
      await definirDb(page, db);
      await page.fill('#iaPergunta', 'modele ' + nomes.join(' e '));
      await page.click('#iaIr');
      await page.click('#iaAprovar', { timeout: 15000 });
      await page.waitForSelector('#iaSaida .ia-item', { timeout: 15000 });
      await page.click('#iaCriar');
      await page.waitForSelector('#iaNascido #iaDesfazer', { timeout: 15000 });
    };
    const tabelas = async () => (await api(page, 'tabelas', { database: db })).tabelas.map(t => t.toLowerCase());

    // Criar: o servidor passa a ter a tabela. O dicionario e o ER saem DAQUI.
    const t1 = 'ia1' + sufixo;
    await modelar(t1);
    verdade((await tabelas()).includes(t1), 'a tabela do plano devia existir no servidor depois de Criar');
    await capturar(ctx, ctx.nomeCaptura('claude-criado'));

    await page.click('#iaVerDic');
    await page.waitForSelector('#btSysTabs', { timeout: 10000 });      // a tela do dicionario de dados

    const t2 = 'ia2' + sufixo;
    await modelar(t2);
    await page.click('#iaVerEr');
    await page.waitForSelector('#btErNova', { timeout: 10000 });       // o diagrama em tela cheia

    // Desfazer: a tabela que ESTA rodada criou sai do servidor; as outras ficam.
    const t3 = 'ia3' + sufixo;
    await modelar(t3);
    await page.click('#iaDesfazer');
    await page.waitForSelector('#iaDesfeito .aviso', { timeout: 10000 });
    const depois = await tabelas();
    verdade(!depois.includes(t3), 'o Desfazer devia ter removido a tabela da rodada');
    verdade(depois.includes(t1) && depois.includes(t2), 'o Desfazer nao pode tocar nas tabelas de outras rodadas');

    // A chave foi so para a Anthropic: o phxsqld nunca a viu.
    verdade(aoPhxsql.length > 0 && aoPhxsql.every(c => !c.includes(CHAVE)),
      'a chave NUNCA pode estar num pedido ao phxsqld');
    verdade(subiram.length >= 5, `esperava ao menos 5 chamadas a «Anthropic», vi ${subiram.length}`);
  },
};
