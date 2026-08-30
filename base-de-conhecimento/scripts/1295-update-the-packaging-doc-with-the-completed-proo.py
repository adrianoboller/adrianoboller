# Update the packaging doc with the completed proof
# 30/08 15:43

p='docs/EMPACOTAMENTO.md'
s=open(p,encoding='utf-8').read()
velho='| Linux ARM64 / ARMv7 | **compila estático, 6,8/6,7 MB** | rodar numa placa de verdade |'
novo='| Linux ARM64 / ARMv7 | **roda: gravou e leu 50 linhas sob emulação** | o desempenho real, que só a placa mede |'
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print("tabela resumo atualizada")
