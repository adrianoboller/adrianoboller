# Bancada da partição por quantidade (a "faixa" do pedido 07/09/2026)

Mede a partição `ModoParticao::PorQuantidade` (`docs/FORMATO.md` §8) — volumes
`Tabela_001.reg` … `Tabela_NNN.reg`, endereço por divisão
(`volume = (rowid-1)/registros_por_arquivo + 1`) — contra um `phxsqld` de pé,
e não lendo o código. Existe porque a alfanumérica (`bancada/alfanumerica/`)
já respondia pelo texto-chave, e faltava a irmã por contagem: pergunta do
dono, 07/09/2026, *«tabela particionada por faixa de qtde de registros
máximo, ex 1.000.000, funcionando como se fosse uma tabela única»*.

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/particao-por-faixa/sonda.py
```

Variáveis: `PHX_SONDA_PORTA` (padrão 6710), `PHX_SONDA_LINHAS` (padrão
1.000.000) e `PHX_SONDA_POR_VOLUME` (padrão 100.000 — dez volumes com o
padrão). A carga de 1.000.000 de linhas em lotes de 5.000 leva pouco mais de
15 segundos nesta máquina; se um dia passar de 5 minutos, reduza
`PHX_SONDA_LINHAS` para 300.000 e diga no relatório, como o pedido original
previu.

A sonda carrega a tabela inteira, mostra os dez volumes em disco, prova que
`varrer` atravessa a fronteira entre arquivos como se fosse uma tabela só
(rowids seguidos, uma chamada, um resultado), que `buscar` por índice acha a
linha certa em qualquer volume, e mede — com 300 repetições de `ler` — que uma
página do último volume custa o mesmo que uma do primeiro, porque o endereço é
uma divisão e não uma busca que anda pelos volumes anteriores. Termina
provando (e não supondo) que o `INSERT` dentro de uma transação é ACEITO aqui
— ao contrário da partição por letra, onde é recusado de propósito — porque o
rowid alvo continua sendo `slots()+1`, a mesma conta de uma tabela sem
partição nenhuma.
