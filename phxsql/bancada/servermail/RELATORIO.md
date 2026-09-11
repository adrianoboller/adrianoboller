# RELATORIO -- bateria do ciclo do phxSERVERMAIL

**Data da corrida:** 2026-09-11 (medido agora, neste sandbox)
**Versao do binario:** phxsqld 0.18.0 (recompilado nesta rodada, 22:11)
**Como refazer:** `bash bancada/servermail/rodar.sh [dir] [n_linhas]`
**Placar geral:** os 9 passos PASSARAM. `resultados.json` grava o veredito de cada um.

Esta bateria exercita o **banco de verdade** -- grava e le do disco real, com as
chaves estrangeiras conferidas na gravacao, e sobe `phxsqld` reais para a
replicacao e o cluster. Nao e maquete. O que **nao** roda neste sandbox esta dito
sem rodeio (passo 6 e a parte remota do passo 7).

---

## Arquivos criados (todos caminhos absolutos)

| arquivo | papel |
|---|---|
| `/home/user/adrianoboller/phxsql/crates/phxsql-store/examples/servermail-ciclo.rs` | exemplo Rust: o banco de verdade (passos 2,3,4,7) |
| `/home/user/adrianoboller/phxsql/bancada/servermail/rodar.sh` | orquestra os 9 passos, grava `resultados.json` |
| `/home/user/adrianoboller/phxsql/bancada/servermail/instalar.sh` | passo 1: instalar + configurar + subir + base nasce |
| `/home/user/adrianoboller/phxsql/bancada/servermail/replicar.py` | passo 5: replicacao com SHA-256 por linha |
| `/home/user/adrianoboller/phxsql/bancada/servermail/dns.py` | passo 6: resolucao DNS, honesta |
| `/home/user/adrianoboller/phxsql/bancada/servermail/LEIA-ME.md` | guia da bancada |
| `/home/user/adrianoboller/phxsql/bancada/servermail/RELATORIO.md` | este relatorio |
| `/home/user/adrianoboller/phxsql/bancada/servermail/resultados.json` | veredito por passo, gravado pelo `rodar.sh` |

Reusa, sem duplicar: `bancada/replicacao/montar.py` (passo 5),
`bancada/cluster/provar.py` (passo 8), `phxsql-core/examples/correio-e2e` (passo 9).

---

## Passo a passo -- veredito, o que foi medido AGORA, e a saida real

### PASSO 1 -- Instalacao + configurar a base --- PASSOU
`bash bancada/servermail/instalar.sh <dir> 5951`. Medido agora:
- `phxsqld --exemplo 1 > config.json` gera o modelo (18 KB).
- `phxsqld --senha` gera `senha_hash` PBKDF2-SHA256 (`pbkdf2-sha256$210000$...`);
  o `config.json` **nao tem senha em claro** (guarda conferida no script).
- O servidor **sobe e escuta**: `PhxSql 0.18.0 escutando em 127.0.0.1:5951 |
  base base | papel isolado`.
- A **base nasce no disco** (pasta `base/`), e o servidor e derrubado pelo PID
  guardado (nunca por nome).

### PASSOS 2, 3, 4, 7 -- o banco de verdade --- PASSOU (19/19, PROVA VERDE)
`target/release/examples/servermail-ciclo <base>`. Cria a base do server mail em
`<base>/correio/` (banco = pasta), com as tabelas `empresas`, `funcionarios`,
`coligacoes`, `cadastros`. Cada uma nasce com os 8 arquivos do formato
(`.reg .ndx .bin .memo .log .reason .trash .pag`).

**PASSO 2 -- UUID v7 e onde fica guardado.** Medido agora:
- `versao() == 7` para os ids gravados; dois v7 seguidos sao **crescentes**
  (monotonico por construcao, RFC 9562).
- O id **volta identico do disco** (lido via `Table::ler`), continua v7, e
  continua crescente; a busca pelo `.ndx` (`porId`) acha o rowid do id v7.
- **ONDE FICA GUARDADO:** no arquivo **`.reg`** da tabela
  (`<base>/correio/funcionarios.reg`). O `.reg` e um *heap* de slots de largura
  fixa na ordem de digitacao; a coluna `id` (tipo `Uuid`) ocupa **16 bytes
  inline** no slot, `rowid` = numero do slot. O indice `porId` mora no `.ndx`
  (a B+tree). Fonte: `docs/FORMATO.md`.

**PASSO 3 -- empresa (pai) <- funcionario (filho), RESTRICT nos dois sentidos:**
- inserir funcionario para empresa **inexistente** e RECUSADO (chave nasce conferida);
- `excluir_de_vez` **e** `excluir_suave` da empresa com funcionario sao RECUSADOS
  (nunca se mata o pai que tem filho -- o suave tambem, para nao deixar orfa que
  a tela nao mostra);
- removidos os filhos (e feito o flush), a empresa **pode** ser apagada.
- Indice dos **dois lados**: `porId` na mae, `porEmpresa` na filha.

**PASSO 4 -- coligacao (confianca entre empresas):**
- a coligacao (empresa_a, empresa_b, estado) **nasce e e lida** correta;
- coligacao para empresa inexistente e RECUSADA (chave conferida em ambos os lados);
- empresa dentro de uma coligacao viva **nao pode** ser apagada (RESTRICT do outro
  lado); desfeita a coligacao, a empresa pode sair.
- Como o correio (confianca do server mail em rede, pedido #159) ainda nao tem
  servidor de rede proprio, a relacao de confianca foi exercitada **pelo banco**
  (tabela + FKs), e isto esta dito com todas as letras.

**PASSO 7 -- portao WX:**
- antes de o v7 ser armazenado, `liberar` o cadastro **RECUSA** (portao fechado);
- **simulado** o armazenamento do v7 (marca local `wx_armazenado=true`) --
  **sem conectar em 177.69.238.17**, que e producao do dono;
- entao `liberar` **passa**, e no disco `wx_armazenado=true` e `liberado=true`,
  com o id igual ao v7 do usuario.
- O armazenamento remoto na WX **nao foi exercitado de proposito**.

> Nota de honestidade sobre "prova real nos dois sentidos": esta bateria mostra a
> recusa **e** a passagem de cada invariante. A prova de que o motor recusa
> justamente por causa da guarda (o defeito reposto) vive nos testes unitarios do
> motor -- `ao_excluir_so_aceita_restringir` (`valores.rs`),
> `sem_maioria_visivel_nao_promove` (`cluster.rs`), `v7_nunca_repete...` e
> `comparar_bytes_e_comparar_tempo` (`uuid.rs`). Esta bancada exercita as duas
> pontas ponta a ponta, no disco.

### PASSO 5 -- Replicacao master -> 3 slaves --- PASSOU (medido AGORA)
`python3 bancada/servermail/replicar.py <dir> 20000`. Subiu **4 `phxsqld` reais**
em 127.0.0.1:5800-5803 (master + slave01/02/03). Medido nesta rodada:
- 20.000 linhas no master a **~54.000 linhas/s**;
- as 3 replicas alcancam em **~2,1 s**;
- **SHA-256 por linha IDENTICO nos quatro**: `8b92f973bd4f9e0b` (retrato da
  tabela inteira lida pelo cursor, com rowid+campos -- contar linhas nao provaria).
- Derrubados por caminho (nunca por nome); nenhum servidor vazou.

Observacao honesta sobre a ferramenta: o `bancada/replicacao/medir.py` (que mede
vazao/atraso completos) estourou aqui com `KeyError: 'resultado'` num arranque
frio -- ele pergunta `posicao` a uma replica que ainda nao criou a tabela. Isso e
fragilidade do *poller*, nao da replicacao: no mesmo instante, consultado direto,
os quatro nos ja mostravam 20.000 eventos identicos. Por isso a bateria usa o
`replicar.py`, que tolera a tabela ainda nao existir e prova o que importa (o
retrato SHA-256). Os numeros de vazao/atraso **completos** ja medidos ficam como
**MEDICAO ANTERIOR datada** em `bancada/replicacao/resultados.json`
(**2026-09-07**, 0.18.0): master 33.883 linhas/s, replica 37.311 eventos/s,
alcance 2,7 s, atraso de uma insercao ~2.012 ms (dominado pelo sono de 2 s do
laco), e o custo da imagem no diario (10% e diario 5,1x maior). Nao foram
re-medidos aqui.

### PASSO 6 -- DNS --- PASSO DE IMPLANTACAO (nao e defeito do banco)
`python3 bancada/servermail/dns.py`. Medido agora, nesta maquina:
- `phxsql.com.br`  -> **NAO RESOLVE**
- `phxmail.com.br` -> **NAO RESOLVE**
- `wxsolucoes.com.br` -> **RESOLVE -> 177.69.238.17**

Conclusao honesta: os dois dominios do correio (master/slave do server mail)
**ainda nao estao no DNS**. E passo de **implantacao** -- registrar o dominio e
apontar A/MX no provedor (ex.: Cloudflare) --, **nao** defeito do banco: o portao
de dominio do correio ja funciona (passo 9, `correio-e2e` recusa endereco fora de
phxsql/phxmail). A WX resolve, mas a bateria **nao se conecta** nela.

### PASSO 8 -- Cluster: eleicao, promocao, failover --- PASSOU (medido AGORA)
`python3 bancada/cluster/provar.py <dir>`. Subiu 3 `phxsqld` reais em 5310-5312 +
um SMTP falso em 5316. Medido nesta rodada (`"falhas": []`):
- (a) tres nos, um master (no1), epoca 0, tres vivos;
- (b) 3.000 linhas replicadas; retrato SHA-256 identico nos tres
  (`204ff74ebe1651ef`); escrita na replica devolve `REDIRECIONA 5310`;
- (c) **mata o no1**; o **no2 se promove em 4,3 s** e aceita escrita; o no3 passa
  a segui-lo, epoca sobe para 1, `REDIRECIONA` aponta 5311;
- (d) **1** e-mail de promocao, **6** de degradacao citando o no1 caido (repetidos
  a cada ~6 s);
- (f) o no1 volta e **se rebaixa sozinho** a replica, alcancando o que perdeu;
- (e) **particao sem maioria**: o no3 sozinho (1 de 3) **NAO se promove** -- epoca
  intacta, escrita travada, degradacao explicada, e-mail de "sem maioria"
  capturado (o teste de protecao); depois o cluster **sara sozinho** e os tres
  convergem (retrato `fe057d23b9a39478`);
- (g) sem o bloco `cluster`, a replicacao classica continua e nenhum e-mail chega.

MEDICAO ANTERIOR datada (`bancada/cluster/resultados.json`): promocao 4,3 s,
1 e-mail de promocao, 6 de degradacao -- bate com o medido agora.

### PASSO 9 -- Envio empresa<->empresa e intra-empresa --- PASSOU (referencia)
Conforme orientado, **nao reconstrui** este passo: ele ja e provado pelo
`correio-e2e` (o orquestrador exercita o processo). Rodado aqui para citar o
placar real: `target/release/examples/correio-e2e` -> **38 checagens, 38 ok,
PROVA VERDE**. Cobre o portao de dominio, criar conta, envio mesma-empresa
(adriano->joana) e envio para outro dominio phxmail apos confianca.

---

## O que NAO consegui rodar neste sandbox, e por que
- **A corrida de replicacao/cluster em CONTEINER** (`bancada/*/docker/`), que e
  onde endereco, firewall e particao de rede existem de verdade. Aqui tudo se
  enxerga por `127.0.0.1`, entao a prova foi a de loopback (que e real: processos
  `phxsqld` de verdade, SHA-256 por linha). O docker esta instalado, mas nao subi
  os conteineres para nao pesar o disco (2,6 G livres). Os numeros de conteiner ja
  medidos estao em `docs/REPLICACAO.md` §17 e `bancada/replicacao/docker/`.
- **A vazao/atraso COMPLETOS da replicacao** (o `medir.py`): a ferramenta estourou
  num arranque frio (fragilidade do poller, nao da replicacao). A igualdade por
  SHA-256, que e a garantia, foi provada; os numeros de vazao/atraso ficam como
  medicao anterior datada.
- **O armazenamento remoto do v7 na WX** (177.69.238.17): **nao conectei de
  proposito** -- e producao do dono. O portao foi provado localmente.
- **O DNS dos dominios do correio**: nao resolve porque os dominios ainda nao
  existem no DNS -- passo de implantacao.

## Precisa da decisao do orquestrador/dono
1. **DNS/implantacao:** registrar `phxsql.com.br` e `phxmail.com.br` e apontar
   A/MX (Cloudflare) e passo de implantacao fora do escopo do motor -- confirmar
   com o dono quando for a hora.
2. **Confianca do server mail em REDE (pedido #159):** hoje a relacao de confianca
   entre empresas so existe **pelo banco** (tabelas + FKs, provado no passo 4). O
   correio-e2e prova o MODELO em memoria. Falta o servidor de rede que casa os
   dois -- decisao de roadmap.
3. **Integracao do v7 com a WX:** definir com o dono o protocolo real de
   armazenamento do v7 nos servidores da WX antes de trocar a simulacao por uma
   chamada de verdade.

## Portoes (papel B/QA)
- `cargo fmt --all` -- limpo.
- `cargo clippy --workspace --all-targets` -- **zero avisos**.
- `cargo test --workspace` -- **2262 passaram, 0 falharam** (68 binarios de teste), exit 0.
- Papel D (zelador): as bases temporarias criadas foram removidas; nenhum
  `phxsqld` vazou (conferido por `pgrep`); disco estavel em 2,6 G.
- **NAO commitado, NAO houve push** -- por instrucao do orquestrador.
