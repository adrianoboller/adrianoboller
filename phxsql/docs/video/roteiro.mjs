/* Roteiro do vídeo do PhxSql: do login à replicação, com legenda em cima.
 *
 * O Playwright grava WebM; o ffmpeg converte para MP4 depois. A legenda é uma
 * faixa injetada na própria página -- assim ela entra no vídeo sem depender de
 * edição posterior. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';

const S = '/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/video';
const b = await chromium.launch();
const ctx = await b.newContext({
  viewport: { width: 1600, height: 900 },
  colorScheme: 'dark',
  recordVideo: { dir: `${S}/bruto`, size: { width: 1600, height: 900 } },
});
const p = await ctx.newPage();
const erros = []; p.on('pageerror', e => erros.push(e.message));

const esperar = ms => p.waitForTimeout(ms);

/* A faixa de legenda: um capítulo, uma frase e um relógio de progresso. */
async function faixa() {
  await p.evaluate(() => {
    if (document.getElementById('faixaDemo')) return;
    const d = document.createElement('div');
    d.id = 'faixaDemo';
    d.innerHTML = `<div id="faixaCap"></div><div id="faixaTxt"></div>`;
    d.style.cssText = `position:fixed;left:0;right:0;bottom:0;z-index:99999;
      background:linear-gradient(0deg,rgba(1,4,24,.97),rgba(1,4,24,.88));
      border-top:2px solid #ff4d10;padding:14px 34px 16px;
      font-family:'Exo 2',system-ui,sans-serif;color:#dde2eb;
      box-shadow:0 -14px 40px rgba(1,4,24,.7)`;
    document.body.appendChild(d);
    document.getElementById('faixaCap').style.cssText =
      `font-family:'IBM Plex Mono',monospace;font-size:12px;letter-spacing:.18em;
       text-transform:uppercase;color:#ff8a1c;margin-bottom:5px`;
    document.getElementById('faixaTxt').style.cssText =
      `font-size:22px;line-height:1.32;font-weight:500;max-width:1400px`;
  });
}

let capAtual = '';
async function diz(cap, txt, ms = 3200) {
  if (cap) { capAtual = cap; console.log('>>', cap); }
  await faixa();
  await p.evaluate(([c, t]) => {
    document.getElementById('faixaCap').textContent = c;
    document.getElementById('faixaTxt').textContent = t;
  }, [capAtual, txt]);
  await esperar(ms);
}

async function api(op, args = {}) {
  return await p.evaluate(([o, a]) => api(o, a), [op, args]);
}

/* Uma cena que falha nao pode derrubar o video inteiro: ela vira uma linha no
   log e o roteiro segue. Foi assim que a gravacao anterior se perdeu inteira
   por causa de um passo so. */
async function cena(nome, fn) {
  try {
    await fn();
  } catch (e) {
    console.log('   !! cena falhou:', nome, '|', String(e.message).split('\n')[0]);
  }
}

// ---------------------------------------------------------------- 01 login
await p.goto('http://127.0.0.1:8900/');
await esperar(1200);
await diz('01 · Entrar', 'PhxSql 0.15.0 — motor de dados em Rust, sem nenhuma dependência externa.', 3800);
await diz('', 'A senha não trafega: o servidor manda um desafio, o cliente devolve o HMAC.', 3600);
await p.fill('#u', 'adm'); await esperar(500);
await p.fill('#s', 'segredo1'); await esperar(500);
await p.fill('#t', 'demo'); await esperar(600);
await p.locator('button:has-text("Entrar")').click();
await p.waitForSelector('#app.ativo');
await esperar(1500);
await diz('', 'Dentro. A barra de ferramentas traz 23 ferramentas; a barra de menu, nove menus.', 3800);

// ------------------------------------------------------- 02 criar database
await diz('02 · Criar o banco', 'Um database é um diretório. Uma tabela são sete arquivos dentro dele.', 3600);
await api('criar_database', { database: 'Comercial' });
await p.evaluate(() => montarArvore(false));
await esperar(600);
await p.evaluate(() => verBancos());
await esperar(2600);
await diz('', 'Comercial criado. Agora a tabela — com tipos, chave primária e índices.', 3400);

// ---------------------------------------------------------- 03 criar tabela
await diz('03 · Criar a tabela', 'Sem SQL: o protocolo é JSON por linha. Uma operação, um objeto, uma resposta.', 3800);
await api('criar_tabela', {
  database: 'Comercial', tabela: 'cadastroClientes', motivo_obrigatorio: true,
  colunas: [
    { nome: 'id', tipo: 'Int4', obrigatoria: true },
    { nome: 'nome', tipo: 'Str(40)', obrigatoria: true },
    { nome: 'cidade', tipo: 'Str(30)' },
    { nome: 'uf', tipo: 'Str(2)' },
    { nome: 'limite', tipo: 'Decimal(12,2)' },
    { nome: 'cadastro', tipo: 'Date' },
    { nome: 'ficha', tipo: 'Memo' },
  ],
  indices: [
    { nome: 'porId', colunas: ['id'], unico: true, primario: true },
    { nome: 'porNome', colunas: ['nome'], nocase: true },
    { nome: 'porCidade', colunas: ['cidade'] },
  ],
});
await p.evaluate(() => montarArvore(false));
await esperar(500);
await p.evaluate(() => verDatabase('Comercial'));
await esperar(2800);
await diz('', 'Sete colunas declaradas — e o motor acrescentou duas: softdeleted e rownum.', 4000);

// ------------------------------------------------------------ 04 inserir
await diz('04 · Inserir', 'Uma linha por vez: o caminho normal, com a ficha de edição.', 3400);
await p.evaluate(() => verConteudoEditavel('Comercial', 'cadastroClientes'));
await esperar(1800);
await p.locator('#btNova').click();
await esperar(1600);
for (const [sel, val] of [['id', '1'], ['nome', 'Adriano Boller'],
                          ['cidade', 'Blumenau'], ['uf', 'SC'],
                          ['limite', '1500.00'], ['cadastro', '2024-10-04']]) {
  const campo = p.locator(`[name="${sel}"], #f_${sel}`).first();
  if (await campo.count()) { await campo.fill(val); await esperar(320); }
}
await esperar(900);
await p.locator('#f_ficha').fill('Cliente desde 2024. Ficha longa mora no .memo, fora do registro.');
await esperar(900);
await diz('', 'O decimal viaja como TEXTO. Um f64 não representa 1500,00 exatamente.', 3800);
await p.locator('#btSalvar').click();
await esperar(1800);
await diz('', 'Gravada. rowid 1, rownum 1 — e os dois são números diferentes por natureza.', 3600);

// ------------------------------------------------------- 05 carga em lote
await diz('05 · Carga em lote', 'Mil linhas com mil pedidos custa mil aberturas, mil travas e mil fsync.', 3800);
await p.evaluate(() => telaImportar('Comercial', 'cadastroClientes'));
await esperar(1600);
const csv = ['id;nome;cidade;uf;limite;cadastro',
  '2;Maria Silva;Joinville;SC;2.500,00;2024-11-12',
  '3;João Souza & Cia;Itajaí;SC;990,50;2025-01-20',
  '4;Ana Prado;Curitiba;PR;12.000,00;2025-03-02',
  '5;Pedro Lima;Florianópolis;SC;450,00;2025-04-15',
  '6;Rita Nunes;Blumenau;SC;7.800,25;2025-06-30'].join('\n');
await p.locator('#impTexto').fill(csv);
await esperar(1400);
await diz('', 'Cinco formatos: JSON, CSV, TXT, XML e HTML. O motor adivinha qual é.', 3600);
await p.locator('#btPrever').click();
await esperar(1800);
await diz('', 'Conferir lê sem gravar. O botão de gravar só acende depois que passa.', 3800);
await p.locator('#btImportar').click();
await esperar(2200);
await diz('', '"2.500,00" virou 2500,00 — o último separador é o decimal. "1.500" fica como está: é ambíguo.', 4600);

// ------------------------------------------- 06 milhares de linhas + página
await diz('06 · Vinte mil linhas', 'Agora em escala: vinte mil linhas em quatro lotes, direto pelo protocolo.', 3600);
for (let bloco = 0; bloco < 4; bloco++) {
  const r = await p.evaluate(async (b) => {
    const cid = [['Blumenau','SC'],['Joinville','SC'],['Itajaí','SC'],['Curitiba','PR'],['Porto Alegre','RS']];
    const linhas = [];
    for (let k = b * 5000 + 7; k < (b + 1) * 5000 + 7; k++) {
      const c = cid[k % 5];
      linhas.push({ id: k, nome: `Cliente ${String(k).padStart(7,'0')}`,
                    cidade: c[0], uf: c[1], limite: `${(k % 9000) + 100}.00`,
                    cadastro: '2025-01-01', ficha: `ficha do cliente ${k}` });
    }
    const x = await api('inserir_lote', { database: 'Comercial', tabela: 'cadastroClientes', linhas });
    return x && (x.gravadas ?? (x.rowids ? x.rowids.length : 0));
  }, bloco);
  console.log(`  lote ${bloco + 1}/4: ${r}`);
  await esperar(400);
}
await p.evaluate(() => verConteudoEditavel('Comercial', 'cadastroClientes'));
await esperar(2000);
await diz('', 'Vinte mil. O "de quantas" saiu de dois contadores do cabeçalho — sem varrer nada.', 4200);
await diz('07 · Paginação', 'Próxima e anterior andam por CURSOR: o custo é o da página, não o da tabela.', 3800);
for (let i = 0; i < 3; i++) { await p.locator('#pgDepois').click(); await esperar(900); }
await diz('', 'E o salto por POSIÇÃO: quando a posição é o rownum, o começo sai de uma bissecção.', 4200);
await p.locator('#pgIr').fill('80');
await p.locator('#pgIr').press('Enter');
await esperar(2000);
await diz('', 'Página 80 de 100, na hora. Numa tabela de 200 mil: 6 ms contra 131 andando até lá.', 4400);
await p.locator('#pgFim').click();
await esperar(1800);

// --------------------------------------------------------- 08 alterar
await diz('08 · Alterar', 'Clicar numa linha abre a ficha. Alterar não renumera: o rownum é herdado.', 3800);
await p.evaluate(() => abrirFicha('Comercial', 'cadastroClientes', 2));
await esperar(1600);
await p.locator('#f_cidade').fill('Bruxelas');
await esperar(700);
await p.locator('#f_limite').fill('9999.99');
await esperar(900);
await p.locator('#btSalvar').click();
await esperar(1800);

// ------------------------------------------- 09 as duas exclusões e a lixeira
await diz('09 · Excluir', 'Excluir virou duas coisas, e o padrão do protocolo é o REVERSÍVEL.', 3800);
await p.evaluate(() => abrirFicha('Comercial', 'cadastroClientes', 3));
await esperar(1500);
await p.locator('#btExcluir').click();
await esperar(1600);
await diz('', 'Esta tabela exige motivo escrito. A escolha é da tabela, feita na criação.', 3800);
await p.locator('#excMotivo').fill('pedido de remoção do titular');
await esperar(1300);
await p.locator('#btExcSim').click();
await esperar(2000);
await diz('', 'Marcada. A linha continua INTEIRA no .reg, com os anexos — e some das listas.', 4000);
await p.evaluate(() => verConteudoEditavel('Comercial', 'cadastroClientes', true));
await esperar(2200);
await diz('', 'A visão das excluídas, com o botão de restaurar. Marcar sem desmarcar seria só perder o dado.', 4400);
await p.evaluate(() => telaMotivos('Comercial', 'cadastroClientes'));
await esperar(2400);
await diz('', 'O .reason guarda o porquê, quem, quando — e a IDENTIDADE da linha em texto.', 4000);
await diz('', '"rowid 4173" não diz nada seis meses depois. A chave primária diz.', 3800);

// ------------------------------------------------ 10 exclusão física + trash
await diz('', 'A exclusão FÍSICA existe, mas se escreve. E ela passa pelo .trash antes.', 3800);
await api('excluir', { database: 'Comercial', tabela: 'cadastroClientes', rowid: 4,
                       fisico: true, motivo: 'duplicidade com o contrato 8812' });
await p.evaluate(() => telaLixeira('Comercial', 'cadastroClientes'));
await esperar(2400);
await diz('', 'A linha é gravada aqui e o DISCO CONFIRMA antes de o slot ser liberado.', 4000);
await diz('', 'Entre perder o dado e duplicá-lo, o motor duplica: duplicidade se resolve olhando.', 4200);
await diz('', 'Lixeira, motivos e diário só quem administra lê — um motivo revela mais que a linha.', 4400);

// ------------------------------------------------------------ 11 consultar
await cena('consultar', async () => {
  await diz('11 · Consultar', 'Aqui vem a parte honesta: NÃO HÁ SQL no PhxSql. Nenhum. O protocolo é JSON.', 4400);
  await diz('', 'Sem SQL não há injeção de SQL — a superfície simplesmente não existe.', 3800);
  await p.evaluate(() => abrirConsulta());
  await esperar(1600);
  await p.locator('#cDb').fill('Comercial');
  await p.locator('#cTab').fill('cadastroClientes');
  await p.locator('#cCol').fill('cidade');
  await esperar(500);
  await p.locator('#cVal').fill('Curitiba');
  await esperar(900);
  await p.locator('#btConsultar').click();
  await esperar(2600);
  await diz('', 'A tabela vai para a RAM com mapas por coluna. 87× o disco, medido.', 4000);
  await diz('', 'A resposta traz "examinadas" e "us": os dois números que dizem se a consulta está boa.', 4200);
});

// ------------------------------------------------------------ 12 integridade
await cena('integridade', async () => {
  await diz('12 · Integridade', 'Verificar confere o CRC de cada registro, cada página de índice e cada bloco.', 4000);
  await p.evaluate(() => { est.atual = { db: 'Comercial', tab: 'cadastroClientes' }; return verificarTabela(); });
  await esperar(3200);
  await diz('', 'E confere se a contagem de chaves de cada índice bate com a de registros vivos.', 4000);
});

// ------------------------------------------------------------- 13 backup
await cena('backup', async () => {
  await diz('13 · Backup', 'Cópia com manifesto SHA-256, e um ZIP com DEFLATE escrito aqui dentro.', 4000);
  await p.evaluate(() => verBackupRestaure('Comercial'));
  await esperar(2800);
  await diz('', 'O manifesto é o que transforma "tenho uma cópia" em "tenho uma cópia que confere".', 4200);
});

// ---------------------------------------------------------- 14 replicação
await cena('replicacao', async () => {
  await diz('14 · Replicação', 'O .log sempre foi o binlog. Faltava a IMAGEM DA LINHA dentro do evento.', 4200);
  await p.evaluate(() => verReplicacao());
  await esperar(3000);
  await diz('', 'Papel source, imagem ligada. O evento N É a posição N — não há GTID a inventar.', 4400);
  await diz('', 'Agora a réplica, na porta 8901. Ela é quem procura: o master não empurra nada.', 4000);
});

// ------------------------------------------------ 15 a réplica, em outra aba
await cena('replica', async () => {
  const p2 = await ctx.newPage();
  await p2.goto('http://127.0.0.1:8901/');
  await esperar(1600);
  await p2.fill('#u', 'adm'); await p2.fill('#s', 'segredo1'); await p2.fill('#t', 'demo');
  await p2.locator('button:has-text("Entrar")').click();
  await p2.waitForSelector('#app.ativo');
  await esperar(1800);
  await p2.evaluate(() => verReplicacao());
  await esperar(3200);
  await p2.evaluate(() => verConteudoEditavel('Comercial', 'cadastroClientes'));
  await esperar(3200);
  await p2.close();
  await p.bringToFront();
});
await cena('replica-legenda', async () => {
  await diz('', 'A réplica montou a tabela sozinha, do bloco de esquema do master, e aplicou tudo.', 4400);
  await diz('', 'Os rowids saem IGUAIS sem ninguém os transmitir: o .reg nunca reaproveita slot.', 4400);
  await diz('', 'Se não saírem iguais, ela divergiu — e a replicação para ali, em vez de espalhar.', 4400);
});

// -------------------------------------------------------- 16 o que falta
await cena('o que falta', async () => {
  await diz('16 · O que ainda falta', 'A parte que nenhum vídeo de produto costuma mostrar.', 3600);
  await p.evaluate(() => verTransacoes());
  await esperar(2800);
  await diz('', 'Não há transação. A inserção desfaz o que gravou se um índice falhar — só isso.', 4200);
  await diz('', 'Não há gatilho, procedimento guardado nem job. Os três foram pedidos, nenhum começou.', 4400);
  await diz('', 'Não há camada SQL, ODBC nem OLE DB. Esta é a camada de armazenamento.', 4000);
  await diz('', 'E a réplica aplica mais devagar que o master escreve: 4.273/s contra 18.773/s.', 4400);
});

// ------------------------------------------------------------- 17 o painel
await cena('painel', async () => {
  await diz('17 · O painel', 'O servidor inteiro numa tela — sete gráficos, numa chamada só.', 3800);
  await p.evaluate(() => montarArvore(true));
  await esperar(3400);
  await diz('', 'PhxSql 0.15.0 · 42.790 linhas de Rust, zero dependências externas, 574 testes.', 4600);
  await diz('', 'Built to store. Engineered to scale.', 4400);
  await esperar(1400);
});

console.log('erros de página:', erros.length ? erros : 'nenhum');
await ctx.close();
await b.close();
console.log('vídeo bruto em', `${S}/bruto`);
