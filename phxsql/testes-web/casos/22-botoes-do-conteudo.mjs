/* Os botoes do CONTEUDO: paginar, marcar, excluir, restaurar, esvaziar.
 *
 * E o caminho pelo qual a pessoa mexe no DADO, e por isso vale mais que os
 * outros: um botao errado aqui nao mostra a tela torta, apaga linha. E foi
 * exatamente aqui que o `rownum` quebrou «todo salvar e todo incluir pela
 * tela» -- coluna de sistema nova quebra quem filtra pela primeira.
 *
 * A REGRA: confere o EFEITO no dado, nunca o estado da tela.
 *
 * «Marcar como excluido» nao se prova vendo o dialogo fechar: prova-se
 * perguntando ao servidor se a linha saiu das ativas E entrou nas excluidas.
 * Um dialogo que fecha sem gravar passaria na primeira prova e reprovaria na
 * segunda -- e a segunda e a que a pessoa vive.
 *
 * O botao se identifica pela CHAVE (`id` ou `data-*`), nunca pela frase: o
 * texto passa pelos seis idiomas da fabrica. */
import {
  entrar, capturar, cenario, api, verdade, bancoDoCaso, abrirPelaArvore,
  clicarOuExplicar, assentar,
} from '../apoio.mjs';

/** Quantas linhas o servidor ve como ativas -- a fonte da verdade, e nao a tela. */
async function ativas(page, db, tab) {
  const r = await api(page, 'varrer', { database: db, tabela: tab, limite: 500 });
  return (r.linhas || []).length;
}

export const caso = {
  nome: 'botoes-do-conteudo',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'BtConteudo');
    const { tab } = await cenario(page, db);

    // Linhas o bastante para a paginacao por cursor existir de verdade: o
    // `est.teto` nasce em 200, entao com oitenta linhas os quatro botoes de
    // pagina nascem CINZAS e o caso passaria sem clicar em nenhum. Vao num
    // lote so -- duzentas e cinquenta viagens de protocolo mediriam a
    // maquina, e nao a tela.
    const lote = [];
    for (let i = 4; i <= 260; i++) {
      lote.push({
        id: i, nome: `Pessoa ${String(i).padStart(3, '0')}`,
        cidade: 'Blumenau', uf: 'SC', limite: '10.00',
        cadastro: '2025-02-01', ficha: '',
      });
    }
    await api(page, 'inserir_lote', { database: db, tabela: tab, linhas: lote });

    const falhas = [];
    const provado = [];
    const botao = async (chave, oQue, fn) => {
      try {
        await fn();
        provado.push(chave);
      } catch (e) {
        falhas.push(`${chave} (${oQue}): ${e.message}`);
      }
    };
    const abrirEditavel = async () => {
      await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
      await page.waitForSelector('#gradeEdit .phx-tabela tbody tr', { timeout: 15000 });
    };
    /** O rowid da primeira linha visivel na grade editavel -- pela COLUNA
     *  `rowid`, achada no cabecalho, e nao pela posicao chutada. */
    const primeiroRowid = () => page.evaluate(() => {
      const g = document.querySelector('#gradeEdit');
      const cab = [...g.querySelectorAll('thead tr:not(.phx-frow)')].pop();
      const ix = [...cab.querySelectorAll('th')]
        .findIndex(t => t.getAttribute('data-campo') === 'rowid');
      const tr = g.querySelector('tbody tr:not(.phx-grupo):not(.phx-grodape):not(.phx-total-geral)');
      return tr ? tr.children[ix].textContent.trim() : null;
    });

    // ------------------------------------------------ 1. paginar por cursor
    await abrirEditavel();

    await botao('#pgDepois', 'ir para a proxima pagina', async () => {
      const p1 = await primeiroRowid();
      await clicarOuExplicar(page, '#pgDepois');
      await page.waitForSelector('#gradeEdit .phx-tabela tbody tr', { timeout: 15000 });
      await assentar(page, 400);
      const p2 = await primeiroRowid();
      verdade(p1 !== p2, `a pagina nao virou: a primeira linha continua rowid ${p1}`);
    });

    await botao('#pgAntes', 'voltar uma pagina', async () => {
      const p2 = await primeiroRowid();
      await clicarOuExplicar(page, '#pgAntes');
      await assentar(page, 600);
      const p1 = await primeiroRowid();
      verdade(p1 !== p2, `voltar nao mudou a pagina: continua rowid ${p2}`);
    });

    await botao('#pgFim', 'ir para a ultima pagina', async () => {
      const antes = await primeiroRowid();
      await clicarOuExplicar(page, '#pgFim');
      await assentar(page, 700);
      const depois = await primeiroRowid();
      verdade(antes !== depois, `«fim» ficou na mesma pagina (rowid ${antes})`);
      // A ULTIMA pagina nao tem proxima: o botao tem de estar cinza. Botao
      // que continua clicavel no fim da lista e o que faz a pessoa achar que
      // ha mais dado.
      verdade(await page.locator('#pgDepois[disabled]').count() === 1,
        'na ultima pagina o «proxima» continuou habilitado');
    });

    await botao('#pgInicio', 'voltar ao comeco', async () => {
      await clicarOuExplicar(page, '#pgInicio');
      await assentar(page, 700);
      verdade(await page.locator('#pgAntes[disabled]').count() === 1,
        'no comeco o «anterior» continuou habilitado');
    });

    // --------------------------------- 2. excluir suave, ver, restaurar
    let alvo = null;
    await botao('[data-rowid] (editar)', 'abrir a ficha pela linha', async () => {
      alvo = await primeiroRowid();
      await clicarOuExplicar(page, `#gradeEdit .bt-editar[data-rowid="${alvo}"]`);
      await page.waitForSelector('#btVoltar', { timeout: 15000 });
      // O EFEITO: a ficha abriu NA LINHA pedida. Conferir so «abriu a ficha»
      // deixaria passar a ficha da linha errada, que e pior que ficha nenhuma.
      const texto = await page.textContent('#painel');
      verdade(texto.includes(alvo), `a ficha abriu sem o rowid ${alvo} em lugar nenhum`);
    });

    await botao('#btVoltar', 'voltar da ficha para o conteudo', async () => {
      await clicarOuExplicar(page, '#btVoltar');
      await page.waitForSelector('#gradeEdit', { timeout: 15000 });
    });

    await botao('#btExcNao', 'cancelar a exclusao sem apagar nada', async () => {
      const antes = await ativas(page, db, tab);
      await clicarOuExplicar(page, `#gradeEdit .bt-editar[data-rowid="${alvo}"]`);
      await page.waitForSelector('#btExcluir', { timeout: 15000 });
      await clicarOuExplicar(page, '#btExcluir');
      await page.waitForSelector('#btExcNao', { timeout: 10000 });
      await clicarOuExplicar(page, '#btExcNao');
      await assentar(page, 300);
      verdade(await page.locator('#btExcNao').count() === 0, 'o dialogo nao fechou');
      // O EFEITO que importa: cancelar nao pode ter apagado nada. A prova e
      // no SERVIDOR -- a tela ainda mostraria a linha mesmo se ela tivesse
      // ido embora, porque a grade nao recarregou.
      const depois = await ativas(page, db, tab);
      verdade(antes === depois,
        `«Cancelar» apagou: ${antes} linhas ativas antes, ${depois} depois`);
    });

    await botao('[data-modo="fisico"]', 'escolher o modo «de vez»', async () => {
      await clicarOuExplicar(page, '#btExcluir');
      await page.waitForSelector('[data-modo="fisico"]', { timeout: 10000 });
      const antes = await page.getAttribute('#btExcSim', 'class');
      await clicarOuExplicar(page, '[data-modo="fisico"]');
      await assentar(page, 200);
      const depois = await page.getAttribute('#btExcSim', 'class');
      // O EFEITO: o botao de confirmar TROCA DE COR -- vermelho e «de vez»,
      // rosa e «marcar». A cor e a convencao da casa, e aqui ela e o unico
      // aviso de que o proximo clique nao volta.
      verdade(antes !== depois,
        `escolher «de vez» nao mudou o botao de confirmar (classe «${antes}»)`);
      verdade(depois.includes('excluir'),
        `o confirmar de «de vez» ficou com a classe «${depois}», e nao a de excluir`);
    });

    await botao('[data-modo="suave"]', 'voltar para o modo «marcar»', async () => {
      await clicarOuExplicar(page, '[data-modo="suave"]');
      await assentar(page, 200);
      const cls = await page.getAttribute('#btExcSim', 'class');
      verdade(cls.includes('marcar'),
        `voltar para «marcar» deixou o confirmar com a classe «${cls}»`);
    });

    await botao('#btExcSim', 'confirmar a exclusao suave', async () => {
      const antes = await ativas(page, db, tab);
      await page.fill('#excMotivo', 'prova da bateria de botoes');
      await clicarOuExplicar(page, '#btExcSim');
      await assentar(page, 900);
      const depois = await ativas(page, db, tab);
      verdade(depois === antes - 1,
        `a linha nao saiu das ativas: ${antes} antes, ${depois} depois`);
    });

    await abrirEditavel();
    await botao('#vwExcl', 'ver as linhas excluidas', async () => {
      await clicarOuExplicar(page, '#vwExcl');
      await page.waitForSelector('#gradeEdit .restaurar', { timeout: 15000 });
      const texto = await page.textContent('#gradeEdit');
      verdade(texto.includes(alvo),
        `a linha ${alvo} foi marcada como excluida e nao apareceu na visao «excluidas»`);
    });

    await botao('[data-rowid] (restaurar)', 'restaurar a linha excluida', async () => {
      const antes = await ativas(page, db, tab);
      // O motivo vem por `prompt` NATIVO, e sem alguem escutando o Playwright
      // o descarta: a funcao voltaria na primeira linha e o teste passaria
      // sem restaurar nada.
      page.once('dialog', d => d.accept('prova da bateria de botoes'));
      await clicarOuExplicar(page, `#gradeEdit .restaurar[data-rowid="${alvo}"]`);
      await assentar(page, 900);
      const depois = await ativas(page, db, tab);
      verdade(depois === antes + 1,
        `restaurar nao devolveu a linha: ${antes} ativas antes, ${depois} depois`);
    });

    await botao('#vwAtivas', 'voltar para as linhas ativas', async () => {
      await abrirEditavel();
      await clicarOuExplicar(page, '#vwExcl');
      await assentar(page, 600);
      await clicarOuExplicar(page, '#vwAtivas');
      await page.waitForSelector('#gradeEdit .bt-editar', { timeout: 15000 });
      verdade(await page.locator('#gradeEdit .restaurar').count() === 0,
        'a visao «ativas» continuou mostrando botao de restaurar');
    });

    // ------------------------------------- 3. a selecao em lote da aba Conteudo
    await abrirPelaArvore(page, db, tab);
    await page.click('.aba[data-aba="conteudo"]');
    await page.waitForSelector('#grade .phx-tabela tbody tr');
    const marcarDuas = async () => {
      await page.locator('#grade tbody .phx-td-sel input').nth(0).check();
      await page.locator('#grade tbody .phx-td-sel input').nth(1).check();
      await page.waitForSelector('#acoesGrade:not([hidden])', { timeout: 5000 });
    };

    await botao('#btLimparSel', 'desmarcar tudo', async () => {
      await marcarDuas();
      await clicarOuExplicar(page, '#btLimparSel');
      await assentar(page, 400);
      // O EFEITO na TELA e no ESTADO: a barra some porque nao ha marcada.
      verdade(await page.locator('#acoesGrade[hidden]').count() === 1,
        'a barra de selecao continuou aberta depois de desmarcar');
      verdade(await page.locator('#grade tbody .phx-td-sel input:checked').count() === 0,
        'sobrou linha marcada depois do «desmarcar»');
    });

    await botao('#btCopiarSel', 'copiar os rowids marcados', async () => {
      await marcarDuas();
      await clicarOuExplicar(page, '#btCopiarSel');
      await page.waitForSelector('#aviso:not([hidden])', { timeout: 5000 });
      // Sem permissao de area de transferencia o navegador RECUSA, e a tela
      // avisa. As duas saidas sao legitimas; o que nao pode e o clique nao
      // dizer nada -- botao mudo e o que faz a pessoa clicar de novo.
      const mal = await page.locator('#aviso.mal').count();
      const recado = (await page.textContent('#aviso')).trim();
      verdade(recado.length > 0, 'o «copiar» nao disse nada');
      if (mal) ctx.notas.push(`copiar rowids recusado pelo navegador: «${recado}»`);
    });

    await botao('#btExcluirSel', 'marcar as selecionadas como excluidas', async () => {
      await marcarDuas();
      const antes = await ativas(page, db, tab);
      // O `confirm` e NATIVO: sem alguem escutando, o Playwright o descarta e
      // a funcao volta na primeira linha -- o clique passaria sem excluir e o
      // teste passaria junto.
      page.once('dialog', d => d.accept());
      await clicarOuExplicar(page, '#btExcluirSel');
      await assentar(page, 1200);
      const depois = await ativas(page, db, tab);
      verdade(depois === antes - 2,
        `as duas marcadas nao sairam: ${antes} ativas antes, ${depois} depois`);
    });

    // ------------------------------------------------------- 4. a lixeira
    // Uma linha DE VEZ, para a lixeira ter o que mostrar. Vai pela api porque
    // o que se prova aqui sao os botoes DA LIXEIRA, e nao o excluir de vez --
    // esse ja tem o dono dele no passo 2.
    const r = await api(page, 'varrer', { database: db, tabela: tab, limite: 1 });
    await api(page, 'excluir', {
      database: db, tabela: tab, rowid: +r.linhas[0].rowid,
      fisico: true, motivo: 'preparo da prova da lixeira',
    });

    await botao('#btVerMotivos', 'ver os motivos registrados', async () => {
      await page.evaluate(([d, t]) => telaLixeira(d, t), [db, tab]);
      await page.waitForSelector('#btVerMotivos', { timeout: 15000 });
      await clicarOuExplicar(page, '#btVerMotivos');
      await page.waitForSelector('#btVoltaMot', { timeout: 15000 });
      const texto = await page.textContent('#painel');
      verdade(texto.includes('preparo da prova da lixeira'),
        'a tela de motivos abriu sem o motivo que acabou de ser gravado');
    });

    await botao('#btVoltaLix', 'voltar da lixeira para gerir tabelas', async () => {
      await page.evaluate(([d, t]) => telaLixeira(d, t), [db, tab]);
      await page.waitForSelector('#btVoltaLix', { timeout: 15000 });
      await clicarOuExplicar(page, '#btVoltaLix');
      await page.waitForSelector('#btNovaTab', { timeout: 15000 });
    });

    await botao('#btEsvaziar', 'esvaziar a lixeira', async () => {
      await page.evaluate(([d, t]) => telaLixeira(d, t), [db, tab]);
      await page.waitForSelector('#btEsvaziar', { timeout: 15000 });
      const antes = await api(page, 'lixeira', { database: db, tabela: tab });
      verdade((antes.descartadas || []).length > 0, 'a lixeira estava vazia antes do teste');
      // O `prompt` do motivo e nativo: sem resposta o expurgo nao acontece.
      page.once('dialog', d => d.accept('prova da bateria de botoes'));
      await clicarOuExplicar(page, '#btEsvaziar');
      await assentar(page, 1200);
      const depois = await api(page, 'lixeira', { database: db, tabela: tab });
      verdade((depois.descartadas || []).length === 0,
        `a lixeira nao esvaziou: ${(depois.descartadas || []).length} linha(s) ficaram`);
    });

    // --------------------------------------------- 5. as voltas de navegacao
    await botao('#btVoltarDb', 'voltar do conteudo para as tabelas do banco', async () => {
      await abrirEditavel();
      await clicarOuExplicar(page, '#btVoltarDb');
      // O EFEITO: a tela do DATABASE, com a tabela listada. Conferir so que a
      // tela mudou deixaria passar «foi para a tela errada».
      await page.waitForFunction(
        d => document.querySelector('#titulo')
          && document.querySelector('#titulo').textContent.trim() === d,
        db, { timeout: 15000 });
      await page.waitForSelector(`#painel [data-tab="${tab}"]`, { timeout: 15000 });
    });

    await capturar(ctx, ctx.nomeCaptura('conteudo-botoes'));
    ctx.notas.push(`${provado.length} botoes do conteudo exercitados`);
    verdade(falhas.length === 0,
      `botoes do conteudo que reprovaram:\n      ${falhas.join('\n      ')}`);
  },
};
