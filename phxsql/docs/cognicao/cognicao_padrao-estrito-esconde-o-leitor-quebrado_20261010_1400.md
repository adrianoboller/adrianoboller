# O padrão estrito esconde o leitor quebrado

**Estado:** PENDENTE

## O que aconteceu

Pedidos 765/767, fatia P12: a camada de proteção passou a ler `phxsys.protecao`
para saber o modo de cada comando. Qualquer falha de leitura vale `proteger` —
o lado estrito, de propósito. O `julgar` é chamado de dois lugares: sem a
trava (o pedido) e **com a trava na mão** (o plano da cascata, em
`alterar_solto` e `planejar_cascata_empilhada`). Pelo `varrer`, a leitura
dentro da trava pediria a trava de novo e voltaria `trava_reentrante` (lido
no código: `travar_dados_para_ler` confere `COM_A_TRAVA` antes de tudo) — que
o leitor converteria, calado, em `proteger`. Medido não pelo caminho do
`varrer`, e sim pela troca equivalente da guarda abaixo (a leitura sob a
trava devolvendo erro).

## O que eu concluí primeiro, e estava errado

Que «falha vale proteger» bastava como garantia: se a leitura quebrar, o
comando continua recusado, então não há defeito a temer. Errado pelo outro
lado: com a leitura **sempre** quebrada dentro da trava, a linha da cascata
nunca vale — o dono baixa `cascata` para `observar` e nada muda — e **nenhum
teste de recusa acusa**, porque recusar é exatamente o que o padrão faz.
Defeito que tem a cara do comportamento correto.

## O que a medição disse

Só o teste que exige o lado PERMISSIVO pega: com
`protecao.modo_dispensa_a_senha` e a linha `cascata` em `observar`, a troca
da chave da mãe com 1.000 filhas tem de **executar** sem a senha. Passando a
ficha (`Some(&Instancia)`) ao `julgar`, executa; com a leitura sob a trava
trocada por erro (guarda `tabela-da-protecao-cega-sob-a-trava`), recusa — e
o teste cai.

Junto, da mesma fatia: semear a tabela em todo arranque derrubou **9** testes
do comportamento velho (listas de bases e de tabelas, `phxsys nao pode nascer
sem alguem pedir`). Como a ausência já vale `proteger`, a tabela passou a ser
criada pelo `protecao_semear` e só completada no arranque onde `phxsys` existe.

## A regra

Quando a falha cai num padrão seguro, prove o leitor pelo lado em que ele
**libera** — o teste do lado estrito passa com o leitor morto.

## Como está guardado hoje

`servidor::testes_da_guarda_da_protecao::com_a_dispensa_observar_executa_sem_a_senha_e_vai_a_trilha`
e a guarda `tabela-da-protecao-cega-sob-a-trava` do `bancada/guardas/catalogo.py`.
O buraco que fica: o mesmo padrão (falha → estrito, calado) vale para a
`Modos::de_varredura` inteira; uma tabela recriada com outro nome de coluna
lê tudo como `proteger` sem aviso nenhum.
