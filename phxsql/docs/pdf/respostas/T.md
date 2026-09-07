# T) exemplo de teste de quórum

> Corrida em 2026-09-07T16:37Z UTC (e uma segunda corrida às 16:31Z, com a
> máquina ocupada por outra frente — ver nota de metodologia) · commit
> `a56a165` · `target/release/phxsqld` · reproduzido por
> `python3 bancada/quorum/medir.py 60`

## Resposta curta

**Quórum de escrita não existe no PhxSql.** O `inserir` no master confirma
**sem esperar réplica nenhuma** — não há o que testar porque não há o que
esperar. É pedido do dono, **medido antes de virar plano** (pedido 207 do
`PENDENCIAS.md`), com a rota **já decidida** («canal aberto» — a réplica
conecta e fica, o master empurra por essa conexão), mas **não construída**.

O que existe e este item testa é o **custo do caminho** que um quórum
pagaria, medido separado do sono do laço de replicação normal (que soma até
2 s e não é transporte — é intervalo de reconexão configurado). Nesta
corrida, 60 voltas, três processos em `127.0.0.1`: **gravar no master:
0,206 ms** · **levar até uma réplica, na hora: 0,470 ms** · **commit
esperando 2-de-3: 0,634 ms (3,08×)** · **commit esperando 3-de-3: 0,733 ms
(3,56×)**. Tudo em localhost — é o **piso**; a rede real custa mais.

## Exemplo exercitado

### O medidor, e por que ele separa três coisas

`bancada/quorum/medir.py` sobe um master e duas réplicas com
`reconectar_em: 3600` (uma hora) — para o laço PRÓPRIO delas não competir com
a medição — e cronometra, na hora, o `inserir` no master, depois o
`replicar`+`aplicar` chamados manualmente contra cada réplica (o que um
quórum síncrono pagaria a mais). Com 3 nós, 2-de-3 é o tempo até a
**primeira** réplica confirmar (o master já é um voto); 3-de-3 é até a
**última**. Há um portão: se `aplicar` devolver zero eventos, o medidor
**para** — zero é sintoma de a réplica ter puxado sozinha, e publicar a
média de zeros seria publicar que o quórum é de graça.

### Saída real, máquina livre (`bash bancada/esta-medindo.sh` não achou nada rodando)

```
$ python3 bancada/quorum/medir.py 60

60 voltas, tres servidores em 127.0.0.1

  gravar no master (o que se paga HOJE)     0.206 ms   faixa [0.153, 4.204]
  levar para UMA replica, na hora           0.470 ms   faixa [0.346, 3.482]

  commit hoje (sem esperar ninguem)         0.206 ms
  commit esperando 2-de-3                   0.634 ms   3.08x
  commit esperando 3-de-3                   0.733 ms   3.56x

resultado gravado: bancada/quorum/resultados.json
```

`resultados.json` completo desta corrida:

```json
{
 "voltas": 60, "gravar_ms": 0.206, "gravar_faixa": [0.153, 4.204],
 "levar_ms": 0.47, "levar_faixa": [0.346, 3.482],
 "quorum_2de3_ms": 0.428, "quorum_2de3_faixa": [0.346, 2.613],
 "quorum_3de3_ms": 0.527, "quorum_3de3_faixa": [0.371, 3.482],
 "commit_hoje_ms": 0.206, "commit_com_2de3_ms": 0.634,
 "commit_com_3de3_ms": 0.733, "vezes_2de3": 3.08, "vezes_3de3": 3.56,
 "maquina": "tres processos phxsqld em 127.0.0.1, no mesmo container",
 "aviso_da_maquina": "TUDO em localhost: a rede real custa mais, e este numero e o PISO do que um quorum custaria",
 "medido_em": "2026-09-07 16:37"
}
```

### Nota de metodologia: a mesma bancada, com a máquina ocupada

Uma corrida anterior desta rodada (16:31Z), com `ps` mostrando outras frentes
com `phxsqld`, `node`/Chromium e um `python3 bancada/...` de pé ao mesmo
tempo, publicou **medianas parecidas** (0,249 / 0,591 / 0,799 / 0,905 ms) mas
com **cauda muito mais longa** (`gravar_faixa` até **1.330,6 ms**,
`levar_faixa` até **463,7 ms**) — contra no máximo 4,2 ms na corrida limpa
acima. `bancada/quorum/medir.py` **não tem** o portão `maquina_ocupada` que
`bancada/replicacao/medir.py` já tem (ele só grava `medido_em`); a diferença
entre as duas corridas desta rodada é a prova viva de por que esse portão
importa — a mediana resiste à disputa de CPU, a cauda não. Registrado aqui,
não corrigido: mexer no medidor de quórum não foi o pedido desta frente.

### O que a lei da casa já obrigava a dizer aqui

Do `CLAUDE.md`: quórum de escrita **não existe ainda** (pedido 207), com a
rota **decidida** pelo dono em 07/09/2026 — «construir, pela rota do CANAL
ABERTO»: a réplica conecta e **fica**, e o master empurra por essa conexão,
preservando a propriedade de firewall (quem abre continua sendo a réplica).
Isto é o que o `docs/REPLICACAO.md` §19 já registra, e este item confirma que
os números por trás da decisão continuam de pé nesta rodada.

## O que NÃO existe, e é dispensa registrada

- **Não existe quórum de escrita.** Nenhuma operação do protocolo espera N
  confirmações antes de responder `ok` a um `inserir`. Não há flag, não há
  campo no `config.json`, não há caminho parcial. É o pedido 207, **aberto**.
- **Não existe o campo do quórum mínimo (M) em lugar nenhum** — nem no
  `config.json`, nem no protocolo, nem na tela (pedido 208). Quando existir,
  a decisão já tomada é que ele mora no bloco `cluster`, ao lado de `nos` —
  nunca em `replicacao.replicas_autorizadas` (que é só ACL de IP, sem
  identidade nem saúde) nem em `replicacao.origens` (que é a lista da
  RÉPLICA, não do master que esperaria o quórum).
- **Não existe canal aberto (push) do master para a réplica.** A replicação
  de hoje é **pull** — a réplica procura, o master nunca empurra — e é
  exatamente essa direção que um quórum síncrono precisa inverter. A rota
  decidida («canal aberto: a réplica conecta e fica») ainda não tem uma linha
  de código.
- **Não existe definição do que o "ok" da réplica significaria** num quórum
  futuro — recebeu, aplicou, ou aplicou e sincronizou em disco? São três
  garantias diferentes, e o `docs/CASSANDRA.md` já registra a armadilha: o
  `QUORUM` do Cassandra® não quer dizer "em N discos", quer dizer "N
  processos copiaram para um `mmap`", com `fsync` a cada 10 s. Decidir isto é
  parte do pedido 207, não decidido aqui.
- **Não existe modo "às vezes recusa" no commit.** Hoje o `inserir` sempre
  aceita (dado durabilidade só do master). Um quórum de escrita mudaria isso
  — sem N réplicas alcançáveis, o commit falharia — e trocar "sempre aceita"
  por "às vezes recusa" é decisão de produto que a resposta curta já cobra:
  quórum **custa disponibilidade**, não a compra de graça.
- **Este medidor não conta como teste de aceitação de quórum** (não há o que
  aceitar ou rejeitar ainda): ele mede **custo de caminho**, não confere
  comportamento. Um teste de aceitação de verdade (falha com o defeito
  reposto, passa com o conserto) só existe depois que o pedido 207 tiver
  código para testar.

## Como se refaz

```bash
cargo build --release
python3 bancada/quorum/medir.py 60
```

Repita `bash bancada/esta-medindo.sh` antes: se ele listar algo, a corrida
mede a disputa por CPU, não o quórum — a nota de metodologia acima é a prova
disso, com os dois números lado a lado.
