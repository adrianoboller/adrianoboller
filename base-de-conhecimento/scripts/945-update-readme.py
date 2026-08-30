# Update README
# 29/08 00:36

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
s = s.replace('''Gravar mil linhas com mil pedidos custa mil aberturas de tabela, mil travas e
mil `fsync`. `inserir_lote` faz tudo uma vez só — **2.715 → 25.985 linhas/s
(9,6×)**, medido com 20.000 linhas pela rede.''',
'''Gravar mil linhas com mil pedidos custa mil aberturas de tabela, mil travas e
mil `fsync`. `inserir_lote` faz tudo uma vez só — **2.609 → 37.021 linhas/s
(14,2×)**, medido com 20.000 linhas pela rede por
[`bancada/carga/medir.py`](bancada/carga/medir.py).''', 1)

alvo = '''## Estado atual'''
novo = '''## Conflito de escrita: o segundo a salvar escolhe

Duas pessoas com a mesma ficha aberta terminavam com a segunda gravação
apagando o trabalho da primeira — sem erro, sem registro, sem ninguém perceber
até faltar o dado.

Cada slot do `.reg` guarda uma **versão**, que sobe a cada regravação. Quem lê
com `"com_versao": true` recebe a versão junto e a manda de volta no
`atualizar`; se ela não for mais a atual, o servidor recusa com o erro **3004
`CONFLITO`**. Conferir custa 24 bytes — o cabeçalho do slot, não a linha.

Na tela, o conflito abre as **três colunas** do HFSQL(R) — «valor anterior», «o
outro escreveu», «você escreve» — e vai um passo além: **já vem marcado quem
mexeu em cada coluna**. Dois que editaram campos diferentes da mesma linha saem
dali com os dois trabalhos preservados, sem escolher nada.

Não é trava: travar na leitura prenderia a linha toda vez que alguém fechasse o
navegador com a ficha aberta. E a conferência é **pedida, não imposta** — quem
não manda a versão continua com a última gravação vencendo, como antes.

## Direito até a tabela, e não só até a base

A folha de pagamento e a tabela de clientes moram no mesmo banco porque o
negócio é um só, e o direito de ler as duas não é o mesmo:

```json
"bases": {
  "Z": { "ler": true, "inserir": true,
         "tabelas": { "folha": {} } }
}
```

A regra da tabela **substitui** a da base ali — o que permite tanto tirar
`folha` de quem lê o banco inteiro quanto dar `clientes` a quem não lê o banco
nenhum. A árvore e o catálogo passam a listar só o que dá para abrir.

## Estado atual'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
