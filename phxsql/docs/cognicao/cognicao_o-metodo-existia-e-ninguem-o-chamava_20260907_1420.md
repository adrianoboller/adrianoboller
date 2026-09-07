# O método existia, estava correto, e ninguém o chamava — pedido 218

**07/09/2026, 14h20. Frente F2 (cluster).**

## 1. O que aconteceu

A frente F8 achou, capturando as telas de configuração, que a chave `"cluster"`
**não aparece em lugar nenhum** da resposta de `{"op":"config"}`. Não vinha
oculta como `token` (`"(oculto)"`) nem rotulada como `cifra.senha`
(`"(vazia)"`): vinha **ausente**. Consequência prática: a tela de cluster do
pedido 208 não tinha de onde ler.

Ao abrir o `config.rs` para escrever o serializador, ele **já estava lá**:

```rust
// crates/phxsql-server/src/config.rs
impl Cluster {
    /// O resumo que a op `config` mostra. Sem token e sem hash: resposta de
    /// protocolo nao carrega credencial.
    pub fn para_json(&self) -> Json { … }
}
```

Escrito, comentado, correto, com os segredos já de fora. E `grep -rn
"cluster.para_json" crates/` devolvia **zero**. O `Config::para_json` — o que a
op `config` chama de verdade — nunca o chamava. Foi uma linha de uma dessas.

O `pub` é o que fez isso durar: método público não gera `dead_code`, e o
compilador nunca teve como avisar.

## 2. O que eu concluí primeiro, e estava errado

**«O bloco não aparece porque ninguém escreveu o serializador dele.»** Foi a
leitura óbvia do achado da F8 — «`Config::para_json` não a serializa» —, e ela
me mandou começar escrevendo um serializador que já existia. Se eu tivesse
seguido, teria nascido um **segundo** `para_json` do cluster, com a mesma
decisão sobre o segredo tomada de novo — e duas decisões iguais escritas em
dois lugares divergem no primeiro campo que alguém acrescentar de um lado só.

E o segundo erro, mais interessante: **achei que o teste de vazamento já
cobria o cluster.** Ele cita o cluster pelo nome:

```rust
("token do cluster", "MARCA-TOKEN-CLUSTER"),
("hash do cluster", "pbkdf2-sha256$1000$a4$dead0004"),
```

Cobria coisa nenhuma. Marca que não foi serializada não aparece no texto, então
o teste passava **com o bloco inteiro ausente** — verde, citando o cluster, e
sem guardar nada. É o «teste que passa por engano» da pétrea, na forma mais
difícil de ver: não é um teste errado, é um teste **certo cuja premissa sumiu**.

## 3. O que a medição disse

- `grep -rn "cluster.para_json\|c.para_json" crates/` → **0 chamadas** para um
  método `pub` de 22 linhas.
- O conserto foi **1 linha** no `Config::para_json` (mais o `Json::Nulo` para o
  caso sem cluster).
- **Prova real, com o defeito reposto** — tirando essa linha, os dois testes
  novos falham nomeando o que falta:

  ```
  o_bloco_cluster_aparece_na_op_config ... FAILED
    a chave "cluster" tem de existir
  sem_cluster_a_chave_vem_nula_e_nao_some ... FAILED
    a chave tem de existir e ser nula: {"bind":…,"editaveis":[…]}
  ```

  Com a linha de volta, os dois passam — e o de vazamento passa a valer, porque
  agora há o que vazar.

## 4. A regra

> **Guarda que cita um campo só vale enquanto o campo existe. Toda guarda de
> ausência ("o segredo não sai") precisa da irmã de presença ("o campo sai"),
> senão ela passa verde sobre o vazio.**

E o corolário, sobre achar o buraco: **antes de escrever o serializador que
falta, procure o que já existe e não é chamado** — `pub` desliga o aviso do
compilador, e é justamente nos módulos que nasceram inteiros de uma vez
(cluster, replicação) que sobra método pronto sem chamador.

## 5. Como está guardado hoje

- A chamada: `Config::para_json` devolve `cluster` ou `Json::Nulo` —
  `crates/phxsql-server/src/config.rs`.
- As duas guardas novas: `o_bloco_cluster_aparece_na_op_config` (presença, com
  id, prioridade, janela e a lista de nós) e
  `sem_cluster_a_chave_vem_nula_e_nao_some` (a chave existe e é nula, para a
  tela distinguir «não estou em cluster» de «o servidor não me contou»).
- A guarda de vazamento que passou a valer:
  `nenhuma_credencial_do_config_sai_pela_op_config`.
- O exercício pelo navegador:
  `testes-web/capturas-cluster.mjs`, passo *«o bloco `cluster` chega na resposta
  de `config`»* — e o irmão dele, *«e o token e o hash do cluster NÃO chegam
  junto»*, contra três servidores de verdade.

**Onde o buraco ficou:** não há conferidor genérico de «campo `pub` de
serialização sem chamador». Um casador de `fn para_json` versus `grep` do nome
acharia este caso, e também acusaria os legítimos (os que só o dono do módulo
chama). Fica registrado como recusa por enquanto — a mesma medida do conferidor
de erro cru: **8 interpolações no repositório e só 2 eram defeito**, e um
casador teria reprovado as outras seis.
