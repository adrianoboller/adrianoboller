/* Os botoes da PhxGrid -- a grade que TODA tela desta casa usa.
 *
 * Por que esta e a primeira leva: a grade e o componente mais reaproveitado
 * da interface («todas as table sao phxgrid»), entao um botao quebrado aqui
 * quebra em dezenas de telas ao mesmo tempo, e nenhuma delas acusa. E porque
 * ela e o lugar onde o CSS global mais morde: o caso `acrescentar-coluna`
 * achou a caixa de marcar esticada a 834px pelo `input{width:100%}` no
 * primeiro minuto em que o cartao existiu.
 *
 * A REGRA DESTE CASO: confere o EFEITO, nunca o estado.
 *
 * Uma prova desta casa ja «passou por engano» porque conferia `estado().ordem`
 * em vez do que apareceu na tela -- o botao podia nem existir e o teste
 * passava. Aqui todo passo le a TABELA depois do clique: quais linhas,
 * quantas, em que ordem, quais colunas. Botao que abre painel confere o que o
 * painel FEZ, e nao que ele abriu.
 *
 * E o botao se acha por CHAVE (`id`, `data-*` ou a classe que o proprio
 * codigo usa como gancho), nunca pela frase: o texto passa pelos seis idiomas
 * da fabrica, e quem casa por frase quebra calado quando alguem melhora a
 * redacao -- ou quando a tela abre em alemao. */
import {
  entrar, capturar, cenario, api, verdade, bancoDoCaso, abrirPelaArvore,
  clicarOuExplicar, assentar,
} from '../apoio.mjs';

/** As celulas de uma coluna do corpo, na ordem em que a tela as mostra.
 *
 * Pela POSICAO no cabecalho, porque a celula do corpo nao carrega
 * `data-campo` -- e a mesma medida que o `abrirLinhaDaGrade` do apoio ja faz. */
async function coluna(page, em, campo) {
  return await page.evaluate(([sel, c]) => {
    const g = document.querySelector(sel);
    if (!g) return null;
    const cab = [...g.querySelectorAll('thead tr:not(.phx-frow)')].pop();
    const ths = [...cab.querySelectorAll('th')];
    const ix = ths.findIndex(t => t.getAttribute('data-campo') === c);
    if (ix < 0) return null;
    // Fora as linhas que NAO sao dado: o cabecalho de grupo, o total do
    // grupo (`phx-grodape`) e o total geral. Contar o total como linha faria
    // «recolher tudo» parecer que nao escondeu nada.
    return [...g.querySelectorAll(
      'tbody tr:not(.phx-grupo):not(.phx-grodape):not(.phx-total-geral)')]
      .map(tr => (tr.children[ix] ? tr.children[ix].textContent.trim() : ''));
  }, [em, campo]);
}

const contarLinhas = (page, em) => page.locator(
  `${em} tbody tr:not(.phx-grupo):not(.phx-grodape):not(.phx-total-geral)`).count();

export const caso = {
  nome: 'botoes-da-grade',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'BtGrade');
    const { tab } = await cenario(page, db);

    // Linhas o bastante para a paginacao existir e para «ordenar» significar
    // alguma coisa. Vao pelo lote, e nao uma a uma: sessenta viagens de
    // protocolo mediriam a maquina, e nao a tela.
    const lote = [];
    for (let i = 4; i <= 63; i++) {
      lote.push({
        id: i,
        nome: `Cliente ${String(i).padStart(3, '0')}`,
        cidade: ['Blumenau', 'Joinville', 'Curitiba', 'Itajaí'][i % 4],
        uf: ['SC', 'SC', 'PR', 'SC'][i % 4],
        limite: String((i * 137) % 9000) + '.00',
        cadastro: '2025-02-01',
        ficha: '',
      });
    }
    await api(page, 'inserir_lote', { database: db, tabela: tab, linhas: lote });

    await abrirPelaArvore(page, db, tab);
    await page.click('.aba[data-aba="conteudo"]');
    await page.waitForSelector('#grade .phx-tabela tbody tr');
    const G = '#grade';
    const falhas = [];
    const provado = [];
    /** Um passo por botao. A falha NOMEIA a chave -- e o nome dela que diz
     *  qual botao reprovou, porque o texto nao serve de identidade. */
    const botao = async (chave, oQue, fn) => {
      try {
        await fn();
        provado.push(chave);
      } catch (e) {
        falhas.push(`${chave} (${oQue}): ${e.message}`);
      }
    };

    // ------------------------------------------------- 1. a paginacao
    await page.selectOption(`${G} .phx-tam`, '50');
    await assentar(page, 300);
    await botao('[data-p]', 'ir para a pagina 2', async () => {
      const p1 = await coluna(page, G, 'id');
      verdade(p1 && p1.length > 0, 'a grade nao trouxe linha nenhuma');
      await clicarOuExplicar(page, `${G} .phx-pag [data-p="2"]`);
      await assentar(page, 400);
      const p2 = await coluna(page, G, 'id');
      verdade(p2 && p2.length > 0, 'a pagina 2 saiu vazia');
      verdade(p1[0] !== p2[0],
        `a pagina nao virou: a primeira linha continua «${p1[0]}»`);
      await clicarOuExplicar(page, `${G} .phx-pag [data-p="1"]`);
      await assentar(page, 400);
    });

    // ---------------------------------- 2. o agregador do cabecalho
    // O EFEITO e o texto do proprio botao mudando de funcao (SUM -> AVG -> …)
    // E o rodape recalculando. Conferir «o botao existe» nao provaria nada.
    await botao('.phx-th-agg', 'alternar o agregador da coluna', async () => {
      const alvo = `${G} th[data-campo="limite"] .phx-th-agg`;
      await page.waitForSelector(alvo);
      const antes = (await page.textContent(alvo)).trim();
      const totalAntes = await page.textContent(`${G} tr.phx-total-geral`).catch(() => '');
      await clicarOuExplicar(page, alvo);
      await assentar(page, 250);
      const depois = (await page.textContent(alvo)).trim();
      verdade(antes !== depois, `o agregador continuou «${antes}»`);
      const totalDepois = await page.textContent(`${G} tr.phx-total-geral`).catch(() => '');
      // O EFEITO: o total geral recalcula. Sem isto o passo provaria so que o
      // rotulo do botao trocou de letra -- rotulo sem efeito e mentira sobre
      // o dado, que e a mesma lei do «Blumenau» virando «BLUMENAU».
      verdade(totalAntes !== totalDepois,
        `o agregador foi de ${antes} para ${depois} e o total geral nao mudou `
        + `(«${totalAntes}») -- rotulo sem efeito`);
    });

    // --------------------------------- 3. o filtro de coluna e o popup
    await botao('.phx-fbtn', 'abrir o filtro da coluna', async () => {
      await clicarOuExplicar(page, `${G} th[data-campo="nome"] .phx-fbtn`);
      await page.waitForSelector(`${G} .phx-fpop:not([hidden])`, { timeout: 5000 });
    });

    await botao('[data-a="za"]', 'classificar de Z a A pelo popup', async () => {
      const antes = await coluna(page, G, 'nome');
      await clicarOuExplicar(page, `${G} .phx-fpop [data-a="za"]`);
      await assentar(page, 400);
      const depois = await coluna(page, G, 'nome');
      // O EFEITO: o dado saiu ordenado ao contrario. Ler `estado().ordem`
      // aqui e o erro que ja passou por engano nesta casa.
      verdade(antes[0] !== depois[0],
        `a grade nao mexeu: primeira linha continua «${antes[0]}»`);
      const fora = depois.findIndex((v, i) => i > 0 && depois[i - 1].localeCompare(v) < 0);
      verdade(fora < 0,
        `a coluna nao ordenou de Z a A: «${depois[fora - 1]}» veio antes de «${depois[fora]}»`);
    });

    await botao('[data-a="az"]', 'classificar de A a Z pelo popup', async () => {
      await clicarOuExplicar(page, `${G} th[data-campo="nome"] .phx-fbtn`);
      await page.waitForSelector(`${G} .phx-fpop:not([hidden])`);
      await clicarOuExplicar(page, `${G} .phx-fpop [data-a="az"]`);
      await assentar(page, 400);
      const depois = await coluna(page, G, 'nome');
      // Nao-decrescente ponto a ponto, e a mensagem NOMEIA o par que quebra.
      // Comparar so o primeiro elemento com o menor da pagina passava por
      // engano quando a pagina inteira ja comecava certa por outro motivo.
      const fora = depois.findIndex((v, i) => i > 0 && depois[i - 1].localeCompare(v) > 0);
      verdade(fora < 0,
        `a coluna nao ordenou de A a Z: «${depois[fora - 1]}» veio antes de «${depois[fora]}»`);
    });

    await botao('.phx-fpop-cancela', 'fechar o popup sem mudar nada', async () => {
      const antes = await coluna(page, G, 'nome');
      await clicarOuExplicar(page, `${G} th[data-campo="cidade"] .phx-fbtn`);
      await page.waitForSelector(`${G} .phx-fpop:not([hidden])`);
      await clicarOuExplicar(page, `${G} .phx-fpop .phx-fpop-cancela`);
      await assentar(page, 250);
      verdade(await page.locator(`${G} .phx-fpop[hidden]`).count() === 1,
        'o popup continuou aberto');
      const depois = await coluna(page, G, 'nome');
      // Cancelar que muda alguma coisa e pior que cancelar que nao fecha.
      verdade(JSON.stringify(antes) === JSON.stringify(depois),
        'o «Cancelar» mexeu na grade');
    });

    // ------------------------------------ 4. o filtro pela linha de filtro
    // O chip so nasce depois de haver filtro, entao o filtro vem primeiro --
    // pela LINHA DE FILTRO, que e o caminho da pessoa.
    await botao('.phx-fpop-ok', 'confirmar o filtro pelo popup', async () => {
      await clicarOuExplicar(page, `${G} th[data-campo="uf"] .phx-fbtn`);
      await page.waitForSelector(`${G} .phx-fpop:not([hidden])`);
      const temLista = await page.locator(`${G} .phx-fpop .phx-fpop-item input`).count();
      if (temLista > 0) {
        // Fonte local: desmarca o primeiro valor e confirma.
        await page.locator(`${G} .phx-fpop .phx-fpop-item input`).first().uncheck();
      }
      const antes = await contarLinhas(page, G);
      await clicarOuExplicar(page, `${G} .phx-fpop .phx-fpop-ok`);
      await assentar(page, 500);
      verdade(await page.locator(`${G} .phx-fpop[hidden]`).count() === 1,
        'o OK nao fechou o popup');
      if (temLista > 0) {
        const depois = await contarLinhas(page, G);
        verdade(depois < antes,
          `o OK nao filtrou nada: ${antes} linhas antes, ${depois} depois`);
      }
    });

    await botao('.phx-frow', 'filtrar pela linha de filtro (preparo do chip)', async () => {
      await page.fill(`${G} .phx-frow [data-campo="cidade"] input`, 'Blumenau');
      await page.keyboard.press('Enter');
      await assentar(page, 600);
      const cid = await coluna(page, G, 'cidade');
      verdade(cid.length > 0, 'a linha de filtro nao deixou linha nenhuma');
      const intrusas = cid.filter(c => !c.includes('Blumenau'));
      verdade(intrusas.length === 0,
        `a linha de filtro deixou passar ${intrusas.length} linha(s) que nao sao `
        + `Blumenau: ${JSON.stringify(intrusas.slice(0, 4))} (de ${cid.length})`);
    });

    await botao('.phx-chip-x', 'tirar UM filtro pelo chip', async () => {
      await page.waitForSelector(`${G} .phx-chip-x`, { timeout: 5000 });
      const chipsAntes = await page.locator(`${G} .phx-chip-x`).count();
      const antes = await contarLinhas(page, G);
      await page.locator(`${G} .phx-chip-x`).first().click();
      await assentar(page, 600);
      const chipsDepois = await page.locator(`${G} .phx-chip-x`).count();
      verdade(chipsDepois < chipsAntes,
        `o chip nao saiu: ${chipsAntes} antes, ${chipsDepois} depois`);
      const depois = await contarLinhas(page, G);
      verdade(depois >= antes, 'tirar filtro trouxe MENOS linhas');
    });

    await botao('.phx-filtros-limpar', 'limpar TODOS os filtros', async () => {
      if (await page.locator(`${G} .phx-filtros-limpar`).count() === 0) {
        // Sobrou um filtro so, e o chip unico nao traz o «limpar tudo». Poe
        // outro de volta para o botao existir.
        await page.fill(`${G} .phx-frow [data-campo="nome"] input`, 'Cliente');
        await page.keyboard.press('Enter');
        await assentar(page, 600);
      }
      await page.waitForSelector(`${G} .phx-filtros-limpar`, { timeout: 5000 });
      await clicarOuExplicar(page, `${G} .phx-filtros-limpar`);
      await assentar(page, 600);
      verdade(await page.locator(`${G} .phx-chip-x`).count() === 0,
        'sobrou chip depois do «limpar tudo»');
    });

    // ------------------------------------------- 5. a barra de agrupamento
    // O agrupamento se ARRASTA, e arrastar num navegador sem cabeca prova o
    // arrastar, nao os botoes. Entao o cenario se monta pela API da grade --
    // que e preparo, como `cenario()` monta banco pela `api()` -- e o que se
    // PROVA e o clique nos botoes que so existem depois dele.
    await page.evaluate(() => window.__phxgrade.agrupar(['uf']));
    await assentar(page, 500);

    await botao('.phx-gpill-dir', 'inverter a direcao do grupo', async () => {
      const antes = await page.$$eval(`${G} tbody tr.phx-grupo`,
        rs => rs.map(r => r.textContent.trim()));
      verdade(antes.length > 1, `agrupou em ${antes.length} grupo(s), preciso de dois`);
      await clicarOuExplicar(page, `${G} .phx-gpill-dir`);
      await assentar(page, 500);
      const depois = await page.$$eval(`${G} tbody tr.phx-grupo`,
        rs => rs.map(r => r.textContent.trim()));
      verdade(antes[0] !== depois[0],
        `a ordem dos grupos nao virou: continua «${antes[0]}»`);
    });

    await botao('[data-todos="fechar"]', 'recolher todos os grupos', async () => {
      const antes = await contarLinhas(page, G);
      await clicarOuExplicar(page, `${G} [data-todos="fechar"]`);
      await assentar(page, 400);
      const depois = await contarLinhas(page, G);
      verdade(depois < antes,
        `recolher nao escondeu linha nenhuma: ${antes} antes, ${depois} depois`);
    });

    await botao('[data-todos="abrir"]', 'expandir todos os grupos', async () => {
      const antes = await contarLinhas(page, G);
      await clicarOuExplicar(page, `${G} [data-todos="abrir"]`);
      await assentar(page, 400);
      const depois = await contarLinhas(page, G);
      verdade(depois > antes,
        `expandir nao mostrou linha nenhuma: ${antes} antes, ${depois} depois`);
    });

    await botao('[data-rodape="1"]', 'ligar o total por grupo', async () => {
      const antes = await page.locator(`${G} tbody tr.phx-grodape`).count();
      await clicarOuExplicar(page, `${G} [data-rodape="1"]`);
      await assentar(page, 400);
      const depois = await page.locator(`${G} tbody tr.phx-grodape`).count();
      verdade(depois !== antes,
        `o total por grupo nao apareceu nem sumiu: ${antes} linha(s) de rodape nos dois`);
    });

    await botao('.phx-gpill-x', 'desagrupar', async () => {
      await clicarOuExplicar(page, `${G} .phx-gpill-x`);
      await assentar(page, 500);
      verdade(await page.locator(`${G} tbody tr.phx-grupo`).count() === 0,
        'sobrou linha de grupo depois de desagrupar');
    });

    // ------------------------------------------- 6. o seletor de colunas
    await botao('.phx-colsel-btn', 'abrir o seletor de colunas', async () => {
      await clicarOuExplicar(page, `${G} .phx-colsel-btn`);
      await page.waitForSelector(`${G} .phx-colsel:not([hidden])`, { timeout: 5000 });
      // O EFEITO util: desmarcar uma coluna a tira da TABELA.
      const antes = await coluna(page, G, 'cidade');
      verdade(antes !== null, 'a coluna «cidade» nao estava na grade');
      await page.locator(`${G} .phx-colsel input[data-campo="cidade"]`).uncheck();
      await assentar(page, 400);
      verdade(await coluna(page, G, 'cidade') === null,
        'desmarcar a coluna nao a tirou da grade');
      await page.locator(`${G} .phx-colsel input[data-campo="cidade"]`).check();
      await assentar(page, 400);
    });

    await botao('[data-pino]', 'congelar a coluna a esquerda', async () => {
      const alvo = `${G} .phx-colsel [data-pino="nome"]`;
      await page.waitForSelector(alvo, { timeout: 5000 });
      await clicarOuExplicar(page, alvo);
      await assentar(page, 400);
      // O EFEITO na TELA: a coluna congelada ganha a classe da grade, e e ela
      // que o CSS usa para gruda-la. Ler o estado interno nao provaria que a
      // coluna gruda.
      const congelada = await page.locator(`${G} th[data-campo="nome"].phx-fixa-esq`).count();
      verdade(congelada === 1, 'a coluna nao ficou congelada a esquerda');
      await clicarOuExplicar(page, alvo);
      await assentar(page, 300);
    });

    // ------------------------------------------- 7. exportar e o JSON
    // O download nao se completa num contexto sem `acceptDownloads`, e nao e
    // ele que se prova aqui: prova-se que o botao MONTA a vista sem estourar.
    await botao('.phx-exp-btn', 'exportar a vista', async () => {
      page.once('download', d => d.cancel().catch(() => {}));
      await clicarOuExplicar(page, `${G} .phx-exp-btn`);
      await assentar(page, 500);
    });

    await capturar(ctx, ctx.nomeCaptura('grade-botoes'));

    ctx.notas.push(`${provado.length} botoes da grade exercitados`);
    verdade(falhas.length === 0,
      `botoes da grade que reprovaram:\n      ${falhas.join('\n      ')}`);
  },
};
