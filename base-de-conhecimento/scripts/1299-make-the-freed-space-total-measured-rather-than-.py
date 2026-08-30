# Make the freed-space total measured rather than summed
# 30/08 15:59

p='zelador.sh'
s=open(p,encoding='utf-8').read()
# O total somado a mao ficava so com a parte do bash e ignorava a do Python --
# disse 362 MiB numa corrida que liberou quase 10 GB. O numero honesto e a
# diferenca medida do proprio disco.
velho='''echo "== antes: $(df -h "$REPO" | awk 'NR==2{print $4}') livres"'''
novo='''LIVRE_ANTES=$(df -k "$REPO" | awk 'NR==2{print $4}')
echo "== antes: $(df -h "$REPO" | awk 'NR==2{print $4}') livres"'''
assert s.count(velho)==1
s=s.replace(velho,novo)
velho2='''echo "== depois: $(df -h "$REPO" | awk 'NR==2{print $4}') livres"
echo "== liberou $((LIBEROU/1024)) MiB"'''
novo2='''LIVRE_DEPOIS=$(df -k "$REPO" | awk 'NR==2{print $4}')
echo "== depois: $(df -h "$REPO" | awk 'NR==2{print $4}') livres"
# Medido no disco, e nao somado das partes: a soma a mao ja disse 362 MiB numa
# corrida que liberou quase 10 GB, porque nao enxergava o que o Python apagou.
echo "== liberou $(( (LIVRE_DEPOIS - LIVRE_ANTES) / 1024 )) MiB, medidos no disco"'''
assert s.count(velho2)==1
open(p,'w',encoding='utf-8').write(s.replace(velho2,novo2))
print("total passa a sair da diferenca medida no disco")
