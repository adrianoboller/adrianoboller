# Make each scene resilient
# 28/08 22:09

import pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/video/roteiro.mjs")
s = p.read_text()

# 1. helper de cena resiliente, logo depois de `api`
antigo = """async function api(op, args = {}) {
  return await p.evaluate(([o, a]) => api(o, a), [op, args]);
}"""
novo = """async function api(op, args = {}) {
  return await p.evaluate(([o, a]) => api(o, a), [op, args]);
}

/* Uma cena que falha nao pode derrubar o video inteiro: ela vira uma linha no
   log e o roteiro segue. Foi assim que a gravacao anterior se perdeu inteira
   por causa de um passo so. */
async function cena(nome, fn) {
  try {
    await fn();
  } catch (e) {
    console.log('   !! cena falhou:', nome, '|', String(e.message).split('\\n')[0]);
  }
}"""
assert antigo in s
s = s.replace(antigo, novo)

# 2. da secao 11 em diante, tudo dentro de cenas
i = s.index("// ------------------------------------------------------------ 11 consultar")
cabeca = s[:i]

cauda = '''// ------------------------------------------------------------ 11 consultar
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
  await diz('', 'PhxSql 0.15.0 · 42.790 linhas de Rust, zero dependências externas, 573 testes.', 4600);
  await diz('', 'Built to store. Engineered to scale.', 4400);
  await esperar(1400);
});

console.log('erros de página:', erros.length ? erros : 'nenhum');
await ctx.close();
await b.close();
console.log('vídeo bruto em', `${S}/bruto`);
'''
s = cabeca + cauda
p.write_text(s)
print("ok")
