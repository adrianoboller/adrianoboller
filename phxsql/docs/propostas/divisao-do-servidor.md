# Divisão do `crates/phxsql-server/src/servidor.rs`

Ordem do dono, 08/10/2026: «o `servidor.rs` não pode ser tão grande; subdivisão
em partes `Servico_XXXXXXX_01.rs`, `_02.rs`…». Os nomes saem em minúsculas
(`servidor/servico_<dominio>_NN.rs`): o Rust exige *snake_case* em módulo e o
`clippy` reprovaria o portão.

Plano do planejador (só leitura), medido no HEAD de 08/10/2026; gravado pelo
integrador.

## 1. O mapa

**77.631 linhas**: produção 1–36.465 (~36,5 mil) e testes 36.466–77.631
(~41,2 mil, em 79 `mod testes_*`). Um único `impl Servidor` de ~31,5 mil
linhas (2061–33632).

| Domínio | Linhas |
|---|---|
| Núcleo (`novo`, papel, `travar_dados*`, portas, `escutar`) | ~980 |
| Ouvinte / accept | ~260 |
| Violação, bloqueio, mensagens | ~510 |
| Relógio e amostrador | ~100 |
| Replicação (laços) | ~1.540 |
| Cluster | ~1.510 |
| Bidirecional | ~1.550 |
| Backup agendado | ~480 |
| Config, usuário, diretivas | ~950 |
| Serviço e jobs | ~930 |
| Política e direito por coluna | ~1.120 |
| Web, HTTP, REST, swagger | ~1.580 |
| Fio de dados, aperto, `atender` | ~770 |
| **Portão** (`despachar` … `portoes_do_pedido`) | ~550 |
| `op_desafio`, `op_login` | ~275 |
| **`executar`** (o `match` das operações) | ~290 |
| `abrir_travada*` | ~300 |
| Administração | ~710 |
| Visões, consultar, agrupar | ~1.520 |
| **`op_pivotar`** | ~270 |
| Bulkinsert e cargas | ~270 |
| Transação | ~2.370 |
| Marca, aplicar conjunto, sujas, sinais, reparo | ~1.700 |
| Sequências e DDL | ~1.440 |
| SQL, rotinas, gatilhos | ~1.000 |
| Leitura, catálogo, varrer | ~850 |
| Escrita, lixeira, LGPD, trilha | ~1.760 |
| Backup, restaurar, PITR | ~990 |
| Sessões e telemetria | ~510 |
| Exportar e estatísticas | ~300 |
| **`op_juntar`, `op_diferencas`, `op_unir`** | ~770 |
| DBlink | ~560 |
| Sistema e disco | ~370 |
| Painel, memória, diário, profiler, posição | ~965 |
| Replicação (operações) | ~915 |
| Quórum | ~565 |

Qualquer método pode ir para outro `impl Servidor` num módulo filho; os campos
privados continuam visíveis ao descendente. O preço: método ou função privada
movida passa a `pub(super)` — **nunca** `pub(crate)` ou `pub`.

**Tem de ficar junto:**

1. `despachar` até `portoes_do_pedido`, contíguo, e `executar`: `catalogo.rs` e
   `rest.rs` recortam esse TEXTO.
2. `op_juntar`, `op_unir`, `op_pivotar` e `op_diferencas`, com a conferência
   própria de cada uma, num arquivo só.
3. `travar_dados` e `travar_dados_para_ler`: `so_um_lugar_toma_a_trava` exige
   cada tomada dentro da função que lhe dá nome.

## 2. A divisão (alvo ≤ 3.000 linhas; nenhum arquivo passa)

**`servidor.rs` (~2.600):** `use`, os `mod`, `Sessao` e afins, `struct Servidor`,
o portão contíguo, `executar`, `campos_do_erro`/`resposta_erro`, e
`FONTES_DO_SERVIDOR` (só em teste).

| Arquivo | Conteúdo | Linhas |
|---|---|---|
| `servico_nucleo_01.rs` | `novo`, portas, `travar_dados*`, `abrir_travada*`, `TravaMedida`, `TomarTrava`, trava reentrante | ~1.600 |
| `servico_rede_01.rs` | ouvinte, fio, aperto, `Remoto`, `Janela`, desafio, login | ~1.620 |
| `servico_web_01.rs` | web, HTTP, REST, swagger | ~1.580 |
| `servico_avisos_01.rs` | violação, mensagens, `Carteiro`, sentinela 509, relógio, amostrador | ~1.120 |
| `servico_replicacao_01.rs` | laços e filas | ~1.650 |
| `servico_replicacao_02.rs` | operações da réplica, linhagem | ~1.100 |
| `servico_quorum_01.rs` | quórum | ~600 |
| `servico_cluster_01.rs` | cluster | ~1.510 |
| `servico_bidirecional_01.rs` | bidirecional e seus tipos | ~1.810 |
| `servico_backup_01.rs` | agendado, operações, PITR | ~1.570 |
| `servico_config_01.rs` | config, usuário, diretivas | ~950 |
| `servico_jobs_01.rs` | serviço e jobs | ~930 |
| `servico_permissao_01.rs` | política, direito por coluna | ~1.250 |
| `servico_admin_01.rs` | administração, sessões | ~1.215 |
| `servico_telemetria_01.rs` | telemetria, profiler, painel, memória, disco, posição | ~1.335 |
| `servico_consulta_01.rs` | visões, consultar, agrupar, filtros | ~1.800 |
| `servico_composicao_01.rs` | **pivotar, juntar, diferenças, unir** e iteradores | ~1.510 |
| `servico_escrita_01.rs` | inserir, atualizar, excluir, lixeira, LGPD, trilha, bulkinsert | ~2.120 |
| `servico_transacao_01.rs` | transação | ~2.370 |
| `servico_marca_01.rs` | marca, sujas, sinais de parar, reparo | ~1.700 |
| `servico_esquema_01.rs` | sequências, DDL | ~1.440 |
| `servico_sql_01.rs` | SQL, rotinas, gatilhos, `MotorDoServidor` | ~1.115 |
| `servico_mcp_01.rs` | `ExecutorLocal` | ~540 |
| `servico_leitura_01.rs` | catálogo, varrer, exportar, estatísticas, verificar | ~1.215 |
| `servico_dblink_01.rs` | DBlink | ~560 |

**Testes:** `servidor/testes_<nome>.rs`, declarado `#[cfg(test)] mod testes_<nome>;`,
**sem mudar nome** (o catálogo cita 614 caminhos `servidor::testes_*::`).
`testes_transacoes` (5.423) tira os 4 sub-módulos para `servidor/testes_transacoes/`.
Recusados: `include!` (o `cargo fmt` é portão e não entra em arquivo incluído) e
agrupar `testes_*` por domínio (reescreveria os 614 nomes).

## 3. Quem lê o arquivo, e como continua achando

Regra: **uma lista só** — todo leitor mede a união `servidor.rs` +
`servidor/**/*.rs`, o mesmo texto cortado em fronteira de item. A régua tem de
dar o **mesmo número** antes e depois: catraca não sobe nem desce pela divisão.

- **`include_str!("servidor.rs")`** em `telemetria.rs`, `rest.rs`, `catalogo.rs`
  e no próprio arquivo (dois módulos de teste): um
  `pub(crate) const FONTES_DO_SERVIDOR` mais o teste que a compara com o
  `read_dir` de `servidor/`. O `conferidor.rs` não inclui o `servidor.rs`.
- **`bancada/guardas/catalogo.py`:** 240 de 882 entradas apontam para
  `servidor.rs`. Repontar mecanicamente para o ÚNICO arquivo que contém o
  `trecho`; zero ou dois candidatos param o passo. Sem glob; o provador não muda.
- **`mapa-da-trava.py`:** `ALVO` vira a lista sem os `testes_*`. Corrigir ANTES:
  `#[cfg(test)] mod x;` sem chave faria o `find("{")` engolir produção como
  teste — e a catraca **desceria** sem melhora nenhuma.
- **`mapa-das-threads.py`:** 22 entradas a repontar; reconhecer arquivo de teste
  declarado em qualquer `.rs`, inclusive `cfg(all(test, debug_assertions))`.
- **`conferidor_canal.rs`:** o isento vai para
  `servidor/testes_firewall_e_mensagens.rs`.
- **Outros Python** que citam o caminho (`profiler/custo.py`, que REESCREVE o
  arquivo; `comparativo/medir.py`; e outros): separar em leitor, mutador ou só
  citação. Leitores usam um helper único, `bancada/fontes_do_servidor.py`.
- **`docs/`:** 1.061 citações `servidor.rs:NNNN` em 135 arquivos, já
  envelhecidas. Os documentos históricos ficam; os vivos passam a citar arquivo
  e símbolo, sem número de linha.

## 4. Ordem

0. **Linha de base gravada:** `wc -l`, a lista de nomes de teste
   (`cargo test -p phxsql-server --lib -- --list | sort`), todas as `--catraca`
   e o `ultima-corrida.json`.
1. **Ferramentas sem mover nada:** `FONTES_DO_SERVIDOR`, o helper Python, os
   mapas multiarquivo, o `mod x;` de teste. Números idênticos.
2. **Testes primeiro**, um módulo (ou lote pequeno) por passo. Não mexe em
   visibilidade. O arquivo cai de 77,6 mil para ~36,5 mil.
3. **Tipos do prelúdio e funções livres**, com `pub(super)`.
4. **Um domínio de `impl` por passo**, do menos arriscado ao mais: dblink, mcp,
   jobs … e por último composição, permissão e núcleo.

**Em cada passo:** só mover; `cargo build`, `clippy`, `fmt`; a suíte do pacote
com a lista de nomes igual à da base; repontar o catálogo e provar com
`provar-guardas.py --so` as guardas do trecho movido (na composição:
`pivotar-sem-portao`, `juntar-sem-portao`, `unir-sem-portao`); todas as catracas
iguais.

**Pronto quando:** o `diff` é só movimento, `pub(super)` e `mod`; a lista de
testes é idêntica; todas as guardas estão PROVADAS; todas as catracas têm o mesmo
número; nenhum arquivo passa de ~3.000 linhas.

## 5. Riscos

1. **A conferência própria parece duplicação do portão.** As quatro operações
   ficam juntas em `servico_composicao_01.rs`, com um cabeçalho que cita o
   `CLAUDE.md`, e nasce uma catraca contando `pode_em(…Ler)` naquele arquivo,
   igual antes e depois. Só as guardas `*-sem-portao` acusam a remoção.
2. **Visibilidade:** `pub(super)`, nunca mais largo.
3. **Testes que usam item privado** seguem funcionando, porque continuam filhos
   de `servidor`.
4. **Trecho de guarda com dois candidatos** (idêntico em dois arquivos ou
   cruzando a fronteira): o passo para e escolhe outra fronteira; nunca se
   afrouxa o trecho.
5. **Leitores que confundem teste e produção** — auditar no passo 1.
6. **`docs/` com número de linha** envelhece de vez.
7. **Função e comando não se duplicam:** a lista de fontes é UMA em Rust e UMA
   em Python, as duas derivadas do disco, com guarda de igualdade.
