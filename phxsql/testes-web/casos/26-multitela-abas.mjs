/* Regioes com aba -- pedido 139, a parte que faltava.
 *
 * O multitela ja tinha abas, regioes lado a lado e janelas soltas (commit
 * 3ac9dae, `docs/MULTITELA.md`). O que faltava, no molde do WINDEV(R) que a
 * foto do dono mostrou, era a peca menor: uma REGIAO recebendo MAIS de uma
 * tela e mostrando a propria barra de abas para trocar entre elas -- que ja
 * existia no mecanismo (`r.abas`, `pintarTira`), mas nunca tinha sido
 * exercitada pelo caminho REAL de abrir uma ferramenta pela barra.
 *
 * E foi exercitando por esse caminho -- Ctrl-clicar "Telemetria" na barra de
 * ferramentas, o jeito que uma pessoa abre uma segunda tela na mesma regiao
 * -- que apareceu um defeito de verdade, silencioso porque so o ENDERECO
 * errava e o ROTULO da aba saia certo:
 *
 *   `disparar()` (index.html) chamava `PhxTelas.novaAba()` (que pinta
 *   "painel" na aba nova, de forma assincrona) e, encadeado, `f.faz()`
 *   (`telaTelemetria`). Os dois disputam o mesmo `Promise.resolve().then()`.
 *   Quando o `folha()` de dentro de `telaTelemetria` tentava `marcar(null)`
 *   para desenderecar a aba (o normal de uma "folha avulsa"), a guarda
 *   `W.abrindo>0` -- pensada so para o `PhxTelas.abrir()` nao desfazer o
 *   PROPRIO `chave` que acabou de por -- ainda estava de pe por causa do
 *   `abrirAdmin("painel")` pendente de `novaAba()`, e suprimia o `marcar(null)`
 *   alheio. A aba ficava rotulada "Telemetria" com `chave:"painel"` por
 *   dentro. Pinar essa aba pinava "Painel"; medido com uma sonda antes do
 *   conserto: `{chave:'painel', abas:2, rotAbas:['Telemetria','Telemetria']}`.
 *
 * O conserto (index.html): as QUATRO ferramentas com par no catalogo do
 * multitela (`fer_query`, `fer_telemetria`, `fer_profiler`, `fer_diagrama`)
 * ganharam um campo `tela:` e passaram a abrir por `PhxTelas.abrir(...)` --
 * o MESMO caminho que a URL, a restauracao do pino e a arvore ja usavam --,
 * em vez de `novaAba()+f.faz()`. Zero campos de formulario nessas quatro, e
 * por isso elas podem ser enderecaveis sem mentir sobre trabalho perdido.
 *
 * O que ESTE caso prova, na ordem do pedido:
 *
 *   1. tres telas na MESMA regiao, abertas pelo caminho real (Ctrl+clique na
 *      barra), cada uma com o CHAVE certo -- a prova de que o defeito acima
 *      nao volta;
 *   2. trocar por CLIQUE na aba;
 *   3. trocar por TECLADO -- e a prova de que NAO e Ctrl+Tab, e por que:
 *      Ctrl+Tab e Ctrl+Shift+Tab trocam a aba do PROPRIO NAVEGADOR em todo
 *      navegador de mesa (Chrome, Firefox, Safari), e a pagina nunca ve o
 *      evento -- e reservado no mesmo grupo de Ctrl+W e Ctrl+N. O atalho que
 *      sobra e Alt+seta, testado nos dois sentidos e na volta (wrap-around);
 *   4. FECHAR uma aba, pinar o resto, RECARREGAR a pagina e ver o arranjo
 *      voltar -- so o que foi pinado;
 *   5. as capturas nas QUATRO larguras do pedido (celular/tablet/desktop/
 *      ultrawide), claro e escuro, sem rolagem lateral em nenhuma.
 *
 * O que este caso NAO reprova (porque ja esta provado em `12-multitela.mjs`):
 * arrastar uma aba de uma regiao para outra, e a calha entre duas regioes.
 * Pedido 139 pergunta "arrastar entre regioes se ja houver arrastar" -- ja
 * havia, e ja tinha prova real; duplicar aqui so envelheceria em dobro. */
import { entrar, capturar, verdade, igual, assentar } from '../apoio.mjs';

const LARGURAS = [
  ['celular', 390, 844],
  ['tablet', 820, 1180],
  ['desktop', 1440, 900],
  ['ultrawide', 3440, 1440],
];

/** Ctrl+clica uma ferramenta da barra pelo `txt` dela -- o mesmo caminho que
 *  uma pessoa usa para abrir uma segunda tela na mesma regiao. */
async function ctrlClicarFerramenta(page, txtChave) {
  const i = await page.evaluate(
    c => FERRAMENTAS.findIndex(f => f && f.txt === c), txtChave);
  verdade(i >= 0, `ferramenta ${txtChave} nao encontrada na barra`);
  await page.click(`.fer[data-f="${i}"]`, { modifiers: ['Control'] });
}

const focoChave = page => page.evaluate(() => PhxTelas._W.foco && PhxTelas._W.foco.chave);
const jsonIgual = (achado, esperado, oQue) =>
  igual(JSON.stringify(achado), JSON.stringify(esperado), oQue);

export const caso = {
  nome: 'multitela-abas',

  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    // -------------------------------------------- 1. tres abas, uma regiao
    igual(await page.evaluate(() => document.querySelectorAll('#regioes .regiao').length),
      1, 'comeca com uma regiao');

    await ctrlClicarFerramenta(page, 'tela.fer_telemetria');
    await assentar(page, 600);
    await ctrlClicarFerramenta(page, 'tela.fer_profiler');
    await assentar(page, 600);

    const tres = await page.evaluate(() => ({
      regioes: document.querySelectorAll('#regioes .regiao').length,
      abas: document.querySelectorAll('.tira .tira-aba').length,
      chaves: PhxTelas._todas().map(t => t.chave),
    }));
    igual(tres.regioes, 1, 'as tres telas ficam na MESMA regiao -- e o pedido 139');
    igual(tres.abas, 3, 'tres abas na tira');
    jsonIgual(tres.chaves, ['painel', 'telemetria', 'profiler'],
      `endereco errado -- o defeito do Ctrl+clique voltou: ${JSON.stringify(tres.chaves)}`);

    await capturar(ctx, ctx.nomeCaptura('tres-abas'));

    // -------------------------------------------------- 2. trocar por CLIQUE
    await page.click('.tira .tira-aba[data-i="0"] .rot');   // no ROTULO -- e onde uma pessoa mira, nao no centro geometrico do botao (que pode cair no pino)
    await assentar(page, 250);
    igual(await focoChave(page), 'painel', 'clique na 1a aba focou o Painel');

    await page.click('.tira .tira-aba[data-i="1"] .rot');
    await assentar(page, 350);
    igual(await focoChave(page), 'telemetria', 'clique na 2a aba focou a Telemetria');

    // ------------------------------------------------- 3. trocar por TECLADO
    //
    // Ctrl+Tab foi o pedido, e e exatamente o que nao chega na pagina -- todo
    // navegador de mesa reserva Ctrl+Tab/Ctrl+Shift+Tab para a PROPRIA aba do
    // navegador. O atalho que sobra e Alt+seta (docs/MULTITELA.md).
    await page.click('.tira .tira-aba.sel .rot');
    await page.keyboard.press('Alt+ArrowRight');
    await assentar(page, 250);
    igual(await focoChave(page), 'profiler', 'Alt+-> foi para a proxima aba');

    await page.keyboard.press('Alt+ArrowRight');
    await assentar(page, 250);
    igual(await focoChave(page), 'painel', 'Alt+-> deu a volta (wrap-around) para a primeira');

    await page.keyboard.press('Alt+ArrowLeft');
    await assentar(page, 250);
    igual(await focoChave(page), 'profiler', 'Alt+<- anda no sentido contrario');

    await page.keyboard.press('Alt+ArrowLeft');
    await assentar(page, 250);
    igual(await focoChave(page), 'telemetria', 'Alt+<- continua andando');

    // --------------------------------- 4. pinar, FECHAR uma, RECARREGAR
    //
    // TRES cliques SEPARADOS, e nao um `forEach` batendo nos tres de uma vez:
    // `alternarPino` chama `desenhar()`, que reescreve o `innerHTML` da tira
    // inteira -- os botoes de pino 2 e 3 capturados ANTES do primeiro clique
    // ficam orfaos do documento assim que o primeiro pino redesenha, e um
    // `.click()` sintetico num no orfao nao borbulha para o ouvinte da tira.
    // Uma pessoa clicando um de cada vez nunca pega esse instante -- e por
    // isso o teste tem de imitar isso, e nao o `forEach` de uma tacada.
    for (let i = 0; i < 3; i++) {
      await page.click(`.tira .tira-aba[data-i="${i}"] .tira-pino`);
      await assentar(page, 200);
    }
    await assentar(page, 300);
    const guardadoAntes = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('phxsql-multitela') || '{}'));
    const pinadasAntes = (guardadoAntes.regioes || [])
      .flatMap(r => r.abas || []).map(a => a.chave).sort();
    jsonIgual(pinadasAntes, ['painel', 'profiler', 'telemetria'],
      `o pino nao guardou as tres: ${JSON.stringify(guardadoAntes)}`);

    await page.click('.tira .tira-aba[data-i="1"] .tira-x');   // fecha a Telemetria
    await assentar(page, 300);
    igual(await page.evaluate(() => document.querySelectorAll('.tira .tira-aba').length),
      2, 'fechar tirou uma aba da tira');

    await page.reload({ waitUntil: 'domcontentloaded' });
    await entrar(page, ctx.url);
    await assentar(page, 1200);
    const depoisDoReload = await page.evaluate(() => ({
      regioes: document.querySelectorAll('#regioes .regiao').length,
      abas: [...document.querySelectorAll('.tira .tira-aba .rot')].map(e => e.textContent.trim()),
    }));
    igual(depoisDoReload.regioes, 1, 'continua uma regiao so depois de recarregar');
    igual(depoisDoReload.abas.length, 2,
      `voltaram abas que ninguem pinou, ou sumiu uma pinada: ${JSON.stringify(depoisDoReload.abas)}`);

    ctx.notas.push('3 telas de catalogo (painel/telemetria/profiler) na mesma regiao, '
      + 'abertas pela barra; clique e Alt+seta trocando; fechar; '
      + `${depoisDoReload.abas.length} pinadas voltaram do recarregar`);

    await capturar(ctx, ctx.nomeCaptura('depois-do-recarregar'));

    // ------------------------------------ 5. as capturas, nas 4 larguras do pedido
    //
    // Reabre a Telemetria para a foto mostrar tres abas de novo -- a captura
    // que prova "abas dentro de uma regiao" precisa das tres, nao das duas
    // que sobraram da prova de fechar.
    await page.evaluate(() => PhxTelas.abrir('telemetria', {}, { nova: true }));
    await assentar(page, 600);

    const problemas = [];
    for (const [apelido, w, h] of LARGURAS) {
      await page.setViewportSize({ width: w, height: h });
      await assentar(page, 350);
      if (w < 640) {
        // Abaixo de 640px a lateral e GAVETA, por cima do conteudo -- e sem
        // fechar ela a foto mostraria a arvore, nao a tira de abas que este
        // caso existe para provar. Uma pessoa de verdade fecharia a gaveta
        // do mesmo jeito para ver o trabalho (`docs/MULTITELA.md`, a mesma
        // faixa de `#app[data-destacada]`/`07-responsivo.mjs`).
        await page.evaluate(() => { if (typeof alternarLateral === 'function') alternarLateral(false); });
        await assentar(page, 200);
      }
      const medida = await page.evaluate(() => ({
        rola: document.documentElement.scrollWidth,
        cabe: document.documentElement.clientWidth,
        abas: document.querySelectorAll('.tira .tira-aba').length,
      }));
      // 1px de folga: arredondamento de layout nao e rolagem lateral de verdade.
      if (medida.rola > medida.cabe + 1) {
        problemas.push(`${apelido}: a pagina rola de lado `
          + `(${medida.rola}px num visor de ${medida.cabe}px)`);
      }
      if (medida.abas !== 3) {
        problemas.push(`${apelido}: esperava 3 abas na tira, achei ${medida.abas}`);
      }
      await capturar(ctx, ctx.nomeCaptura(apelido));
    }
    await page.setViewportSize({ width: 1600, height: 950 });
    verdade(problemas.length === 0, `largura:\n      ${problemas.join('\n      ')}`);
  },
};
