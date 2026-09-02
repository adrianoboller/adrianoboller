/* A telemetria responde enquanto o servidor escreve?
 *
 * A tela do gestor de threads depende de uma volta do relogio de 2 s trazer a
 * lista. Se a op `telemetria` ficar atras da trava global de dados durante uma
 * carga, a tela fica vazia -- e ai o vermelho da bateria e do PRODUTO, e nao
 * do teste. Isto mede, em vez de supor. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { subir, PORTA_WEB } from '/home/user/adrianoboller/phxsql/testes-web/servidor.mjs';
import { entrar, api, cenario, bancoDoCaso } from '/home/user/adrianoboller/phxsql/testes-web/apoio.mjs';

const srv = await subir({ phxsqld: '/home/user/adrianoboller/phxsql/target/release/phxsqld' });
const nav = await chromium.launch();
const page = await (await nav.newContext()).newPage();
try {
  await entrar(page, `http://127.0.0.1:${PORTA_WEB}/`);
  const db = bancoDoCaso({ tema: 'claro' }, 'Carga');
  const { tab } = await cenario(page, db);

  const medir = async (rotulo, n) => {
    const t = [];
    for (let i = 0; i < n; i++) {
      const t0 = Date.now();
      await api(page, 'telemetria', {});
      t.push(Date.now() - t0);
    }
    t.sort((a, b) => a - b);
    console.log(`${rotulo}: mediana ${t[Math.floor(n/2)]} ms | pior ${t[n-1]} ms | n=${n}`);
    return t[n - 1];
  };

  const parado = await medir('servidor PARADO ', 12);

  // Agora com escrita pesada em paralelo: lotes de 2.000 linhas sem esperar.
  const carga = (async () => {
    for (let v = 0; v < 6; v++) {
      const linhas = [];
      for (let k = 0; k < 2000; k++) {
        linhas.push([1000 + v * 2000 + k, `nome ${k}`, 'Blumenau', 'SC', '10.00', '2025-01-01', '']);
      }
      await api(page, 'inserir_lote', { database: db, tabela: tab, linhas });
    }
  })();
  const carregado = await medir('servidor ESCREVENDO', 12);
  await carga;

  console.log(`\npior sob carga / pior parado = ${(carregado / Math.max(parado, 1)).toFixed(1)}x`);
  console.log(carregado > 15000
    ? 'ACIMA do limite de 15 s do caso -> o vermelho da bateria e de PRODUTO'
    : 'ABAIXO do limite de 15 s do caso -> a telemetria nao e o que estoura');
} finally { await nav.close(); srv.derrubar(); }
