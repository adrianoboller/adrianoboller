// Caminhos e insumos dos roteiros de qualificacao, todos relativos a este arquivo: os roteiros
// nasceram apontando para o scratchpad de uma sessao (que some), e roteiro que so roda numa
// maquina nao e prova -- e anedota.
//
//   --ui DIR   arvore da interface a medir (padrao: apps/phxclaw-ui). Medir em COPIA e o
//              caminho do RED: copia com o conserto desfeito, roteiro apontado para ela.
//   saida      tests/desktop/out/qualificacao/ (ignorado pelo git, como o resto de out/).
import { createRequire } from 'node:module';
import { readFileSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
export const { chromium } = require('/opt/node22/lib/node_modules/playwright');
export const AQUI = dirname(fileURLToPath(import.meta.url));
export const RAIZ = resolve(AQUI, '../../..');
export const DADOS = join(AQUI, '..', 'dados');
export const OUT = join(AQUI, '..', 'out', 'qualificacao');
export const CAP = join(OUT, 'cap');
mkdirSync(CAP, { recursive: true });

const iUi = process.argv.indexOf('--ui');
export const UI = resolve(iUi > 0 ? process.argv[iUi + 1] : join(RAIZ, 'apps/phxclaw-ui'));
export const MEDIR = readFileSync(join(AQUI, 'medir-pagina.js'), 'utf8');
export const GRADE = JSON.parse(readFileSync(join(DADOS, 'grade_bash.json'), 'utf8'));
export const CONFIG = JSON.parse(readFileSync(join(DADOS, 'config_vista.json'), 'utf8'));
export const TELAS = ['geral', 'agentes', 'ide', 'ferramentas', 'absorcao', 'tarefas', 'config'];
