# Resolve the LGPD screen conflict keeping the server-side scan
# 29/08 20:52

import io,re
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()
MARCA=">>>>>>> worktree-agent-a0bc9d47d803ae652"

def blocos(s):
    out=[]
    i=0
    while True:
        a=s.find("<<<<<<< HEAD",i)
        if a<0: break
        b=s.index("\n=======\n",a)
        c=s.index(MARCA,b)
        cfim=s.index("\n",c)+1
        out.append((a, s[a+len("<<<<<<< HEAD\n"):b+1], s[b+len("\n=======\n"):c], cfim))
        i=cfim
    return out

bs=blocos(s)
assert len(bs)==5, len(bs)

# 1) comentario: junta as duas metades -- a razao arquitetural DELES e a licao do HEAD
comentario = """ * A marca vive no esquema, por coluna, no campo `dado_pessoal`, que vale
 * `"nao"`, `"pessoal"` ou `"sensivel"` (LGPD art. 5o I e II). Esta tela so
 * AUDITA: mostra onde as marcas estao, por base e por tabela.
 *
 * QUEM VARRE E O SERVIDOR, pela op `dados_pessoais`, e nao um laco aqui. A op
 * nao tem campo `tabela`, entao ela confere o direito tabela a tabela POR
 * DENTRO, como o `juntar` e o `unir`. Varrer aqui com `tabelas` + `esquema`
 * refazia essa conferencia por fora, que e exatamente onde ela um dia deixa
 * de existir.
 *
 * # O defeito que esta tela teve, e que vale mais que o conserto
 *
 * Ela passou versoes lendo um campo booleano chamado `pessoal`, que o
 * servidor nunca devolveu -- o esquema responde `dado_pessoal`, em texto. O
 * motor gravava a marca (PSCH v6) e a tela relatava «0 colunas marcadas» numa
 * base com seis colunas classificadas: uma tela de conformidade respondendo
 * «nao sei» sobre um motor que sabe. Quem achou foi a bateria de frontend,
 * PERCORRENDO a tela -- ler o codigo dela nao acharia.
 *
 * O que fez o defeito sobreviver tanto tempo foi a HONESTIDADE dela: como nao
 * achava o campo, dizia «nao sei» em vez de «nenhum dado pessoal encontrado».
 * Isso a impediu de mentir, e e a razao de o aviso continuar aqui embaixo --
 * mas tambem fez o defeito parecer um servidor sem marcas em vez de uma tela
 * quebrada.
 *
 * A licao, em docs/LGPD.md: «configuracao que nao e lida mente» tem um lado
 * espelhado. Aqui o campo era lido pelo motor e ignorado pela tela, e o
 * estrago e o mesmo pelo outro lado -- quem marca acredita que marcou.
 *
 * E a regra que a tela nao pode quebrar: coluna SEM CLASSIFICACAO nao e
 * coluna sem dado pessoal. Uma diz «ninguem olhou ainda»; a outra diz
 * «alguem olhou e disse que nao e». O numero que separa as duas e
 * `colunas_sem_classificacao`, e ele fica em cima, entre os grandes.
"""

escolhas = [comentario, None, None, None, None]  # None = fica com o lado DELES
novo=[]
fim=0
for idx,(a,meu,deles,cfim) in enumerate(bs):
    novo.append(s[fim:a])
    if escolhas[idx] is not None:
        novo.append(escolhas[idx])
    else:
        novo.append(deles)
    fim=cfim
novo.append(s[fim:])
s="".join(novo)
assert "<<<<<<<" not in s and MARCA not in s

# a frase de "onde se marca" ficou velha do lado deles: a op de marcar existe agora
velho = """       <p>Esta tela <b>lê</b> a marca, e quem varre é o servidor — a op
       <code>dados_pessoais</code>, que confere o direito tabela a tabela por
       dentro. Marcar é no <code>criar_tabela</code>, no campo
       <code>dado_pessoal</code> de cada coluna.</p>"""
nova = """       <p>Esta tela <b>lê</b> a marca, e quem varre é o servidor — a op
       <code>dados_pessoais</code>, que confere o direito tabela a tabela por
       dentro. Marcar e desmarcar é na aba <b>Estrutura</b> da tabela, na
       coluna LGPD; clique numa linha acima para ir até lá.</p>"""
assert s.count(velho)==1
s=s.replace(velho,nova)
io.open(p,"w",encoding="utf-8").write(s)
print("index.html resolvido")
