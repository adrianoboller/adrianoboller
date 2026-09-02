# Duas portas de criação divergem em silêncio

- **Quando:** 2026-09-02, 19:10 (SP000057, `ao_alterar`)
- **Onde:** `crates/phxsql-core/src/schema.rs` contra
  `crates/phxsql-server/src/valores.rs`
- **Custo:** **a mesma tabela nascia com integridade referencial diferente
  conforme quem a criasse**

## O que aconteceu

A chave estrangeira pode nascer por duas portas: a API Rust
(`ForeignKey::new`) e o JSON do protocolo (`acao_ri_de_texto`, campo ausente).

- Pela API: `ao_alterar` nascia **`Restringir`**.
- Pelo JSON: nascia **`Cascata`**.

Ninguém tinha escrito isso; cada porta tinha um padrão próprio, e os dois
padrões nunca se encontraram porque **nenhum teste criava a mesma chave pelas
duas portas e comparava**.

## O que eu concluí primeiro, e estava errado

Escrevi no briefing da frente que «`ao_alterar` nasce cascata em
`schema.rs:166`». Era falso, e eu tinha lido o arquivo. Li a linha do
`SchemaBuilder` e generalizei para as duas portas — que é o erro clássico de
ler UM caminho e concluir sobre o comportamento.

## O que a medição disse

A frente reconferiu antes de implementar e achou os dois valores. Também achou
que o teste citado no comentário do código **não existia mais** — comentário
apontando para teste apagado não trava nada, e parece que trava.

## A regra

**Toda configuração com mais de uma porta de entrada precisa de um teste que
entre pelas duas e compare o resultado.** E: comentário que cita um teste tem
de citar um teste que existe — a citação envelhece calada quando o teste sai.

## Como está guardado hoje

`ForeignKey::new` passa a devolver `Cascata`, fechando a divergência, e há
teste (`a_chave_nasce_com_cascata_ao_alterar`) que reprova se o padrão voltar.

**O buraco que ficou:** não há guarda genérica contra «comentário cita teste
que não existe». É varredura barata de escrever, e ainda não foi escrita.
