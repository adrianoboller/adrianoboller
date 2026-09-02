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

/** A porta responde NESTE instante? (o `esperarPorta` insiste; esta pergunta.) */
function portaAberta(porta) {
  return new Promise(r => {
    const s = connect(porta, '127.0.0.1');
    s.on('connect', () => { s.destroy(); r(true); });
    s.on('error', () => r(false));
    s.setTimeout(400, () => { s.destroy(); r(false); });
  });
}

/** Sobe um phxsqld isolado. Devolve o que a bateria precisa para o derrubar. */
export async function subir({ phxsqld, portaDados = PORTA_DADOS, portaWeb = PORTA_WEB, log }) {
  const dir = mkdtempSync(join(tmpdir(), 'phx-bateria-'));
  const base = join(dir, 'dados');
  const caminhoConfig = join(dir, 'config.json');
  writeFileSync(caminhoConfig,
    JSON.stringify(config(base, hashDaSenha(phxsqld, SENHA), portaDados, portaWeb), null, 2));

  // PORTA JA OCUPADA E MEDIÇÃO DE OUTRO SERVIDOR.
  //
  // Sem esta conferencia o buraco e silencioso e caro: se sobrou um `phxsqld`
  // de uma corrida anterior segurando a porta, o que subir agora MORRE ao
  // tentar prende-la -- e o `esperarPorta` acha a porta aberta do mesmo jeito,
  // porque quem responde e o VELHO. A bateria entao dirige o navegador contra
  // um servidor com o estado acumulado de rodadas passadas, e as falhas saem
  // parecendo defeito de produto: «a tabela pacientes ja existe», «a arvore
  // esperava 26 bancos e achou 25». Custou duas corridas inteiras para
  // aparecer, e as duas mediram outro servidor.
  //
  // E a mesma familia do binario velho, que esta bateria ja recusa: medidor
  // que mede a coisa errada e pior que medidor que nao roda.
  if (await portaAberta(portaWeb) || await portaAberta(portaDados)) {
    throw new Error(
      `a porta ${portaWeb} ou ${portaDados} ja esta ocupada -- provavelmente um `
      + 'phxsqld de uma corrida anterior que nao caiu. A bateria RECUSA subir '
      + 'assim: o servidor novo morreria e ela mediria o velho, com o estado '
      + 'acumulado dele.\n'
      + "Ache com:  ps -eo pid,etime,cmd | grep 'phxsqld --config /tmp/phx-bateria-'\n"
      + 'e derrube pelo PID (nunca por `pkill -f`, que pega servidor de outro agente).');
  }

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
  // O cinto, alem do suspensorio: a porta pode abrir e o NOSSO processo ter
  // morrido -- e ai quem responde e outro. Perguntar pela porta responde
  // «tem servidor»; so o `morreu` responde «e o MEU servidor».
  if (morreu !== null) {
    throw new Error(
      `o phxsqld que esta bateria subiu morreu com codigo ${morreu}, mas a porta `
      + `${portaWeb} respondeu -- entao quem esta atendendo e outro servidor. `
      + `Nao da para medir assim.\n${saida.join('')}`);
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
