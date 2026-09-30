# O `eprintln!` não é uma escrita só: a linha lida de outro processo só vale com o `\n`

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-server/tests/porta-lida-pela-metade.rs::o_endereco_pela_metade_espera_o_resto`; `crates/phxsql-server/tests/porta-lida-pela-metade.rs::a_porta_pela_metade_nao_vira_outra_porta`

## O que aconteceu

Pedido 581. O `permissao-do-banco::a_base_antiga_por_link_simbolico_tambem_alerta`
caiu uma vez no `./portoes.sh` com `AddrParseError` no `unwrap()` do
`tests/comum/mod.rs:179` — o `porta_do_phxsqld`, que lê o arquivo de erro
padrão do `phxsqld` a cada 20 ms e procura a linha
`porta de dados escutando em <endereço>`. Isolado, 5 de 5 verdes.

## O que eu concluí primeiro, e estava errado

Que a linha era **uma** escrita e o risco era o leitor pegar o arquivo antes
de ela existir — e isso o `find_map` já tratava, voltando `None`. Nessa
leitura o `parse` só falharia se o texto do servidor mudasse, e o floco não
tinha explicação. O erro estava na premissa: «uma linha do `eprintln!` chega
inteira ao arquivo».

## O que a medição disse

`strace -f -e trace=write` no `phxsqld` subindo com `bind 127.0.0.1:0`:

```
write(2, "porta de dados escutando em ", 28) = 28
write(2, "127", 3)                        = 3
write(2, ".", 1)                          = 1
write(2, "0", 1)                          = 1
...
```

O `stderr` da `std` não tem buffer, e o `write_fmt` escreve cada pedaço do
formato — e o `Display` do `SocketAddr` escreve octeto por octeto. Quem lê no
meio vê o prefixo INTEIRO com o endereço pela metade: `127.0.` dá
`AddrParseError` (o floco), e `127.0.0.1:43` de `:43210` **passa** no `parse`
com a porta errada — o pior dos dois, porque a queda aparece longe, no
`connect`.

A janela é de microssegundos: **0 quedas em 230 corridas** do teste original
sob 8 laços de CPU em 4 núcleos, antes do conserto. Por isso a prova não é o
laço, e sim um servidor falso (`sh` com `printf` em dois pedaços e 400 ms no
meio): com o defeito reposto, os dois testes caem toda vez (guarda
`porta-lida-pela-metade`, PROVADA 2/2).

## A regra

**Quem lê de outro processo uma linha que ele escreve com `println!`/`eprintln!`
só confia na linha que já tem o `\n`** — descarte o resto depois do último
`\n` antes de casar prefixo. Casar o prefixo não prova que a linha acabou.

## Como está guardado hoje

- O motor é `comum::porta_no_texto` (`crates/phxsql-server/tests/comum/mod.rs`):
  só olha até o último `\n`, e linha INTEIRA ilegível volta `Err` na hora
  (esperar não a consertaria).
- As quatro cópias do `porta_aberta` que liam a mesma linha com o mesmo
  `find_map` + `parse().unwrap()` (`config-phz.rs`, `core-sem-segredo.rs`,
  `caminhos-do-config.rs`, `cadastro-acessorio-trancado.rs`) passaram a chamar
  o motor — eram irmãs de verdade: mesma leitura, mesma ordem.
- **O buraco que fica:** o servidor continua escrevendo a linha em pedaços.
  Qualquer leitor novo fora do `comum` (um script de bancada, uma ferramenta)
  pode repetir o erro; o `provar-demonstracao-phz.py` só testa presença do
  prefixo e não lê o endereço, então hoje não é afetado.
