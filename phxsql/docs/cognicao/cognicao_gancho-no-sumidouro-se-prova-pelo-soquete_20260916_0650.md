# O gancho no sumidouro se prova pelo soquete — e o que se põe sob a trava é só a fila

Frente D da rodada de 16/09/2026 (pedido 249, saúde do disco). Três achados
na mesma manhã, do mesmo lugar: onde mora o gancho decide como se prova, e
decide também o que ele tem o direito de fazer.

## 1. O que aconteceu

O gancho do erro de E/S entrou em `Servidor::anotar` — o único sumidouro por
onde toda resposta de erro passa (21 chamadas, todas com `codigo`). Escrevi
a prova como as outras do arquivo: `s.despachar(...)` direto e o relé falso
esperando. O `inserir` recebeu o `Io` de verdade (código 5001, `.reg` trocado
por diretório) e **nenhum e-mail chegou** (`recv_timeout` de 10 s vencido) e
`erros_es` ficou em **0**.

Depois, com a prova verde pelo soquete, a catraca `rede-ou-espera` do
`mapa-da-trava.py` foi de 0 para **1 (teto 0)**: a seção
`descarregar_sujas_com -> fecho_recusado -> evento_de_disco ->
avisar_saude_do_disco -> enviar`.

E, no meio, a `std` 1.94.1: `io::Error::from_raw_os_error(5)` (EIO) devolve
`ErrorKind::Uncategorized`, enquanto 30 (EROFS) devolve `ReadOnlyFilesystem`
e 28 (ENOSPC) `StorageFull`.

## 2. O que eu concluí primeiro, e estava errado

- «O `despachar` é o caminho do pedido; provar por ele prova o gancho.» Não:
  `despachar` **devolve** o resultado, e quem chama `anotar` é o `atender`
  (a porta de dados), o `atender_http` e os jobs. Um teste por `despachar`
  prova a recusa e **pula o sumidouro**. Os testes vizinhos passavam por
  `despachar` porque provavam outra coisa (política, bloqueio) — copiei a
  forma sem conferir se ela alcançava o meu gancho.
- «O envio já está numa thread própria (`telemetria.subir`), então não há
  rede sob a trava.» Em execução, verdade; para a catraca, não: o mapa é
  **estático** e vê o `enviar` dentro do fechamento passado ao `subir`. E o
  orquestrador estava certo em não aceitar o argumento: uma regra que só se
  defende «olhando com cuidado» é uma regra que a próxima pessoa quebra sem
  ver. O desenho certo é o que o mapa consegue provar sozinho.
- «`ErrorKind` basta para classificar o erro do disco.» Só para EROFS e
  ENOSPC. EIO — o erro que mais interessa numa sonda de disco — só se nomeia
  pelo errno.

## 3. O que a medição disse

- Pelo `despachar`: 0 e-mails, `erros_es = 0`, 10 s de espera vencida. Pelo
  soquete (`aceitar_ate_mandarem_parar` + `TcpStream`): **1** e-mail, **0** no
  segundo erro da janela, `erros_es = 2`, corrida do teste em **0,75 s**.
- `mapa-da-trava.py --catraca`: `rede-ou-espera` **1 (teto 0)** com o `subir`
  dentro da seção crítica; **0** com a fila (`Mutex<VecDeque>` +
  `Condvar::notify_one`) e o envio na thread `sonda-disco`. Os padrões que o
  mapa conta como rede são `TcpStream::connect`, `TcpListener::bind` e
  `set_read_timeout`; como espera, `sleep` — um `notify_one` não é nenhum
  dos dois, e o `wait_timeout` só existe na thread do carteiro.
- `rustc 1.94.1`: errno 30 → `ReadOnlyFilesystem`, 28 → `StorageFull`, 5 →
  `Uncategorized`, 21 → `IsADirectory`, 20 → `NotADirectory`.
- Playwright: `waitForFunction` com predicado `async` «passou» em 0 ms — uma
  `Promise` é valor verdadeiro. O caso inteiro levou 643 ms com uma espera
  de 2 s dentro; com o laço explícito, 4.002 ms e 3.920 ms nos dois temas.

## 4. A regra

**Gancho que mora no sumidouro se prova pelo caminho que chega ao sumidouro
— e o que ele faz sob a trava é entregar a uma fila, nunca alcançar a rede.**
Classifique erro do sistema pelo errno **e** pelo `kind`, porque a `std` só
categoriza metade.

## 5. Como está guardado hoje

- `servidor::testes_da_saude_do_disco::*` sobem a porta de dados de produção
  (`porta_de_dados_de_verdade`) e falam por `TcpStream`; o comentário do
  ajudante diz por quê.
- `SaudeDoDisco::entregar`/`esperar` (`saude_do_disco.rs`) e o comentário de
  `evento_de_disco` nomeiam a catraca que motivou o desenho; o
  `mapa-da-trava.py --catraca` é quem o vigia, com teto zero.
- `saude_do_disco::classificar` olha os dois lados, com a tabela medida no
  cabeçalho do módulo e em `docs/SAUDE-DO-DISCO.md` §2.1.
- O `esperarEstado` de `testes-web/casos/29-saude-do-disco.mjs` diz no
  comentário por que é um laço e não um `waitForFunction`.
- **Buraco que ficou:** o sítio do `fecho_recusado` dentro de
  `descarregar_sujas_com` não tem prova com defeito reposto — não há como
  fazer um `fsync` falhar sem injeção de falha. O método é provado; a
  chamada, não. Dito em `docs/SAUDE-DO-DISCO.md` §6.
