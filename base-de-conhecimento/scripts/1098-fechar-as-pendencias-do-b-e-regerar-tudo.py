# Fechar as pendencias do B e regerar tudo
# 29/08 07:48

import io,re
p='docs/PENDENCIAS.md'
s=io.open(p,encoding='utf-8').read()
linhas=s.split('\n')
novos={
 101:('☑️','**Cifrar e compactar `.log`, `.trash` e `.reason`** | **cifra ligada; compactação medida duas vezes e recusada duas vezes.** ChaCha20-Poly1305 (RFC 8439, vetores oficiais) nos três diários, **desligada por padrão** — com o defeito «cifra imposta» reposto, 43 testes antigos quebram. Nonce do offset que o arquivo já tem, chave por PBKDF2 e por volume, replicação continuando com imagens decifradas pela sessão. E o corte do diário virou `recursos.diario_volume_mib`: remedido, compactar poupa 14,7% no melhor caso contra 2,1× mais que o `.ndx` daria — a recusa ficou com dois números em vez de um'),
 86:('◐','**Depois testar com PostgreSQL(R) e outros** | **cliente, dialeto e ligação prontos** — SCRAM contra o vetor do RFC 7677, SQL por motor, e as cinco operações do DbLink reescritas para não saberem qual motor atendem (o `servidor.rs` delega). Provado por soquete contra um servidor de protocolo próprio, byte a byte, nos dois sentidos. **O que falta é só o que o nome do pedido diz**: a prova contra um PostgreSQL(R) de verdade, que não existe nesta máquina — o que ela exige está em `docs/DBLINK.md`'),
}
for i,l in enumerate(linhas):
    m=re.match(r'^\| (☑️|◐|☐) \| (\d+) \|', l)
    if m and int(m.group(2)) in novos:
        e,t=novos[int(m.group(2))]
        linhas[i]=f'| {e} | {m.group(2)} | {t} |'
s='\n'.join(linhas)
feitos=sum(1 for l in linhas if l.startswith('| ☑️ |')); parc=sum(1 for l in linhas if l.startswith('| ◐ |')); plan=sum(1 for l in linhas if l.startswith('| ☐ |'))
s=re.sub(r'\*\*\d+ feitos · \d+ parciais · \d+ planejados\*\*, de \d+ pedidos\.',
         f'**{feitos} feitos · {parc} parciais · {plan} planejados**, de {feitos+parc+plan} pedidos.', s)
io.open(p,'w',encoding='utf-8').write(s)
print(f'{feitos} feitos, {parc} parciais, {plan} planejados')
