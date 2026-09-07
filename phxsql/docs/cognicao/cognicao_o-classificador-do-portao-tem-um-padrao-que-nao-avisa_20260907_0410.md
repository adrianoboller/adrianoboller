# O portão é um só — e o classificador dele tem um padrão que não avisa

*Descoberto em 07/09/2026, 04h10, ao pôr a operação `procurar_texto` no
protocolo.*

## 1. O que aconteceu

A operação nova nasceu com o campo `"tabela"` no primeiro nível, que é
exatamente o que a pétrea do portão único manda. O teste do portão passou na
metade que nega — a tabela negada foi negada — e **falhou na metade que
permite**:

> `[sp000025] acesso negado: ana nao tem permissao de administrar em b.clientes`

`Atividade::da_operacao` termina em `_ => Atividade::Administrar`. Para uma
operação **desconhecida** isso está certo, e é a decisão certa: negar por
omissão. O que esse `_` não distingue é a operação que **está no catálogo** e
esqueceu a linha dela.

Escrevi então um conferidor que lê o fonte da própria função e exige que cada
nome do `OPERACOES` apareça escrito ali. Ele achou **13 outras**.

## 2. O que eu concluí primeiro, e estava errado

Que a lição do dia era a de sempre — *o portão lê um campo, procure quem não
tem esse campo* — e que bastava conferir se `procurar_texto` carregava
`"tabela"`. Carregava. O portão nunca teve defeito nenhum: quem tinha era a
**tabela de poderes** que ele consulta, e ela falha por um mecanismo diferente
— não por ler o campo errado, mas por responder **por omissão** com a cara de
quem respondeu por regra.

E concluí errado uma segunda vez, mais rasa: que o achado seria uma lista de
permissões erradas para consertar. Não era.

## 3. O que a medição disse

**13 operações** caíam no `_`:

| grupo | quais | quantas |
|---|---|---:|
| DDL e reparo | `renomear_tabela`, `reparar` | 2 |
| memória | `memoria_liberar` | 1 |
| DBLINK | `dblink_salvar` … `dblink_sincronizar` | 10 |

E o detalhe que mais ensina: **um teste já afirmava uma delas.**
`a_memoria_pede_leitura_e_o_backup_pede_administrar` diz que `memoria_liberar`
pede administrar — e passava porque o `_` respondia isso, não porque alguém
tivesse decidido. É a forma mais silenciosa de teste que passa por engano: ele
está certo no resultado e vazio na causa.

**Nenhum poder mudou no conserto.** As 13 passaram a estar escritas com o valor
que já valia, cada uma com o motivo. Rebaixar qualquer uma agora tiraria ou
daria direito sem ninguém ter pedido — e o `dblink_ler`, que parece leitura, é
o caso que decide: o poder ali é sobre a **credencial de outro servidor**, e o
dado do outro lado não tem portão nosso.

## 4. A regra

**Padrão de segurança que nega está certo para o desconhecido e errado para o
catalogado: quando houver uma lista e um `_`, escreva o conferidor que casa as
duas.** E o conserto declara o valor que JÁ valia — quem quiser mudá-lo passa a
ter de dizer.

## 5. Como está guardado hoje

`usuarios::tests::toda_operacao_do_catalogo_declara_o_poder_que_pede` lê o
fonte da função (`include_str!`) e nomeia as que faltam. Ele **recusa passar
vendo zero** — confere primeiro que achou o corpo certo —, pelo mesmo motivo do
conferidor de textos fora da fábrica: leitor quebrado aprovaria tudo.

Guarda `operacao-sem-poder-declarado` no catálogo, com o defeito reposto
tirando a linha do `procurar_texto`: caem o conferidor e o teste do portão.

**O buraco que eu ia deixar, e não deixei:** a primeira versão do conferidor
casava só o nome **canônico**, e um **apelido** (`join`, `union`, `pivot`) que
existisse apenas no catálogo passaria por ele e cairia no `_` no ar. Escrevi
isso aqui como buraco declarado, reli, e vi que fechá-lo custava trocar
`o.nome` por `o.nomes()` — uma linha. *Buraco que se fecha em trinta segundos
não é buraco declarado, é preguiça documentada.* Hoje o conferidor cobre nome e
apelido, e passa.
