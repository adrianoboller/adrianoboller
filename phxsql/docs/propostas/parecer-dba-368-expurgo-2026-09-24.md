# Parecer do DBA (papel C): pedido 368, expurgo da trilha `.lgpd` — 24/09/2026

**Veredito: integra, com duas condições baratas (C1 e C2), e com o formato B na mesma versão.** Nenhuma via apaga volume indevido: nem o ativo, nem o de fronteira, nem volume trocado. A reabertura no meio das três fases é segura.

## A) Revisão

**O que se sustenta:**

- O ativo é barrado duas vezes: no plano e no `unlink`.
- Sai sempre um prefixo, e o `unlink` corre em ordem crescente.
- «Mais novo» sai de uma varredura com CRC conferido.
- O bilhete confere UUID + fim do volume.
- A fase 3 só aceita um `ExpurgoSelado`.
- 29/02 recua para 28/02, o que apaga menos.
- Queda antes do selo não apaga nada.

| # | Sev. | Onde | Cenário |
|---|---|---|---|
| A1 | ALTO (já existia antes do 368) | `trilha.rs:714-720`, `table.rs:4279→4283`, `servidor.rs:11415` | O teto de volumes da trilha é o `max_arquivos` do `.reg`, e o expurgo não devolve vaga. No teto, `atualizar` grava a linha e o `.log` e só então falha na trilha: fica linha sem trilha, e a cascata do `ao_alterar` é pulada, deixando filha órfã. Provado: com `max_arquivos 3`, a 71ª alteração falhou e a linha ficou gravada; com `max_arquivos 2`, a mãe ficou em `id=66` e a filha aponta para `65`. |
| A2 | MÉDIO → C2 | `servidor.rs:165` (`OPS_ESCRITA`) | A trilha é do nó, mas o expurgo do admin é redirecionado ou recusado na réplica. O irmão `esvaziar_lixeira` tem o mesmo furo com o `.trash`. |
| A3 | MÉDIO → C1 | `FORMATO.md` §6, `motivo.rs` | A tag 4 tem dois donos, separados por prefixo de texto. É decidir pela frase. |
| A4 | BAIXO | `servidor.rs:20313` (`op_trilha`) | A paginação por `pular` faz a exportação do auditor pular k registros sem aviso se houver expurgo entre duas páginas. |
| A5 | BAIXO | preparar/concluir, `trilha.rs:1321` | Pode sobrar rastro sem apagamento (queda entre as fases, recusa na fase 3, erro de `unlink` no meio), e o `Err` perde a lista parcial. |
| A6 | BAIXO | `servidor.rs:20659` | O intervalo diário usa relógio de parede. |

**C1:** nada de tipo 5. O expurgo da trilha vira o bit 0 do byte 9 (`flags`) do registro do `.reason`, com tipo 4. O byte está sempre em 0 hoje, é coberto pelo CRC e pelo AAD, e o leitor atual não o lê. Provado numa cópia: o leitor atual leu «expurgo», e os testes em claro e cifrado passaram.

**C2:** `expurgar_trilha` sai do `OPS_ESCRITA`, porque é manutenção de um arquivo local do nó.

## B) O formato que fecha o buraco da tabela padrão

Premissas medidas:

- Tabela sem paginação: a trilha é um arquivo só, que nunca fecha.
- `existentes()`: 625 µs e 999 statx por chamada.
- Abrir tabela paginada: 6,7 ms e 10.001 statx, contra 34 µs e 16 statx sem paginação.

Hipóteses:

- H1, numerar sempre e cortar só por tamanho: **morreu**. A tabela de pouco tráfego nunca fecha volume.
- H2, cortar por tempo reescrevendo: **morreu**. Reescreveria arquivo append-only e, cifrado, re-selaria cada registro.
- H3 **vence**: o ativo tem nome fixo, e os fechados são numerados.

| comportamento | PG 4 | MariaDB 3 | MySQL 2 | resultado |
|---|---|---|---|---|
| rotaciona por tamanho | `log_rotation_size` | `server_audit_file_rotate_size` | `audit_log_rotate_on_size` | convergem |
| rotaciona por tempo | `log_rotation_age` | não | `audit_log_rotate_on_time` | 6 × 3 → entra |
| rotação pedida | `pg_rotate_logfile()` | `server_audit_file_rotate_now` | `audit_log_rotate()` | convergem |
| ativo de nome fixo | não | sim | sim | 5 × 4 → nome fixo |
| poda do arquivo inteiro, nunca do ativo | fora do motor | por contagem | por idade/tamanho | convergem |

Onde divergimos, e por quê:

- O número nunca muda: o rastro e o bilhete citam o número do volume.
- A poda é por idade, porque é a decisão do dono.

Decisão:

- O ativo é sempre `<tabela>.lgpd`, e os fechados são `<tabela>_NNN.lgpd`, com NNN igual ao `volume` do cabeçalho: no mínimo 3 dígitos, sem teto, nunca reusado.
- A trilha fica independente da paginação do `.reg`.
- O volume fecha por:
  - tamanho `lgpd.volume_mib` (padrão 64);
  - idade do primeiro registro acima de `lgpd.volume_dias` (padrão 30), verificada na passada e na op do admin;
  - pedido do admin.
- O ativo nasce com N = maior fechado + 1, por uma listagem uma vez na vida do ativo.
- Custo:
  - abrir tabela: 1 statx para a trilha;
  - append: +0;
  - cerca de 61 arquivos por tabela em 5 anos;
  - retenção máxima: prazo + 31 dias.
- Migração sem reescrita:
  - a trilha de arquivo único vira o ativo, com volume 1;
  - a trilha paginada `_001` a `_K` vira fechados, e o ativo nasce `K+1`.
- Nenhum byte de registro, de cabeçalho ou do PSCH muda. O `.log`, o `.trash` e o `.reason` não mudam, fora o bit do C1.
- O downgrade de tabela paginada não é suportado depois do B. Por isso o B entra na mesma versão que o 368.

**Sobe ao dono: nada.**

## Pendências

- A1 (metade de I/O): disco cheio. A cascata não pode depender do observador.
- A4: paginação da `op_trilha` por cursor.
- Achado lateral: abrir tabela paginada custa 6,7 ms contra 34 µs.
