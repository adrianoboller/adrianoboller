# Check rates on the second reading
# 28/08 14:29

s=open('sisprobe.mjs').read()
s=s.replace('console.log(JSON.stringify(a, null, 1).slice(0, 3000));','console.log("apertados:", JSON.stringify(a.resultado.apertados), "alertas:", JSON.stringify(a.resultado.alertas));')
s=s.replace('console.log("cpu:", JSON.stringify(b.resposta?.cpu ?? b.cpu));','const r=b.resultado; console.log("cpu:", JSON.stringify(r.cpu), "\\nrede eth0:", JSON.stringify(r.rede.find(x=>x.interface==="eth0")), "\\nio:", JSON.stringify(r.io[0]));')
s=s.replace('console.log("segundos:", (b.resposta ?? b).segundos_desde_a_ultima);','console.log("segundos:", r.segundos_desde_a_ultima);')
open('sisprobe.mjs','w').write(s)
