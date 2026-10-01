# O `RwLock` prefere o escritor: trocar a ficha do backup pela de leitura não soltava a leitura

## O que aconteceu

Pedido 513, passo 1 (H2 da decisão de 01/10/2026): o backup segurava a ficha
EXCLUSIVA da trava de dados a cópia inteira, e a H2 mandava trocá-la pela de
LEITURA — «o leitor anda, o escritor espera». A troca de uma linha
(`travar_dados()` → `travar_dados_para_ler()` no `op_backup`) compila e passa
em qualquer prova que leia ANTES de gravar.

A prova pelo soquete (`servidor::testes_do_retrato_do_backup`) põe o escritor
na fila ANTES da leitura, com a cópia parada 1,5 s. Com a troca de uma linha só,
a leitura esperou **1.373 ms** — praticamente o mesmo que com a ficha exclusiva
(**1.358 ms**). O `std::sync::RwLock` do Linux (futex) recusa leitor novo quando
há escritor esperando (`is_read_lockable`), e num servidor de verdade alguém
grava no primeiro minuto de um backup de uma hora.

## O que eu concluí primeiro, e estava errado

Que a H2 era só trocar a trava, e que «provar que nada grava sob a trava de
leitura» era a única dúvida (a do papel C). A premissa escondida era «leitor
não espera leitor» valer também com um escritor na fila — e não vale: a
preferência pelo escritor existe justamente para ele não passar fome (o teste
`o_escritor_nao_passa_fome_entre_leitores` prova o outro lado da mesma moeda).

E uma segunda, menor, no mesmo dia: armei a pausa de teste com o NOME da
operação (`"backup"`), e o `despachar` arma pelo nome da op ANTES de qualquer
trava — a pausa parava só a conexão, a escrita passava em 1 ms e a prova caía
pelo motivo errado. O gancho da cópia ganhou nome próprio (`backup_copia`).

## O que a medição disse

| como a cópia segura a trava | leitura com escritor na fila | escrita |
|---|---|---|
| ficha exclusiva (antes) | 1.358 ms | 1.507 ms |
| ficha de leitura, sem portão (H2 ingênua) | 1.373 ms | 1.521 ms |
| ficha de leitura + portão do retrato | **1–6 ms** | 1.506–1.514 ms |

Cópia parada 1,5 s, `cargo test` em depuração, máquina com carga ~5.

## A regra

Trocou um `Mutex`/ficha exclusiva por ficha de leitura para «soltar os
leitores»? Prove com um ESCRITOR já na fila — sem isso a prova mede o caso que
não acontece em produção.

## Como está guardado hoje

O escritor espera no portão do retrato (`crates/phxsql-server/src/retrato.rs`),
ANTES da fila do `RwLock`, e por isso não faz o leitor esperar. Duas guardas no
catálogo: `backup-copia-sob-a-exclusiva` e `backup-sem-portao-do-retrato`.

O buraco que fica: a tabela gravada na última janela `por_lote` antes do backup
tem o cabeçalho do `.log` atrasado, e a ficha compartilhada recua para a
exclusiva para curá-lo — essa primeira leitura ainda espera a cópia.
