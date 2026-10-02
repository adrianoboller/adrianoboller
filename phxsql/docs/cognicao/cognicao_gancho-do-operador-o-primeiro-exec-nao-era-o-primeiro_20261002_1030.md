# O gancho do operador: o «primeiro exec do produto» não era o primeiro, e três decisões que parecem detalhe

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 249, meio do SMS: o parecer do papel J mandou implementar o
`alertas.gancho` (`gancho.rs`, `Alertas.gancho`) chamando-o de «PRIMEIRO `exec`
do produto». Ao implementar, a leitura do fonte achou `blacklist.rs`
(`std::process::Command::new(&trocado[0]).args(..).output()`, a regra de
firewall) e `sistema.rs` (`df`). O gancho é o primeiro **para avisar** e o
primeiro com `env_clear`, prazo duro e saída descartada — não o primeiro.

## 2. O que eu concluí primeiro, e estava errado

1. **«Capturo a saída e passo pelo crivo de segredo.»** O parecer permitia
   «descartados ou truncados e passados pelo crivo». Capturar exige ler dois
   pipes sem travar (duas threads novas, e o `mapa-das-threads` teria uma
   entrada a mais) e depende de o crivo conhecer o formato do segredo de um
   gateway que não conhecemos. Descartar (`Stdio::null()`) não precisa de
   thread nem de crivo: o que não se captura não vaza. Mudei antes de
   escrever.
2. **«O `ligado` se confere em `executar` e no carteiro, por garantia.»**
   Escrevi as duas; era a mesma decisão em dois lugares, que a lei da casa
   proíbe (e que nenhuma guarda isolaria: removendo uma, a outra segura e o
   teste passa). Ficou uma só, no carteiro, antes de montar texto.
3. **«A reserva de execução única pode ser um `static AtomicBool`.»** Com
   testes em paralelo no mesmo processo, um servidor descartaria o gancho do
   outro e o teste falharia raramente. Ficou por servidor, em
   `SaudeDoDisco.gancho_em_voo`.
4. **`/tmp` direto nos testes.** Usei `std::env::temp_dir()` e a catraca
   `ninguem_chama_temp_dir_fora_do_catalogo` reprovou a suíte inteira (e o
   provador de guardas disse «árvore limpa VERMELHA»). Teste novo cria
   diretório por `DirTemp`.

## 3. O que a medição disse

- Script recém-escrito e executado em seguida pode dar `ETXTBSY` (errno 26)
  quando outra thread de teste está com o arquivo aberto para escrita no
  instante do `fork`; o `iniciar` repete até 5 vezes (20 ms). Sem isso o teste
  de prazo flocaria em suíte paralela.
- O filho morto por `kill` **sem** `wait` fica em `/proc/<pid>` (zumbi) até o
  processo de teste acabar: é por isso que a prova olha `/proc`, e não só o
  retorno de `executar`.
- O stderr do servidor só se prova em **outro processo** (o teste reexecuta o
  binário de teste com `--nocapture`): o `eprintln!` do carteiro sai de uma
  thread que o libtest não captura.

## 4. A regra

Antes de escrever «o primeiro X do produto» num parecer, procure X no fonte
(`grep Command::new`); e quando o filho é externo, **o que ele imprime nunca
é capturado** — descarta-se, em vez de confiar num crivo.

## 5. Como está guardado hoje

Dez guardas `gancho-*` no `bancada/guardas/catalogo.py` (uma por decisão de
segurança) e `disco-silencio-furado` estendida aos dois testes do gancho.
**Buraco:** o `blacklist.rs` (firewall) continua com `.output()` sem prazo e
com o ambiente herdado — fora desta frente, registrado para a revisão SEC.
