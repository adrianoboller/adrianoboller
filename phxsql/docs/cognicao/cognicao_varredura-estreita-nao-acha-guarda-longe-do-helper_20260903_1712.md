# Varredura estreita não achou um guarda que já existia — pedido 150

## 1. O que aconteceu

Para medir a premissa do pedido 150 ("quantos testes criam diretório
temporário, quantos já limpam, quantos não") escrevi um script
(`medir_testes_temp.py`) que varre cada `fn` do código, acha as que chamam
`std::env::temp_dir()`, e decide se o teste já está limpo checando se existe
`impl Drop` **perto** do corpo do helper (uma janela de 50 caracteres antes do
início da função). O resultado: 524 testes "sujos" (sem `Drop`), incluindo os
20 de `crates/phxsql-ffi/src/testes.rs`.

Ao converter os helpers de `phxsql-store`, resolvi conferir se algum outro
arquivo já tinha um guarda parecido antes de mexer nele — rodei `grep -rln
"impl Drop for" crates/*/tests/ crates/*/src/` sem a janela estreita, olhando
o arquivo inteiro. `phxsql-ffi/src/testes.rs` apareceu, e o `impl Drop for
Area` estava lá desde sempre, a **43 linhas** da função `Area::nova` — fora da
janela de 50 caracteres que o script checava perto do *corpo*, não do
`Area::nova` inteiro.

## 2. O que eu concluí primeiro, e estava errado

Concluí, a partir do número do script, que os 524 testes precisavam do mesmo
tratamento — helper devolve `PathBuf` cru, sem guarda, herda o defeito do
pedido 150. Ia converter `phxsql-ffi/src/testes.rs` junto com os outros.

Estava errado: `Area` (o wrapper que `Area::nova` já devolve nesse arquivo) **já
tem** `impl Drop for Area` que apaga o diretório — o padrão certo, exatamente o
que o pedido 150 pede, só que sob outro nome. O script não viu porque procurou
`impl Drop` perto de onde o `PathBuf`/temp_dir aparece, e não perto de onde o
**tipo devolvido** (`Area`) é declarado — e neste arquivo os dois ficam
longe um do outro.

## 3. O que a medição disse

524 → **504** depois da correção: 20 dos "sujos" já estavam limpos, e a
diferença é exatamente `phxsql-ffi/src/testes.rs`. Conferido também que os
outros 7 `impl Drop for` do repositório (`TravaMedida` em `servidor.rs`,
`Preparada` em `restaurar.rs`, `NdxFile` em `ndx.rs`, `ComAJanela` em
`exclusao-na-janela.rs`, e os dois guardas deste próprio pedido) protegem
recursos **diferentes** de diretório temporário de teste — nenhum outro falso
positivo achado nos 148 arquivos varridos.

## 4. A regra

Antes de contar algo como "sem guarda", procure o `impl Drop` pelo **tipo
devolvido** no arquivo inteiro (`grep "impl Drop for <Tipo>"` sem janela), não
perto de onde o recurso é criado — o construtor e o `Drop` podem estar a
dezenas de linhas de distância, principalmente quando o guarda tem outros
métodos no meio.

## 5. Como está guardado hoje

`phxsql-ffi/src/testes.rs` não foi tocado neste pedido — já cumpria o padrão
antes de o pedido existir, e fica como referência de como o `Drop` deveria
estar em todo canto. O número final relatado no `PENDENCIAS.md` (pedido 150)
usa os 504, não os 524 do primeiro script.
