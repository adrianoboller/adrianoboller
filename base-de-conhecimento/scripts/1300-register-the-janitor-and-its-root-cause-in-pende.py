# Register the janitor and its root cause in PENDENCIAS
# 30/08 15:59

p='docs/PENDENCIAS.md'
ls=open(p,encoding='utf-8').read().split('\n')
linha = ('| ☑️ | 149 | **Um zelador que mantenha espaço em disco** | `phxsql/zelador.sh`. '
 'A regra que decide se ele ajuda ou destrói é uma só: **nada é apagado sem antes se provar que nenhum '
 'processo vivo está usando aquilo** — um zelador que apaga o `target` de quem está compilando não '
 'economiza espaço, perde uma rodada de trabalho. Cada worktree é conferida por processo com `cwd` '
 'dentro dela, e não por data ou nome. Ele **não mata processo nenhum** (matar o `phxsqld` de um agente '
 'vizinho já derrubou a própria sessão aqui), não apaga fonte, e não apaga o pacote da versão corrente. '
 'A primeira corrida achou o que vinha estrangulando o ambiente a sessão inteira: **80.088 diretórios '
 'de teste soltos em `/tmp`, 6,4 GB** — e o disco foi de 6,4 GB para 19 GB livres. Dois critérios '
 'guardam o que pode estar em uso, e erram para o lado seguro: PID vivo no nome, ou mexido nos últimos '
 '30 minutos (1.439 preservados). Duas lições ficaram no script: conferir 80 mil diretórios chamando '
 '`/proc` um a um estourou o tempo — a lista de processos se levanta **uma** vez; e o total somado das '
 'partes disse **362 MiB** numa corrida que liberou quase 10 GB, então ele passou a sair da diferença '
 'medida no próprio disco |')
raiz = ('| ⏳ | 150 | **A bateria não limpa o que cria** | é a causa-raiz do pedido 149, e enquanto ela '
 'ficar o zelador trata sintoma para sempre. São 80.088 diretórios `/tmp/phxsql-*` deixados para trás, '
 'o mais antigo de três dias. O padrão certo é o diretório temporário morrer com o teste que o criou '
 '(um guarda que apaga no `Drop`, e não um `rm` no fim do corpo — teste que falha no meio nunca chega '
 'ao fim). Vale medir antes quantos testes usam o padrão e quantos não: **medir a premissa do item vem '
 'antes de implementar o item** |')
i = 166
ls.insert(i, linha)
ls.insert(i+1, raiz)
open(p,'w',encoding='utf-8').write('\n'.join(ls))
print("pedidos 149 (zelador) e 150 (a causa-raiz) registrados")
