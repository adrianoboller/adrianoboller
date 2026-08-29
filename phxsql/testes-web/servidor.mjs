/* Sobe e derruba UM phxsqld so para a bateria, e derruba pelo PID.
 *
 * Um servidor proprio, num diretorio proprio, com portas proprias: a bateria
 * nao pode depender de um servidor que alguem deixou no ar nem sujar a base
 * de ninguem. E a queda e pelo PID que este modulo guardou -- `pkill -f`
 * mataria tambem o servidor do vizinho que roda a mesma bateria ao lado.
 *
 * A senha NAO fica em texto no config: o hash sai do proprio `phxsqld
 * --senha`, como no `bancada/replicacao/montar.py`. Nao ha uma segunda
 * implementacao de PBKDF2 aqui para divergir da do servidor. */
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { connect } from 'node:net';

/* A faixa reservada a esta bateria. Fora dela nao se encosta. */
export const PORTA_DADOS = 6200;
export const PORTA_WEB = 6201;

export const USUARIO = 'adm';
export const SENHA = 'segredo1';
export const TOKEN = 'bateria';

/** O hash vem do binario, e nao de um PBKDF2 escrito aqui do lado. */
function hashDaSenha(phxsqld, senha) {
  const r = spawnSync(phxsqld, ['--senha'], { input: senha, encoding: 'utf8' });
  const m = /"senha_hash": "([^"]+)"/.exec(r.stdout || '');
  if (!m) throw new Error(`phxsqld --senha nao devolveu o hash: ${r.stdout}${r.stderr}`);
  return m[1];
}

function config(base, hash, portaDados, portaWeb) {
  return {
    base,
    bind: `127.0.0.1:${portaDados}`,
    token: TOKEN,
    max_linhas: 1000,
    // A bateria abre e fecha ficha o tempo todo; sem isto a sessao poderia
    // vencer no meio de um caso longo e o erro sairia como «nao autenticado»,
    // que manda procurar defeito no lugar errado.
    web: { ligado: true, bind: `127.0.0.1:${portaWeb}`, sessao_minutos: 60 },
    recursos: { durabilidade: 'sistema', cache_paginas: 512 },
    usuarios: [{
      id: 10, nome: 'Adriano Boller', login: USUARIO, senha_hash: hash,
      supervisor: true, ativo: true, bases: {},
    }],
    replicacao: { papel: 'isolado' },
  };
}

const dormir = ms => new Promise(r => setTimeout(r, ms));

/** Espera a porta ACEITAR conexao -- «o processo subiu» nao e «a porta abriu». */
async function esperarPorta(porta, prazoMs = 20000) {
  const fim = Date.now() + prazoMs;
  while (Date.now() < fim) {
    const abriu = await new Promise(r => {
      const s = connect({ host: '127.0.0.1', port: porta }, () => { s.destroy(); r(true); });
      s.on('error', () => r(false));
      s.setTimeout(500, () => { s.destroy(); r(false); });
    });
    if (abriu) return true;
    await dormir(150);
  }
  return false;
}

/** Sobe um phxsqld isolado. Devolve o que a bateria precisa para o derrubar. */
export async function subir({ phxsqld, portaDados = PORTA_DADOS, portaWeb = PORTA_WEB, log }) {
  const dir = mkdtempSync(join(tmpdir(), 'phx-bateria-'));
  const base = join(dir, 'dados');
  const caminhoConfig = join(dir, 'config.json');
  writeFileSync(caminhoConfig,
    JSON.stringify(config(base, hashDaSenha(phxsqld, SENHA), portaDados, portaWeb), null, 2));

  const proc = spawn(phxsqld, ['--config', caminhoConfig], {
    cwd: dir, stdio: ['ignore', 'pipe', 'pipe'],
  });
  const saida = [];
  proc.stdout.on('data', d => saida.push(String(d)));
  proc.stderr.on('data', d => saida.push(String(d)));
  let morreu = null;
  proc.on('exit', c => { morreu = c; });

  if (log) log(`servidor pid ${proc.pid} — dados ${portaDados}, web ${portaWeb}, base ${base}`);

  if (!(await esperarPorta(portaWeb))) {
    matar(proc);
    throw new Error(`a porta web ${portaWeb} nao abriu (saida=${morreu}):\n${saida.join('')}`);
  }
  if (!(await esperarPorta(portaDados))) {
    matar(proc);
    throw new Error(`a porta de dados ${portaDados} nao abriu:\n${saida.join('')}`);
  }

  return {
    pid: proc.pid,
    dir,
    base,
    url: `http://127.0.0.1:${portaWeb}/`,
    saida,
    async derrubar() {
      matar(proc);
      // Espera o processo sair de verdade antes de devolver a porta: a
      // proxima rodada tentaria prender a mesma e receberia «endereco em uso».
      for (let i = 0; i < 60 && morreu === null; i++) await dormir(100);
      try { rmSync(dir, { recursive: true, force: true }); } catch { /* o /tmp limpa */ }
    },
  };
}

/** SIGTERM neste PID, e so neste. Nunca `pkill -f`. */
function matar(proc) {
  if (!proc.pid) return;
  try { process.kill(proc.pid, 'SIGTERM'); } catch { /* ja morreu */ }
  setTimeout(() => { try { process.kill(proc.pid, 'SIGKILL'); } catch { /* ok */ } }, 4000).unref();
}
