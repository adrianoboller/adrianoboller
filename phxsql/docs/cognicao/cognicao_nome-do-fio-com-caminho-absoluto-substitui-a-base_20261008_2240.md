# Nome que vem do fio com caminho absoluto substitui a base: só a parte validada estava protegida

## O que aconteceu

Pedido 726 (revisão SEC de 08/10/2026, A1). `Database::trocar_pelo_retrato`
passava a TABELA pelo `validar_nome` e o SCHEMA direto em
`self.caminho().join(sc)`. O nome vem da origem pelo fio, e `Path::join` com
caminho absoluto **substitui** a base: `{"tabela":"/srv/.../vizinho.produtos"}`
apagava e reescrevia o `produtos.reg` de outro database da réplica. O irmão,
`retratar_tabela`, já usava `self.diretorio(schema)` — que valida.

## O que eu concluí primeiro, e estava errado

Que a variante `..` do achado (schema `..` subindo para a raiz) também se
alcançava, e escrevi a prova dela. Morreu: `separar_qualificado` corta no
PRIMEIRO ponto, então `"...produtos"` dá schema nenhum, e o `..` com barra cai
na tabela, que já passava pelo `validar_nome`. O caminho absoluto era a única
fuga.

## O que a medição disse

Com o defeito reposto, pelo soquete, a origem falsa (`TcpListener` que anuncia
`base` 5 e responde o retrato) deixou o `vizinho/produtos.reg` com os 8 bytes
`ATACANTE`. Com o conserto, o arquivo fica `ORIGINAL` e a rodada recusa.

## A regra

Nome que vem do fio vira caminho pela MESMA função que todo o catálogo usa
(`Database::diretorio`), nunca por `join` direto — e quando dois caminhos
irmãos montam o mesmo diretório, procure o que montou à mão.

## Como está guardado hoje

`crates/phxsql-server/tests/retrato-da-replica-adverso.rs::o_retrato_com_caminho_absoluto_nao_escreve_fora_do_database`
e `phxsql-store` `catalogo::testes_gestao::o_retrato_com_schema_absoluto_nao_sai_do_database`.
Nenhum conferidor genérico procura `join` com nome externo; o buraco é esse.
