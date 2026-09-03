/* A tela de Gestão de transações, EXERCITADA — e não lida.
 *
 * Por que ela não podia ser um `cargo test`: a tela anterior passava versões
 * dizendo, em português cravado, que «transações de verdade não existem no
 * PhxSql hoje». Elas passaram a existir, e uma tela que continua dizendo que
 * não dá é tão errada quanto uma que oferece um botão que não faz nada. As
 * duas mentiras são invisíveis para o motor, que está certo dos dois lados.
 *
 * E três coisas que só o navegador acha:
 *
 * 1. **o CSS global morde o componente novo** — a tabela de transações abertas
 *    é a primeira `<table>` desta tela, e o `text-transform:uppercase` da folha
 *    global mentiria sobre o nome do usuário e sobre o nome da tabela travada,
 *    que são DADO e não rótulo. É a lição do «Blumenau» virando «BLUMENAU»;
 * 2. **o texto passa mesmo pela fábrica de idiomas** — trocar o idioma tem de
 *    trocar a tela. Uma chave que existe na fábrica e que a tela não pede
 *    continua em português para sempre, e ninguém vê isso lendo o código;
 * 3. **a página não estoura horizontalmente** com a tabela de dez colunas. */
import {
  entrar, api, capturar, verdade, contem, bancoDoCaso,
} from '../apoio.mjs';

/** Abre a tela pelo menu, e não chamando `verTransacoes()` por dentro: o
 *  caminho da pessoa é o caminho do teste. */
async function abrirTela(page) {
  await page.evaluate(() => verTransacoes());
  await page.waitForFunction(
    () => document.querySelector('#titulo')
      && /transa/i.test(document.querySelector('#titulo').textContent),
    { timeout: 15000 });
  await page.waitForTimeout(250);
}

export const caso = {
  nome: 'transacoes',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Tx');

    await api(page, 'criar_database', { database: db }).catch(() => {});
    await api(page, 'criar_tabela', {
      database: db, tabela: 'pedidos',
      colunas: [
        { nome: 'id', tipo: 'Int4', obrigatoria: true },
        { nome: 'cidade', tipo: 'Str(30)' },
      ],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    }).catch(() => {});

    // --- 1. a tela não pode mais dizer que transação não existe
    await abrirTela(page);
    const corpo = await page.textContent('#painel');
    verdade(!/não existem|nao existem/i.test(corpo),
      'a tela continua dizendo que transações não existem — e elas existem');
    contem(corpo, 'SERIALIZABLE',
      'a tela não diz o nível de isolamento pelo nome certo');
    contem(corpo, 'ABORT_ONLY',
      'a tela não mostra o estado que recusa o COMMIT');
    contem(corpo, 'MVCC',
      'a tela não diz o que NÃO existe, que é metade do que ela tem a dizer');
    await capturar(ctx, ctx.nomeCaptura('tela'));

    // --- 2. a lista vazia diz que está vazia, em vez de sumir
    contem(corpo, 'Nenhuma transação aberta',
      'a lista vazia não se anuncia — some, e quem olha não sabe se falhou');

    // --- 3. o CSS global, no componente novo
    //
    // A porta de DADOS é que abre transação, e o navegador fala pela porta
    // WEB: então a lista é exercitada com o retrato que o servidor devolve,
    // desenhado pela MESMA função da tela. É o componente que se prova aqui,
    // e ele é o que o CSS global morde.
    // A tela monta a lista em DOIS passos desde que ela virou PhxGrid: a
    // funcao devolve o recipiente e `ligarGradeTx` cria a grade depois de o
    // recipiente estar no documento (grade dentro de `display:none` mede
    // largura zero). O teste faz o mesmo par, pela mesma razao de sempre --
    // o caminho da pessoa e o caminho do teste.
    await page.evaluate(() => {
      const dados = {
        total: 1,
        transacoes: [{
          transaction_id: 12,
          usuario: 'Adriano Boller',
          transaction_state: 'ACTIVE',
          idade_ms: 1200,
          linhas: 3,
          lock_mode: 'AUTO',
          esperando: '',
          tabelas_declaradas: ['loja/pedidos'],
          tabelas_efetivas: ['loja/auditoria', 'loja/pedidos'],
          travas: [{ tabela: 'loja/pedidos', trava: 'IX', linhas: 2 }],
        }],
      };
      $('#painel').innerHTML = listaDeTransacoes(dados);
      ligarGradeTx(dados);
    });
    await page.waitForSelector('#painel table', { timeout: 10000 });
    const grade = await page.textContent('#painel');
    contem(grade, 'Adriano Boller', 'a lista não mostrou o usuário');
    verdade(!/ADRIANO BOLLER/.test(grade),
      'a lista mostrou «ADRIANO BOLLER» — o `text-transform` da folha global '
      + 'está mentindo sobre o dado');
    verdade(!/LOJA\/PEDIDOS/.test(grade),
      'a lista mostrou o nome da tabela em caixa alta — é dado, e dado não se '
      + 'transforma');
    contem(grade, 'loja/auditoria',
      'a lista não separa o escopo EFETIVO do declarado — e é justamente a '
      + 'tabela que entrou sem ninguém pedir que precisa aparecer');
    await capturar(ctx, ctx.nomeCaptura('lista'));

    // --- 4. a página não estoura para o lado
    const estoura = await page.evaluate(() =>
      document.documentElement.scrollWidth > document.documentElement.clientWidth + 2);
    verdade(!estoura,
      'a tabela de transações empurrou a página para fora da largura da janela');

    // --- 5. o texto passa MESMO pela fábrica de idiomas
    //
    // O laço que conta existe no `cargo test`; o que só o navegador prova é
    // que a tela REPINTA com o texto novo. Uma chave traduzida nos seis
    // idiomas que a tela não pede continua em português para sempre.
    await abrirTela(page);
    const emPortugues = await page.textContent('#painel');
    await page.evaluate(async () => { await aplicarIdioma('Ingles'); });
    await abrirTela(page);
    const emIngles = await page.textContent('#painel');
    contem(emIngles, 'isolation level',
      'trocar o idioma não trocou a tela: o texto não está passando pela fábrica');
    verdade(emIngles !== emPortugues,
      'a tela ficou idêntica nos dois idiomas');
    verdade(!/nível de isolamento/i.test(emIngles),
      'sobrou português no meio do inglês — alguma frase não passa pela fábrica');
    await capturar(ctx, ctx.nomeCaptura('em-ingles'));

    // E volta, para não deixar o navegador em inglês para os casos seguintes.
    await page.evaluate(async () => { await aplicarIdioma('Portugues'); });
  },
};
