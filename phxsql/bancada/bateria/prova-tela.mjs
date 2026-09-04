/* A mesma bateria, pela TELA -- porque metade dos defeitos desta casa so
 * apareceu no navegador.
 *
 * Nao sobe servidor nenhum: quem sobe e o `prova-bateria.py --tela`, que ja
 * montou o banco, as tabelas, as chaves e os gatilhos pelo soquete e sabe
 * matar o processo que criou. Aqui so se OLHA e se CLICA.
 *
 *   node bancada/bateria/prova-tela.mjs <porta-web> <porta-dados> <token> <senha> [tiros]
 *
 * O que ela prova, e cada passo com o esperado escrito antes:
 *
 *  1. entrar pelo desafio-resposta (a senha nao sai da maquina);
 *  2. o database e as quatro tabelas aparecem na arvore;
 *  3. a grade da tabela de chave Uuid: as colunas de sistema NAO viram
 *     colunas, e o `rownum` aparece como a coluna de ordem;
 *  4. a Estrutura mostra o tipo Uuid, a chave primaria e a estrangeira;
 *  5. a ficha nova de uma tabela de chave Uuid GRAVA -- e este passo e o que
 *     apanhava: o campo dizia «em branco ... gera um v7» e em branco o
 *     servidor recusava a linha inteira;
 *  6. o SIGNAL de um gatilho vira recado legivel na tela, com a frase do dono.
 *
 * Sai com 0 se tudo passou, 1 se algum passo falhou. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';

const [portaWeb, portaDados, token, senha, tiros] = [
  process.argv[2] || '6301', process.argv[3] || '6300',
  process.argv[4] || 'bateria', process.argv[5] || 'adm-1234',
  process.argv[6] || '',
];

const falhas = [];
const confere = (rotulo, visto, esperado) => {
  const ok = JSON.stringify(visto) === JSON.stringify(esperado);
  console.log(`  ${ok ? 'ok  ' : 'ERRO'} ${rotulo}: ${JSON.stringify(visto)}`
    + (ok ? '' : `   (esperava ${JSON.stringify(esperado)})`));
  if (!ok) falhas.push(rotulo);
};
const contem = (rotulo, texto, pedaco) => {
  const ok = String(texto || '').toLowerCase().includes(pedaco.toLowerCase());
  console.log(`  ${ok ? 'ok  ' : 'ERRO'} ${rotulo}: ${JSON.stringify(String(texto || '').slice(0, 140))}`
    + (ok ? '' : `   (esperava conter ${JSON.stringify(pedaco)})`));
  if (!ok) falhas.push(rotulo);
};

const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 1600, height: 1000 } });
const p = await ctx.newPage();
// Erro de pagina nao derruba a prova: ele vira um passo que falha no fim, e
// assim os outros ainda dizem o que sabem.
const erros = [];
p.on('pageerror', e => erros.push('pageerror: ' + e.message));
// So conta o que sai DESTA pagina. A folha de fonte do Google nao carrega em
// maquina sem saida para a internet, e reprovar a bateria por isso seria
// reprovar a rede, nao o produto -- mas um 400 vindo do NOSSO servidor e
// defeito, e por isso a resposta entra com o codigo e a URL.
const daCasa = u => u.includes('127.0.0.1') || u.startsWith('/');
p.on('response', r => {
  if (r.status() >= 400 && daCasa(r.url())) erros.push(`HTTP ${r.status()} em ${r.url()}`);
});
p.on('requestfailed', r => {
  if (daCasa(r.url())) erros.push(`pedido falhou: ${r.url()} (${r.failure().errorText})`);
});
const esperar = ms => p.waitForTimeout(ms);
const tiro = async n => { if (tiros) await p.screenshot({ path: `${tiros}/${n}.png` }); };

try {
  console.log('\n=== T1. entrar ===\n');
  await p.goto(`http://127.0.0.1:${portaWeb}/`);
  await esperar(800);
  await p.fill('#pt', portaDados);
  await p.fill('#u', 'adm');
  await p.fill('#s', senha);
  await p.fill('#t', token);
  await p.locator('button:has-text("Entrar")').click();
  await p.waitForSelector('#app.ativo', { timeout: 20000 });
  await esperar(1200);
  await tiro('t1-entrou');
  confere('entrou como adm', await p.evaluate(() => est.usuario.login), 'adm');
  // 127.0.0.1 e contexto seguro: o caminho bom (desafio-resposta) tem de ser
  // o que rodou. Cair no Base64 aqui seria a senha viajando sem precisar.
  confere('pelo desafio-resposta', await p.evaluate(() => podeProvar()), true);

  console.log('\n=== T2. o database e as tabelas na arvore ===\n');
  const tabelas = await p.evaluate(() => api('tabelas', { database: 'escola' })
    .then(r => r.tabelas.slice().sort()));
  confere('as cinco tabelas', tabelas.filter(t => !t.startsWith('carga_')),
    ['alunos', 'auditoria', 'cadeia', 'sem_gatilho', 'turmas']);

  console.log('\n=== T3. a grade da tabela de chave Uuid ===\n');
  await p.evaluate(() => verConteudoEditavel('escola', 'alunos'));
  await esperar(1500);
  await tiro('t3-grade');
  // O cabecalho se le pela CHAVE (`data-campo`), e nao pela frase que aparece.
  // Quando esta tela virou PhxGrid, o `<th>` passou a ter o botao de filtro
  // DENTRO dele, e `textContent` virou «cidade▼». Comparar com «cidade»
  // quebrou em dois lugares de uma vez: aqui, e no `indexOf` logo abaixo, que
  // devolvia -1 e fazia a celula da cidade sair `null` no lugar de «none» --
  // uma guarda de dado maiusculo que parecia acusar o CSS e so nao achava a
  // celula. Chave nao tem seta pendurada, e nao muda de idioma.
  const grade = await p.evaluate(() => {
    // A linha de titulos e a ultima que NAO e a linha de filtro; a de filtro
    // repete os mesmos `data-campo` e dobraria a contagem.
    const linhas = [...document.querySelectorAll('.phx-grid thead tr:not(.phx-frow)')];
    const th = [...(linhas.at(-1)?.querySelectorAll('th') ?? [])];
    return {
      campos: th.map(x => x.getAttribute('data-campo')),
      // O rotulo VISIVEL sai do span do titulo, sem o botao de filtro junto.
      rotulos: th.map(x => (x.querySelector('.phx-th-titulo') || x).textContent.trim()),
    };
  });
  // A de sistema que se ESCONDE e a `softdeleted`; a que se MOSTRA, como
  // coluna de ordem, e o `rownum`. Nenhuma das duas vira coluna de dado.
  confere('softdeleted nao e coluna da grade', grade.campos.includes('softdeleted'), false);
  confere('as colunas declaradas estao la',
    ['id', 'turma_id', 'nome', 'cidade', 'nota'].every(c => grade.campos.includes(c)), true);
  contem('e o cabecalho traz a coluna de ordem', grade.rotulos.join(' | '), 'Nº');
  // A cidade tem de sair como esta GRAVADA. Maiuscula por CSS seria uma
  // mentira sobre o dado -- ja aconteceu com «Blumenau».
  // A celula do CORPO nao carrega `data-campo` -- so o cabecalho carrega --,
  // entao a coluna se acha pela POSICAO, e a posicao sai da chave.
  const cidadeNaGrade = await p.evaluate(i => {
    const tr = document.querySelector('.phx-grid tbody tr:not(.phx-grupo)');
    if (i < 0 || !tr) return null;
    const td = tr.querySelectorAll('td')[i];
    if (!td) return null;
    return { texto: td.textContent.trim(), caixa: getComputedStyle(td).textTransform };
  }, grade.campos.indexOf('cidade'));
  confere('a grade nao troca a caixa do dado', cidadeNaGrade && cidadeNaGrade.caixa, 'none');

  console.log('\n=== T4. a Estrutura mostra a chave e a estrangeira ===\n');
  await p.evaluate(() => abrirTabela('escola', 'alunos'));
  await esperar(1500);
  await tiro('t4-estrutura');
  const estrutura = await p.evaluate(() => document.querySelector('#painel').innerText);
  contem('o tipo Uuid aparece', estrutura, 'Uuid');
  contem('a chave estrangeira aparece', estrutura, 'fk_turma');
  contem('e diz para onde aponta', estrutura, 'turmas');

  console.log('\n=== T5. a ficha nova de uma tabela de chave Uuid GRAVA ===\n');
  await p.evaluate(() => verConteudoEditavel('escola', 'alunos'));
  await esperar(1200);
  await p.locator('#btNova').click();
  await esperar(1000);
  await tiro('t5-ficha-nova');
  const idNaFicha = await p.evaluate(() => {
    const e = document.querySelector('#f_id');
    return { valor: e ? e.value : null, dica: e ? e.placeholder : null };
  });
  // O defeito: o campo vinha VAZIO dizendo que em branco geraria um v7 -- e em
  // branco o servidor recusa com «obrigatoria e recebeu NULL».
  confere('o id da linha nova ja vem pedindo um v7', idNaFicha.valor, 'novo');
  contem('e a dica diz a verdade', idNaFicha.dica, 'novo');
  // E SO a chave primaria. A chave ESTRANGEIRA e Uuid tambem, e preenche-la
  // com `novo` geraria um id sorteado apontando para turma nenhuma: um campo
  // em branco quem olha corrige, um campo com a resposta errada nao.
  const fkNaFicha = await p.evaluate(() => document.querySelector('#f_turma_id').value);
  confere('a chave estrangeira NAO vem preenchida', fkNaFicha, '');

  const turma = await p.evaluate(() => api('varrer',
    { database: 'escola', tabela: 'turmas', max: 5 }).then(r => r.linhas[0].id));
  await p.fill('#f_turma_id', turma);
  await p.fill('#f_nome', 'Joana pela tela');
  await p.fill('#f_cidade', '  itajai ');
  await p.fill('#f_nota', '9.50');
  await p.locator('#btSalvar').click();
  await esperar(1800);
  await tiro('t5-depois-de-incluir');
  const entrou = await p.evaluate(() => api('varrer',
    { database: 'escola', tabela: 'alunos', max: 5000 })
    .then(r => r.linhas.filter(l => l.nome === 'Joana pela tela')));
  confere('a linha entrou pela tela', entrou.length, 1);
  confere('com um id v7 de verdade', entrou[0] && entrou[0].id.charAt(14), '7');
  // O gatilho BEFORE roda no caminho da tela igual ao do soquete.
  confere('e o gatilho normalizou o que veio da tela',
    entrou[0] && entrou[0].cidade, 'ITAJAI');

  console.log('\n=== T6. o SIGNAL do gatilho vira recado na tela ===\n');
  await p.evaluate(() => verConteudoEditavel('escola', 'alunos'));
  await esperar(1200);
  await p.locator('#btNova').click();
  await esperar(900);
  await p.fill('#f_turma_id', turma);
  await p.fill('#f_nome', 'Sem nota pela tela');
  await p.fill('#f_cidade', 'gaspar');
  await p.fill('#f_nota', '');       // e o que o gatilho recusa
  // Daqui para a frente UM erro de rede e esperado: a recusa do gatilho volta
  // como 400 no HTTP, que e o que uma operacao recusada e. O que se confere e
  // que veio SO ela.
  const errosAntes = erros.length;
  // A MOLDURA da sprint (`[SP000021] `) abre toda recusa DE PROPOSITO -- entrou
  // em d4f8563, no molde do `ERROR 1064 (42000)` do MySQL, e a mesma resposta
  // traz a sprint tambem no campo `sprint`, justamente para ninguem ter de
  // recortar a frase de volta. Entao a moldura se LE do campo, e nao se digita
  // aqui: numero digitado a mao envelhece calado, e este ja envelheceu uma
  // vez -- foi a moldura nova que fez esta guarda reprovar sem que nada de
  // errado tivesse acontecido com a frase do dono.
  const sprints = [];
  const anotarSprint = async r => {
    if (!r.url().endsWith('/api')) return;
    const j = await r.json().catch(() => null);
    if (j && j.ok === false && j.sprint) sprints.push(j.sprint);
  };
  p.on('response', anotarSprint);
  await p.locator('#btSalvar').click();
  await esperar(1800);
  p.off('response', anotarSprint);
  const moldura = sprints.length ? `[${sprints.at(-1)}] ` : '';
  await tiro('t6-recusado');
  const recado = await p.evaluate(() => {
    const a = document.querySelector('#aviso');
    return a && !a.hidden ? a.textContent.trim() : '';
  });
  // Depois da moldura vem a frase INTEIRA do dono, e nada mais nosso no meio:
  // `String(erro)` do JavaScript punha «Error: » ali -- o nome da classe, em
  // ingles, no lugar onde esta a regra que a pessoa acabou de esbarrar. A
  // guarda continua sendo essa; o que mudou e que a moldura, que e nossa e
  // declarada, entra medida em vez de digitada.
  confere('e a resposta traz a sprint como CAMPO, e nao so na frase',
    /^SP\d{6}$/.test(sprints.at(-1) || ''), true);
  confere('o recado comeca com a frase do dono, sem prefixo nosso',
    recado.startsWith(moldura + 'aluno sem nota nao entra'), true);
  contem('e ela chega inteira', recado, 'SIGNAL SQLSTATE 45000');
  const naoEntrou = await p.evaluate(() => api('varrer',
    { database: 'escola', tabela: 'alunos', max: 5000 })
    .then(r => r.linhas.filter(l => l.nome === 'Sem nota pela tela')));
  confere('e a linha recusada NAO entrou', naoEntrou.length, 0);
  confere('a recusa volta como 400, e so ela', erros.slice(errosAntes),
    [`HTTP 400 em http://127.0.0.1:${portaWeb}/api`]);
  erros.length = errosAntes;
} catch (e) {
  console.log('  ERRO a prova pela tela parou:', String(e.message).split('\n')[0]);
  falhas.push('a prova pela tela parou');
  await tiro('erro');
}

confere('nenhum erro de pagina', erros.slice(0, 5), []);
await b.close();

console.log('\n' + '='.repeat(66));
if (falhas.length) {
  console.log(`${falhas.length} PASSO(S) DA TELA FALHARAM:`);
  falhas.forEach(f => console.log('  -', f));
  process.exit(1);
}
console.log('a bateria da tela passou inteira');
