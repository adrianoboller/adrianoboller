# Fronteiras do `servidor.rs`

Primeiro passo da **SP000005** — a decomposição do `servidor.rs`. Este
documento **não move uma linha de código**: ele mede o que há lá dentro, onde
as costuras já existem, e o que hoje impede a divisão.

Mover 22 mil linhas enquanto nove frentes escrevem na mesma árvore destruiria o
trabalho delas. E há um motivo mais fundo para medir antes: a conta que decide
o custo da sprint mudou de **ordem de grandeza** quando foi medida em vez de
suposta. A seção 3 mostra por quê.

> **Nenhum número desta página se digita.** Todos saem de
> `docs/fronteiras/mapa-do-servidor.py`, entre marcas `<!-- mapa:…:inicio -->`.
>
> ```bash
> python3 docs/fronteiras/mapa-do-servidor.py                    # o relatório
> python3 docs/fronteiras/mapa-do-servidor.py --escrever         # reescreve esta página
> python3 docs/fronteiras/mapa-do-servidor.py --json             # para outro gerador
> ```
>
> O script aceita outro caminho como argumento, então renomear o alvo não exige
> editá-lo. **A prosa fica fora das marcas** — o que se escrever dentro delas
> morre no próximo `--escrever`.

<!-- mapa:carimbo:inicio -->
**Medido em** 2026-09-03 — `servidor.rs` com **23.171** linhas, sha256 `279a91244c8a7532` (com mudanca ainda nao commitada).

> A árvore é compartilhada: este arquivo cresceu **23.171 − 22.560 = 611** linhas desde o número do roteiro da SP000005.
> Se o `wc -l` de hoje não for 23.171, **esta página envelheceu** — rode o gerador de novo.
<!-- mapa:carimbo:fim -->

---

## 1. O mapa medido

<!-- mapa:mapa:inicio -->
| medida | valor |
|---|---|
| linhas totais | **23.171** |
| linhas de código | **15.840** |
| linhas de teste | **7.331**, em **18** módulos `#[cfg(test)]` |
| funções de produção | **324** |
| blocos `impl` | **15** |
| campos do `struct Servidor` | **44** |
| fatia do crate `phxsql-server` | **41%** de 55.912 linhas |
<!-- mapa:mapa:fim -->

### Os blocos `impl`, e o que a tabela mostra sozinha

<!-- mapa:impls:inicio -->
| bloco | ini | fim | linhas | fns |
|---|---|---|---|---|
| `impl Sessao` | 278 | 291 | 14 | 2 |
| `impl Remoto` | 300 | 340 | 41 | 2 |
| `impl Janela` | 361 | 413 | 53 | 4 |
| `impl Servidor` | 686 | 14941 | 14.256 | 273 |
| `impl ExecutorLocal` | 14973 | 15014 | 42 | 4 |
| `impl crate::mcp::Executor for ExecutorLocal` | 15016 | 15039 | 24 | 1 |
| `impl phxsql_sql::rotina::Motor for MotorDoServidor<'_>` | 15431 | 15457 | 27 | 2 |
| `impl std::ops::Deref for TravaMedida<'_>` | 15539 | 15544 | 6 | 1 |
| `impl std::ops::DerefMut for TravaMedida<'_>` | 15546 | 15550 | 5 | 1 |
| `impl Drop for TravaMedida<'_>` | 15552 | 15569 | 18 | 1 |
| `impl<F: FnMut()> Drop for AoSair<F>` | 15573 | 15577 | 5 | 1 |
| `impl Servidor` | 15618 | 15649 | 32 | 2 |
| `impl crate::pivot::Iterador for LinhasEmMemoria` | 15751 | 15755 | 5 | 1 |
| `impl Contagem` | 15787 | 15822 | 36 | 2 |
| `impl crate::pivot::Iterador for LinhasDaTabela<'_>` | 15830 | 15839 | 10 | 1 |
<!-- mapa:impls:fim -->

**Um bloco de mais de catorze mil linhas, e catorze de 5 a 53.** O problema do
arquivo não é o tamanho: é que 90% dele é **um único `impl`**.

### O que as funções tocam

Agrupado pelo que a região **toca**, não pelo nome que tem. Uma função conta em
cada domínio que alcança, então as colunas somam mais que o total.

<!-- mapa:dominios:inicio -->
| domínio | funções | linhas |
|---|---|---|
| config | 83 | 5.431 |
| trava-de-dados | 83 | 5.243 |
| observacao | 40 | 2.964 |
| permissao | 40 | 2.820 |
| replicacao-e-cluster | 30 | 2.283 |
| catalogo-e-disco | 30 | 2.139 |
| transacao-e-travas | 32 | 1.628 |
| rede | 20 | 1.557 |
| sql | 18 | 1.394 |
| interface-web | 13 | 1.291 |
<!-- mapa:dominios:fim -->

### Quantos domínios cada função atravessa

Esta é a tabela que diz onde as costuras **já existem**.

<!-- mapa:atravessa:inicio -->
| domínios | funções | linhas |
|---|---|---|
| 0 | 94 | 1.348 |
| 1 | 129 | 4.518 |
| 2 | 68 | 3.153 |
| 3 | 18 | 1.762 |
| 4 | 11 | 1.539 |
| 5 | 2 | 296 |
| 7 | 1 | 280 |
| 9 | 1 | 116 |
<!-- mapa:atravessa:fim -->

<!-- mapa:atravessa-resumo:inicio -->
**223 das 324 funções (69%) tocam zero ou um domínio** — 5.866 linhas que já estão prontas para sair. As que atravessam quatro ou mais são **15**, e somam 2.231 linhas.
<!-- mapa:atravessa-resumo:fim -->

As duas piores são `novo` (9 domínios) e `op_painel` (7), e as duas são
**legítimas**: uma monta o servidor inteiro, a outra desenha o painel que
mostra o servidor inteiro. Não são defeitos a consertar; são as duas funções
que **nunca** vão para um filho sozinhas.

---

## 2. As costuras naturais

Regiões contíguas cobrem os métodos dos dois `impl Servidor`. O gerador
confere a cobertura: método que caia num vão aparece como `FORA`.

- **sai** — métodos de fora que a região chama.
- **entra** — métodos da região que o resto chama.
- **campos** — campos do `Servidor` que ela toca.
- **imports** — quantos dos itens dos `use` do topo ela precisa.

<!-- mapa:regioes:inicio -->
| região | linhas | fns | sai | entra | campos | imports | trava |
|---|---|---|---|---|---|---|---|
| arranque-e-identidade | 218 | 11 | 0 | 8 | 8 | 20 | sim |
| porta-e-aceitacao | 156 | 3 | 12 | 0 | 7 | 9 | não |
| firewall-e-mensagens | 257 | 12 | 1 | 10 | 4 | 15 | sim |
| relogios-de-fundo | 63 | 2 | 0 | 2 | 2 | 4 | não |
| replicacao | 328 | 9 | 4 | 3 | 3 | 6 | sim |
| cluster | 555 | 12 | 3 | 3 | 2 | 6 | sim |
| bidirecional | 330 | 6 | 3 | 1 | 3 | 5 | sim |
| backup-agendado | 131 | 3 | 2 | 1 | 2 | 4 | sim |
| config-e-servico | 243 | 6 | 3 | 5 | 11 | 10 | não |
| jobs | 580 | 19 | 3 | 8 | 6 | 7 | não |
| web-http-rest | 968 | 14 | 9 | 3 | 8 | 14 | sim |
| porta-de-dados-e-aperto | 404 | 6 | 9 | 0 | 6 | 14 | não |
| portoes-e-despacho | 671 | 7 | 129 | 4 | 7 | 8 | não |
| operacoes-de-dados | 7.471 | 163 | 11 | 111 | 27 | 43 | sim |
| erros-de-resposta | 19 | 2 | 1 | 2 | 0 | 2 | não |
<!-- mapa:regioes:fim -->

<!-- mapa:regioes-cobertura:inicio -->
As 15 regiões cobrem **275 de 275** métodos (nenhum método num vão).
<!-- mapa:regioes-cobertura:fim -->

### O que a tabela diz

**Uma região que só chama outras é fronteira.** `portoes-e-despacho` chama mais
de cem métodos distintos e é chamada por três. Isso não é acoplamento: é a
definição de um **roteador**. O `executar` é um `match` de ~140 braços que não
faz mais nada. É a costura mais limpa do arquivo, e a única região cujo número
alto é uma **qualidade**.

**Uma região que atravessa três domínios não é fronteira.**
`operacoes-de-dados` — mais de sete mil linhas, e a maioria dos campos do
`Servidor` — é chamada por mais de cem métodos de fora. Ela não é uma região:
é o **resto**, e só se divide depois, por assunto (esquema, escrita, leitura,
transação, LGPD), nunca de uma vez.

---

## 3. O acoplamento que impede a divisão

Esta seção é o produto real desta rodada, e ela **inverteu a conclusão** com
que este trabalho começou.

### 3.1 O que eu concluí primeiro, e estava errado

A tabela da seção 2 parecia dizer que o custo da sprint eram as travessias:
`portoes-e-despacho` teria de expor mais de cem métodos, `operacoes-de-dados`
teria de receber mais de cem chamadas de fora. Com `travar_dados` (privado),
`TravaMedida` (tipo privado com campo privado) e `Sessao` (tipo privado) no
caminho, a conta era de dezenas de itens virando `pub(crate)` — e cada
`pub(crate)` num motor de dados é uma garantia que passa a depender de
disciplina em vez de compilador.

**Isso vale para uma divisão em módulos IRMÃOS — e irmão é o layout que o crate
usa hoje** (`cluster.rs`, `transacao.rs`, `travas.rs` são irmãos de
`servidor.rs`). Medido, com `rustc`:

```rust
// irmão: `pub mod servidor;` e `pub mod cluster;` lado a lado
impl Servidor { pub fn do_irmao(&self) -> u32 { self.dados + self.travar() } }
```
```
error[E0616]: field `dados` of struct `Servidor` is private
error[E0624]: method `travar` is private
```

**Mas a divisão não precisa ser em irmãos.** Em Rust, um item privado é visível
no módulo que o declara **e em todos os descendentes dele**. Um módulo
**filho** lê campo privado, chama método privado e nomeia tipo privado do pai
sem nenhum `pub`:

```rust
// servidor.rs (inalterado, só ganha uma linha)   +   servidor/cluster.rs
mod cluster;
// -------------------------------------------------------------------
use super::{Servidor, Sessao};
impl Servidor {
    pub fn do_filho(&self) -> u32 {
        let _s: Option<Sessao> = None;   // tipo privado do pai
        self.dados + self.travar()       // campo privado + método privado
    }
}
```
```
0 erros
```

E não é preciso sequer renomear o arquivo: na edição 2021 — a do workspace —
`servidor.rs` **continua se chamando `servidor.rs`** e os filhos moram em
`servidor/`. Também medido: 0 erros. O `git` vê o arquivo encolher e arquivos
novos aparecerem; não há renomeação, então o histórico das linhas que ficam
sobrevive intacto.

**As travessias custam zero na divisão em filhos.** O número da seção 2 está
certo — é o mesmo número — mas ele mede o custo de um layout que não é o que se
deve usar. *Diagnóstico plausível não é diagnóstico medido*, e este quase virou
o plano da sprint.

### 3.2 O que sobra de acoplamento real

Descontada a visibilidade, sobram quatro coisas — e são estas que custam.

**(a) O `self` gordo.** `crates/phxsql-server/src/servidor.rs:467-684`. O campo
`config` é lido por 76 das funções, `telemetria` por 25, `transacoes` por 24.
Isto **não** impede a divisão em filhos (todo filho enxerga todos os campos),
mas impede a divisão em **tipos**: não há como dar a `cluster.rs` um
`&EstadoDoCluster` enxuto enquanto ele precisa de `config` e `telemetria`
junto. É o teto da sprint, não o piso dela.

**(b) A trava de dados.** `travar_dados` é chamada por 74 métodos e
`abrir_travada` por 32 — são a encanação comum, e quase toda região passa por
ali.

**(c) O estado e as constantes de módulo — o mais perigoso, e o mais fácil de
não ver.** Declarações cujos leitores ficam **longe** delas:

<!-- mapa:globais:inicio -->
| nome | declarado | usado nas linhas | regiões que atravessa |
|---|---|---|---|
| `VERSAO` | 58 | 4250, 4504, 4513, 4574, 6094, 12142, 13801 | operacoes-de-dados, portoes-e-despacho, web-http-rest |
| `OPS_ESCRITA` | 61 | 4696, 5090, 5419, 5740, 5790, 5873, 7625, 15881, 15935 | operacoes-de-dados, porta-de-dados-e-aperto, portoes-e-despacho, web-http-rest, **fora de toda região** |
| `OPS_NO_SPARE` | 155 | 5729, 16661 | portoes-e-despacho, **fora de toda região** |
| `OPS_DE_TRANSACAO` | 204 | 7579 | operacoes-de-dados |
| `OPS_EMPILHAVEIS` | 230 | 7605, 7630 | operacoes-de-dados |
| `OPS_DE_REPLICACAO` | 244 | 5770, 16562, 16580, 16617 | portoes-e-despacho, **fora de toda região** |
| `MARCAS_POR_TABELA` | 419 | 14574 | operacoes-de-dados |
| `TETO_DO_LOTE_SERVIDO` | 436 | 14564 | operacoes-de-dados |
| `PRAZO_DO_GATILHO_ANTES` | 465 | 10056 | operacoes-de-dados |
| `AMOSTRA` | 10962 | 10966 | operacoes-de-dados |
| `COM_A_TRAVA` | 15199 | 938, 969, 15557 | arranque-e-identidade, **fora de toda região** |
| `CADEIA_MAXIMA` | 15585 | 10125, 21516 | operacoes-de-dados, **fora de toda região** |
| `PROFUNDIDADE_DA_CADEIA` | 15593 | 10124, 10136, 10137 | operacoes-de-dados |
| `CADEIA_CORTADA` | 15600 | 10130, 10134, 10166 | operacoes-de-dados |
| `TETO_PIVOT` | 15698 | 7331, 7332 | operacoes-de-dados |
| `TETO_JUNCAO` | 15700 | 7177, 7182, 12762 | operacoes-de-dados |
<!-- mapa:globais:fim -->

`COM_A_TRAVA` é a guarda contra o abraço mortal com a própria trava —
*«aconteceu três vezes neste projeto»*. Ela é lida e armada em `travar_dados` e
desarmada no `Drop` de `TravaMedida`, que fica **fora de toda região**.

> **Se `travar_dados` for para um filho e o `Drop` ficar no pai — ou o
> contrário — e alguém redeclarar o `thread_local!` no arquivo novo em vez de
> importá-lo, nascem duas células independentes e a guarda para de guardar em
> silêncio.**

O modo de falha não é um `assert` vermelho: é o servidor **pendurando**, sem
log e sem pilha, levando junto todas as outras conexões. O teste que trava isso
— `a_trava_pedida_duas_vezes_pela_mesma_thread_vira_erro` — roda com prazo
justamente porque a falha é uma parada, e ele acusaria em 30 s em vez de
pendurar o `cargo test` inteiro.

**Regra para a sprint:** `travar_dados`, `TravaMedida` (com os três `impl`
dele) e o `thread_local! COM_A_TRAVA` são **uma peça só**. Vão juntos ou ficam
juntos, nunca separados. O mesmo vale para o par
`PROFUNDIDADE_DA_CADEIA` / `CADEIA_CORTADA`, que a cadeia de gatilhos lê e
escreve em pontos diferentes da mesma função.

E as listas `OPS_*` ficam no pai: `OPS_ESCRITA` é lida de quatro regiões e
ainda de fora do módulo (`catalogo.rs`). Um filho que redeclarasse uma delas
teria uma segunda verdade sobre o que é escrita.

**(d) O portão de permissão, e o campo que ele lê.** `portoes_do_pedido` é o
portão único, e ele lê `pedido.texto_ou("tabela")`.

<!-- mapa:portao-numeros:inicio -->
|  |  |
|---|---|
| operações `op_*` | **116** |
| leem o campo `"tabela"` **do pedido** | **24** |
| nomeiam tabela por **outro** campo | **9** (**7** são tabela de verdade) |
| carregam conferência **própria** de permissão | **22** |
<!-- mapa:portao-numeros:fim -->

As que o portão não enxerga:

<!-- mapa:portao-furos:inicio -->
| operação | linha | campo do pedido | tabela aninhada em | o que é | confere por conta |
|---|---|---|---|---|---|
| `op_copiar_tabela` | 6797 | `destino`, `destino_database` | — | tabela | sim |
| `op_pivotar` | 7094 | — | `j.tabela` | tabela | sim |
| `op_duplicar_tabela` | 9535 | `destino` | — | tabela | sim |
| `op_renomear_tabela` | 9575 | `destino` | — | tabela | sim |
| `op_backup` | 11528 | `destino` | — | **caminho de arquivo** | não precisa |
| `op_conferir_backup` | 11605 | `destino` | — | **caminho de arquivo** | não precisa |
| `op_juntar` | 12779 | — | `pa.tabela`, `pb.tabela` | tabela | sim |
| `op_unir` | 12904 | `tabelas` | `x.tabela` | tabela | sim |
| `op_dblink_ligar` | 13166 | `tabelas` | — | tabela | sim |
<!-- mapa:portao-furos:fim -->

**Todos os furos reais estão tapados hoje** por uma conferência dentro da
própria operação. As duas do `backup` são falso positivo do crivo por nome: ali
`destino` é um diretório, e o gerador separa os dois casos **pelo uso**
(`Path::new(&destino)`) em vez de pelo nome — furo inventado gasta a mesma
leitura que o verdadeiro.

> **O que isso obriga na divisão:** essas conferências próprias *parecem*
> duplicação. Se `op_juntar`, `op_unir` e `op_pivotar` forem para um
> `servidor/consultas.rs` e alguém "limpar" o bloco de permissão repetido, **a
> porta dos fundos reabre — e nenhum teste do portão pega**, porque o portão
> nunca viu aquelas tabelas.

Os testes que pegam estão em `testes_direito_por_tabela`:
`juntar_nao_e_a_porta_dos_fundos`, `unir_nao_e_a_porta_dos_fundos` e
`pivotar_nao_e_a_porta_dos_fundos`. **Esses três testes viajam com essas três
operações.**

### 3.3 As portas de entrada, e os irmãos de cada uma

*Conserto entra no caminho que o motivou e o caminho irmão fica.* O gerador
confere qual portão cada porta alcança — pela rota **mais curta**, não por uma
rota qualquer (em profundidade ele dizia que `executar_derivado` chega ao
portão «via `executar` > `op_job_rodar` > `rodar_job`», quando ele chama
`portoes_do_pedido` na linha seguinte; rota que existe não é a rota que o
pedido faz):

<!-- mapa:portas:inicio -->
| porta | linha | linhas | portão | via |
|---|---|---|---|---|
| `escutar` | 983 | 86 | **NENHUM** | direto |
| `atender` | 5157 | 293 | `despachar` | direto |
| `atender_http` | 4174 | 143 | `despachar` | `api_http` |
| `atender_rest` | 4480 | 61 | `despachar` | `api_rest` |
| `atender_swagger` | 4547 | 58 | **NENHUM** | direto |
| `executar_derivado` | 3796 | 5 | `portoes_do_pedido` | direto |
| `executar_job` | 3802 | 9 | `portoes_do_pedido` | direto |
<!-- mapa:portas:fim -->

Os irmãos, para quem for mexer em qualquer um deles:

- **As quatro portas que atendem soquete**: `atender` (dados), `atender_http`,
  `atender_rest`, `atender_swagger`. Três passam pelo `portao_de_rede_http`; a
  de dados tem o seu próprio caminho.
- **Os dois laços de aceitação**: `aceitar_ate_mandarem_parar` (porta de dados)
  e `aceitar_http` (servindo web, REST e swagger).
- **Os três `subir_*` de porta**: `subir_web`, `subir_rest`, `subir_swagger`.
- **Os dois caminhos que não vêm da rede**: `executar_derivado` (o SQL
  traduzido) e `executar_job` (o agendador). Ambos chamam `politica_do_pedido`
  **e** `portoes_do_pedido` antes de `executar` — e é isso que os impede de
  serem a porta dos fundos.

`atender_swagger` não alcança portão nenhum, e está certo: ele serve
`/openapi.json` e os rótulos de tela. Mas ele **toma a trava de dados** para
ler `phxsys.mensagens`, e é a única função da região `web-http-rest` que toca a
trava sem alcançar um portão. Quem dividir essa região precisa saber disso
antes de "simplificar" a dependência.

---

## 4. A ordem proposta

O gerador ordena as candidatas pela razão entre linhas movidas e travessias
pagas, e marca a primeira:

<!-- mapa:custo-por-fatia:inicio -->
| região | linhas | travessias (sai+entra) | linhas por travessia | campos | imports |
|---|---|---|---|---|---|
| **cluster** | 555 | 6 | 92,5 | 2 | 6 |
| bidirecional | 330 | 4 | 82,5 | 3 | 5 |
| web-http-rest | 968 | 12 | 80,7 | 8 | 14 |
| jobs | 580 | 11 | 52,7 | 6 | 7 |
| replicacao | 328 | 7 | 46,9 | 3 | 6 |
| porta-de-dados-e-aperto | 404 | 9 | 44,9 | 6 | 14 |
| backup-agendado | 131 | 3 | 43,7 | 2 | 4 |
| relogios-de-fundo | 63 | 2 | 31,5 | 2 | 4 |
| config-e-servico | 243 | 8 | 30,4 | 11 | 10 |
| firewall-e-mensagens | 257 | 11 | 23,4 | 4 | 15 |
| porta-e-aceitacao | 156 | 12 | 13,0 | 7 | 9 |
<!-- mapa:custo-por-fatia:fim -->

### A fatia mais barata

<!-- mapa:fatia-mais-barata:inicio -->
**`cluster` — 555 linhas, 12 funções, `servidor.rs:1969-2570`.** Paga 6 travessias (3 saindo, 3 entrando), toca **2** dos 44 campos do `Servidor` (`cluster`, `telemetria`) e precisa de 6 dos 70 imports do topo — a melhor razão **92,5 linhas por travessia** de todas as regiões candidatas.

As travessias, nomeadas: ela chama `anotar`, `rodada_da_replica`, `travar_dados`; é chamada em `op_cluster_estado`, `op_cluster_pulso`, `subir_cluster`.
<!-- mapa:fatia-mais-barata:fim -->

O campo `cluster` é `Option<Arc<EstadoCluster>>` — quando o `config.json` não
traz o bloco, *nada disto existe: nenhuma thread, nenhum portão*
(`servidor.rs:641-642`). Já existe um `crate::cluster` irmão com o
`EstadoCluster`; o filho novo leva os métodos que ficaram para trás.

**O que ela quebra se extraída errado:**

1. **Se for irmão em vez de filho** — não compila, e é a falha barata:
   `travar_dados` e `Sessao` são privados. Erro na hora, não em produção.
2. **Se levar `rodada_da_replica` junto** — ela é da região `replicacao` e é
   chamada também pelo laço de réplica comum. Levar arrasta a replicação
   inteira e a fatia deixa de ser barata.
3. **`promover_a_master` tem uma irmã que fica no pai: `promover_para_primario`
   (`servidor.rs:844`).** São duas promoções, e — medido, não suposto — elas
   **não** mexem no mesmo estado: a do cluster chama `estado.promover(época)`,
   a do spare escreve os atômicos `papel_vivo` e `somente_leitura_vivo`. A
   disjunção é de propósito, e quem escolhe entre as duas é o portão de escrita
   em `portoes_do_pedido` (`servidor.rs:5790-5800`): *com* cluster vale
   `estado.recusa_de_escrita()`, *sem* cluster vale o `somente_leitura` vivo.
   Separá-las em arquivos diferentes é legítimo — mas o
   `if let Some(estado) = &self.cluster` que as escolhe fica no pai, e tem de
   continuar sendo o **único** lugar que decide. Uma terceira promoção escrita
   depois, num filho, sem passar por esse `if`, é a porta dos fundos pelo mesmo
   molde do `juntar`.
4. **Se levar o `Drop` de `TravaMedida`** — ver 3.2(c). Não leva.

Depois dela vale a mesma ordem da tabela acima. `portoes-e-despacho` sai **por
último entre as pequenas**: é limpa, mas é o roteador, e mexer nela enquanto as
operações ainda estão todas no pai troca centenas de linhas de lugar sem
separar nada. `operacoes-de-dados` **não é uma fatia** — é o que sobra, e só se
divide por assunto depois que as demais saíram.

### O primeiro commit da sprint, se for um só

Antes de qualquer região: **mover `travar_dados` + `TravaMedida` + os três
`impl` dele + o `thread_local! COM_A_TRAVA` para `servidor/trava.rs`, juntos.**
São ~90 linhas hoje espalhadas por catorze mil, e são a peça que toda outra
região precisa. Juntá-las é o único movimento que deixa a árvore **melhor** que
antes mesmo se a sprint parar ali.

---

## 5. O risco medido

### Os testes de dentro

<!-- mapa:testes-resumo:inicio -->
**7.331 linhas em 18 módulos** vivem dentro do arquivo — 32% dele. E eles não são caixa-preta: **18 dos 18** alcançam pelo menos um item privado do módulo.
<!-- mapa:testes-resumo:fim -->

<!-- mapa:testes:inicio -->
| módulo | linhas | privados que alcança |
|---|---|---|
| `testes_politica` | 101 | `OPS_ESCRITA` |
| `testes_firewall_e_mensagens` | 442 | `Sessao`, `despachar`, `executar` |
| `testes_papel` | 285 | `OPS_DE_REPLICACAO`, `OPS_NO_SPARE`, `Sessao`, `despachar`, `portoes_do_pedido` |
| `testes_supressao_de_origem` | 124 | `Sessao` |
| `testes_criar_qualificada` | 127 | `Sessao`, `executar` |
| `testes_exclusao` | 534 | `Sessao`, `despachar`, `executar` |
| `testes_conflito` | 231 | `Sessao`, `executar` |
| `testes_direito_por_tabela` | 585 | `Sessao`, `despachar`, `executar` |
| `testes_profiler_desligado` | 115 | `Sessao`, `executar` |
| `testes_portao_do_profiler` | 117 | `Sessao`, `despachar` |
| `testes_bulkinsert` | 223 | `Sessao`, `despachar`, `executar` |
| `testes_sql` | 264 | `Sessao`, `despachar`, `executar` |
| `testes_gatilhos` | 666 | `Sessao`, `despachar`, `executar` |
| `testes_chave_estrangeira` | 653 | `Sessao`, `despachar`, `executar` |
| `testes_config_gravar` | 369 | `Sessao`, `despachar`, `executar` |
| `testes_restaurar_backup` | 538 | `Sessao`, `despachar`, `executar` |
| `testes_janela_e_cadeia` | 430 | `Sessao`, `despachar`, `executar`, `travar_dados` |
| `testes_transacoes` | 1.410 | `Sessao`, `despachar` |
<!-- mapa:testes:fim -->

**O que se perde de cobertura ao mover: nada, se os testes forem filhos.** Um
`#[cfg(test)] mod testes_x` dentro de `servidor/cluster.rs` continua sendo
descendente do módulo `servidor` e continua enxergando `Sessao`, `despachar` e
`travar_dados`. **Se forem para `tests/`, perdem-se os módulos inteiros** —
nenhum deles compila fora do módulo.

`testes_janela_e_cadeia` é o mais sensível: é o único que chama `travar_dados`
direto, e é onde mora o teste da guarda contra o abraço mortal.

### Os testes de fora

Aqui o risco é quase nulo, e é uma boa notícia medida.

<!-- mapa:externos:inicio -->
| arquivo | usa |
|---|---|
| `crates/phxsql-cmd/tests/console.rs:19` | `Servidor` |
| `crates/phxsql-server/tests/cache-paginas-pelo-config.rs:35` | `Servidor` |
| `crates/phxsql-server/examples/custo-da-transacao.rs:38` | `ExecutorLocal` |
| `crates/phxsql-server/examples/custo-da-trava.rs:42` | `ExecutorLocal` |
| `crates/phxsql-server/examples/custo-do-portao.rs:32` | `ExecutorLocal` |
| `crates/phxsql-server/src/catalogo.rs:31` | `OPS_ESCRITA` |
| `crates/phxsql-server/src/lib.rs:43` | `Servidor` |
| `crates/phxsql-server/src/main.rs:141` | `ExecutorLocal` |
<!-- mapa:externos:fim -->

<!-- mapa:externos-resumo:inicio -->
Apenas **8 arquivos** fora do módulo citam `servidor::`, e entre eles usam **3** itens: `ExecutorLocal`, `OPS_ESCRITA`, `Servidor`. A superfície pública real do `servidor.rs` são esses 3 nomes — uma divisão em filhos preserva os três sem tocar em nenhum.

`crates/phxsql-server/tests/` tem **14** baterias com **4.347** linhas ao todo, e **13 delas** falam com o servidor **pelo soquete** sem citar o módulo. São a rede de segurança da sprint.
<!-- mapa:externos-resumo:fim -->

---

## O que este documento não faz

Não move código, não propõe assinatura nova, não mede desempenho. A fatia
proposta na seção 4 é uma **hipótese com número ao lado**, e o número que a
sustenta é a razão linhas-por-travessia. Quem executar a SP000005 confere a
premissa antes de implementar o item — *inclusive quando o item é nosso*.

E o número que mais importa aqui é o que **mudou de ordem de grandeza quando
foi medido**: mais de cem travessias no layout irmão, **zero** no layout filho.
Era a diferença entre uma sprint de dez rodadas e uma de uma.
