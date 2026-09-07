# Guarda que confere o diretório não pega extensão que o teste não fabricou — pedido 213

## 1. O que aconteceu

Investigando o pedido 213 (o `.fts` faltando na Figura 1, na Figura 8 e na
tabela-mestra do `docs/FORMATO.md`), achei a causa: `EXTENSOES_TODAS`, em
`phxsql-store/src/catalogo.rs`, também não tinha `.fts`. Era a **terceira**
vez que essa constante ficava incompleta — nasceu com seis extensões quando a
tabela já tinha nove, depois com nove quando já tinha dez —, e as duas vezes
anteriores geraram uma guarda deliberadamente desenhada para não repetir o
erro: `excluir_tabela_nao_deixa_arquivo_nenhum_para_tras`, que **não confere a
lista contra ela mesma** (isso passaria com qualquer número) — confere o
**diretório**, depois de um `excluir_tabela`, por arquivos que sobraram com o
prefixo da tabela.

Essa guarda já existia, já rodava, e a suíte inteira estava verde com `.fts`
faltando na lista.

## 2. O que eu concluí primeiro, e estava errado

Ao ler o comentário da guarda (*"confere o DIRETÓRIO, não a lista"*), concluí
que ela já cobria qualquer extensão nova por construção — afinal, ela não lê
`EXTENSOES_TODAS` para decidir o que checar, ela varre o disco de verdade.
Se um `.fts` tivesse ficado para trás depois de um `excluir_tabela`, o teste
teria pego.

Estava errado. O teste **cria** os arquivos que vai conferir: escreve
`pedidos.lgpd` a mão (porque o esquema de duas colunas do teste não teria
gerado um sozinho), chama `excluir_tabela`, e olha o que sobrou com o prefixo
`pedidos.`. Um `.fts` nunca apareceu no `read_dir` de ANTES nem de DEPOIS,
porque ninguém o criou — a varredura do diretório é honesta sobre o que
existe, mas o que existe é decidido pelo `setup` do teste, e o `setup` só
sabia das extensões que a lista de ontem conhecia. A guarda provava "nada
sobra do que eu botei lá", não "nada sobra do que a tabela pode ter".

## 3. O que a medição disse

Reproduzido com o defeito reposto (removendo `"fts"` de `EXTENSOES_TODAS` e
rodando a suíte de `phxsql-store` como estava ANTES de eu acrescentar
`std::fs::write(base.join("loja/pedidos.fts"), ...)` ao setup do teste): a
suíte inteira — as **162** provas de `phxsql-store` — passava verde, `.fts`
ausente da lista e tudo. Só depois de eu **acrescentar** a criação manual do
`.fts` no setup (a mesma técnica que já existia para o `.lgpd`) é que o teste
passou a acusar: `excluir_tabela deixou para tras: ["pedidos.fts"]`.

## 4. A regra

Uma guarda que "confere o diretório, não a lista" só protege as extensões que
o **setup do próprio teste** manufatura antes de conferir. Extensão nova
entrando no motor não vira invisível para o `read_dir` — vira invisível para o
teste, porque ninguém lembrou de criar um arquivo de mentira daquela extensão
no `setup`. Toda vez que uma extensão nova entrar no motor (`.fts`, e a
próxima que vier), o `setup` de `excluir_tabela_nao_deixa_arquivo_nenhum_
para_tras` e de `renomear_move_a_tabela_inteira` precisa ganhar uma linha
fabricando um arquivo dela — do mesmo jeito que já faz para `.lgpd`.

## 5. Como está guardado hoje

`excluir_tabela_nao_deixa_arquivo_nenhum_para_tras` e
`renomear_move_a_tabela_inteira` (em `phxsql-store/src/catalogo.rs`) agora
fabricam um `pedidos.fts` de mentira, ao lado do `pedidos.lgpd` que já
fabricavam — e falham com o defeito reposto (confirmado). Um novo teste,
`arquivos_da_tabela_enxerga_o_fts`, cobre o terceiro sintoma (o método que a
TELA lê). E a lacuna de fundo — três cópias escritas à mão sem sair do
código — ganhou uma guarda própria e permanente:
`crates/phxsql-server/src/conferidor_inventario.rs` (catraca
`TETO_INVENTARIO_DESCASADO`, `docs/CATRACAS.md` §8), que lê
`Database::extensoes_de_uma_tabela()` e confere as três cópias contra ela a
cada corrida — em vez de depender de alguém lembrar de fabricar mais um
arquivo de mentira no `setup` da próxima extensão.
