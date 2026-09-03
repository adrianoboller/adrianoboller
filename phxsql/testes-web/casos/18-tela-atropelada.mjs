/* A TELA QUE CHEGA DEPOIS NAO PODE ESCREVER POR CIMA DA QUE CHEGOU AGORA.
 *
 * Este e o caso que faltava a uma guarda que existia ha meses. O comentario do
 * `abrirAdmin` dizia, com honestidade rara, que ela NAO tinha prova real: a
 * sonda escrita na epoca passava com a guarda e passava com o defeito
 * reposto, e teste que passa por engano e pior que teste que falta.
 *
 * A SP000056 reproduziu a corrida, e ao reproduzi-la achou o furo: o contador
 * era privado do `abrirAdmin`, entao a guarda defendia `abrirAdmin` de
 * `abrirAdmin` e de mais ninguem. Toda tela que pinta por `folha()` -- a
 * telemetria, o backup, o profiler, umas cinquenta -- passava por fora dela, e
 * o `abrirAdmin` pendente escrevia o Painel por cima. **A guarda existia; o
 * alcance dela e que era o defeito.**
 *
 * O ESTRAGO, medido: `.tlm` nascia no `#painel` e sumia 37 a 104 ms depois,
 * substituida pelos `.kpis` do Painel, com o `#titulo` ainda dizendo
 * «Telemetria» -- porque o titulo o `abrirAdmin` escreve ANTES do `await` e o
 * corpo DEPOIS. Titulo de uma tela e corpo da outra: a tela mentindo sobre si
 * mesma, que e a mesma familia do «Blumenau» virando «BLUMENAU».
 *
 * POR QUE ESTE CASO REPRODUZ E A SONDA DE ANTES NAO. Ele nao torce por
 * timing: SEGURA a resposta da op `painel` no fio ate a segunda tela estar
 * pintada, e so entao solta. Com isso a corrida deixa de ser sorteio e vira
 * ordem fixa -- que e o unico jeito de um caso de bateria provar uma corrida
 * sem virar ele proprio um intermitente. Com o defeito reposto (o
 * `tomarPainel()` do `folha` comentado) ele reprova nos dois temas.
 *
 * Ele nao e um caso «da telemetria»: a telemetria e so a vitima mais barata
 * de montar. O que se prova e a regra do painel. */
import { entrar, verdade, igual } from '../apoio.mjs';

export const caso = {
  nome: 'tela-atropelada',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    // O FIO SEGURO. A op `painel` e a que o `abrirAdmin("painel")` espera; ela
    // fica presa aqui ate a gente mandar soltar. Nao ha `sleep` nenhum no caso
    // -- o que decide a ordem e a soltura, e nao o relogio.
    let soltar;
    const preso = new Promise(r => { soltar = r; });
    let segurou = false;
    await page.route('**/api', async rota => {
      let corpo = {};
      try { corpo = JSON.parse(rota.request().postData() || '{}'); } catch { /* nao e JSON */ }
      if (!segurou && corpo.op === 'painel') {
        segurou = true;
        await preso;
      }
      await rota.continue();
    });

    // 1. Pede o Painel e NAO espera: e o que o clique na arvore faz.
    await page.evaluate(() => { window.__painel = abrirAdmin('painel'); });
    await page.waitForFunction(() => !!document.querySelector('#painel .centro'), { timeout: 15000 });

    // 2. Com o Painel preso no `await`, a pessoa pede outra tela.
    await page.evaluate(() => telaTelemetria());
    await page.waitForSelector('#tlmThreads', { state: 'attached', timeout: 15000 });
    igual(await page.textContent('#titulo'), 'Telemetria',
      'a segunda tela nem chegou a escrever o proprio titulo');

    // 3. Solta o Painel: ele volta do `await` com o corpo pronto e um painel
    //    que nao e mais dele.
    soltar();
    await page.evaluate(() => window.__painel);
    // A prova precisa medir DEPOIS de o atropelo ter tido chance de acontecer.
    // Sem esta espera ela conferiria o painel antes do dano -- que e o defeito
    // que ja passou por prova nesta casa: a conferencia acontecia antes.
    await page.waitForTimeout(500);

    // O VEREDITO, e ele e sobre o par: titulo e corpo tem de ser da MESMA tela.
    const d = await page.evaluate(() => ({
      titulo: (document.querySelector('#titulo') || {}).textContent,
      temTelemetria: !!document.querySelector('#tlmThreads'),
      temKpisDoPainel: !!document.querySelector('#painel .kpis'),
    }));
    igual(d.titulo, 'Telemetria', 'o titulo deixou de ser o da tela pedida');
    verdade(d.temTelemetria,
      'o Painel atrasado escreveu por cima da tela que a pessoa pediu depois dele'
      + ` (titulo=${JSON.stringify(d.titulo)}, kpis do Painel no corpo=${d.temKpisDoPainel})`);
    verdade(!d.temKpisDoPainel,
      'o corpo do Painel aparece sob o titulo de outra tela -- a tela esta mentindo sobre si mesma');

    // ---- E AGORA A VITIMA QUE O DOCUMENTO NOMEIA -----------------------
    //
    // O `TESTES.md` §9.8 registrou este defeito com outra vitima -- «titulo de
    // Configuracoes, corpo do Painel» -- e ficou em «anotado, e nao
    // consertado» (§5.6). O contador que entrou depois NAO o fechou: as
    // Configuracoes tambem pintam por `folha()`, e era justamente `folha` que
    // ficava de fora da catraca. Repetir a corrida com ela e o que transforma
    // «deve estar resolvido junto» em «esta medido».
    // Solta o fio anterior antes de pendurar o proximo: dois tratadores no
    // mesmo padrao empilham, e o dia em que um deles ganhar por outro motivo a
    // falha sai apontando para o lugar errado.
    await page.unroute('**/api');
    let soltar2;
    const preso2 = new Promise(r => { soltar2 = r; });
    let segurou2 = false;
    await page.route('**/api', async rota => {
      let corpo = {};
      try { corpo = JSON.parse(rota.request().postData() || '{}'); } catch { /* nao e JSON */ }
      if (!segurou2 && corpo.op === 'painel') { segurou2 = true; await preso2; }
      await rota.continue();
    });
    await page.evaluate(() => { window.__painel2 = abrirAdmin('painel'); });
    await page.waitForFunction(() => !!document.querySelector('#painel .centro'), { timeout: 15000 });
    await page.evaluate(() => verConfigServidor());
    await page.waitForSelector('#painel .idi-escolha', { timeout: 15000 });
    soltar2();
    await page.evaluate(() => window.__painel2);
    await page.waitForTimeout(500);
    const c = await page.evaluate(() => ({
      titulo: (document.querySelector('#titulo') || {}).textContent,
      temConfig: !!document.querySelector('#painel .idi-escolha'),
      temKpisDoPainel: !!document.querySelector('#painel .kpis'),
    }));
    verdade(c.temConfig && !c.temKpisDoPainel,
      'o defeito da §9.8 do TESTES.md continua: titulo de Configuracoes e corpo do Painel'
      + ` (titulo=${JSON.stringify(c.titulo)}, kpis do Painel no corpo=${c.temKpisDoPainel})`);

    // E O LADO CONTRARIO, que e o que impede a guarda de virar «nunca pinta
    // nada»: pedido DEPOIS, o Painel tem de pintar normalmente. Guarda que
    // recusa tudo passa neste caso pela metade de cima e nao serve.
    await page.unroute('**/api');
    await page.evaluate(() => abrirAdmin('painel'));
    await page.waitForSelector('#painel .kpis', { timeout: 15000 });
    igual(await page.textContent('#titulo'), 'Painel',
      'o Painel pedido por ultimo nao assumiu a tela');
  },
};
