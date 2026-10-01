# Backup provado por restauração

**Regra.** Backup só vale **restaurado e comparado**. Não é «o pacote foi
gerado sem erro» nem «a ferramenta de verificação disse ok»: é extrair num
diretório de prova e comparar com a origem — o objeto árvore do git dos dois
lados, ou o SHA-256 de cada arquivo. E o pacote sai **de script, nunca montado
à mão**: pacote feito à mão é pacote que ninguém consegue refazer igual.

**Cicatriz.** Medido: cortaram-se 2 MiB do fim de um `git bundle` bom.
`git bundle verify` respondeu «The bundle records a complete history» e saiu
**0**; `git clone` dele morreu com `index-pack died` e saiu **128**. O
`verify` lê o cabeçalho, não o packfile. Quem só roda o verify entrega backup
podre com a consciência limpa — e backup só reprova na hora de restaurar, que
é a pior hora possível.

**E o que ele não leva, ele diz.** Pacote completo que deixa de fora
compilado, arquivo grande ou um arquivo que sumiu do ambiente imprime cada
ausência **nomeada**, sob um cabeçalho que não é linha de êxito. Foi assim que
se descobriu que a lei global morava só no disco de um contêiner efêmero.

**Como aplicar.** `scripts/backup.sh` (histórico, provado por `git clone` +
árvore igual) e `scripts/backup-completo.sh` (árvore inteira + arquivos fora
do repositório, provado por SHA-256 arquivo a arquivo).
