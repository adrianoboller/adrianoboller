<!-- GERADO por classificar.py a partir das cognições. Não se edita: mude a seção «## Estado» da cognição e rode o gerador. -->

# Evitar — falhas observadas, com causa e prevenção

294 cognições: **9 frutíferas**, **5 infrutíferas**, **280 pendentes** (sem evidência validada — não entram aqui).

Antes de repetir um caminho, procure aqui se ele já falhou e o que o previne.

## [O Wine finge gravar a ACL, e a sintetiza na leitura](cognicao_wine-finge-gravar-acl_20260924_1500.md)

- **Causa:** `SetNamedSecurityInfoW` sob o Wine lê a ACL e nunca a grava, devolvendo sucesso.
- **Prevenção:** usar `SetFileSecurityW` e provar pelo efeito, lendo de volta; a prova estrita fica no `prova-windows.ps1` (passo 3b), para Windows real.
- Evidência: commit `ed57c81`; `phxvpn/prova-windows.ps1`

## [Sondar cedo demais inventa defeito — e o Wine derruba serviços](cognicao_sondar-cedo-demais_20260924_1620.md)

- **Causa:** sondas com prazo fixo escolhido de cabeça (6 s, 4 s) mediram antes de o serviço subir e inventaram defeito; sob o Wine, o serviço ainda morre quando o último processo de usuário sai.
- **Prevenção:** esperar por condição, com teto generoso, e imprimir em quanto tempo valeu; sob o Wine, manter um processo de usuário vivo durante a prova.
- Evidência: commit `3adbdb9`

## [A sonda que chega cedo envenena o NAT que a recebe](cognicao_sonda-que-chega-cedo-envenena-o-nat_20260924_0405.md)

- **Causa:** NAT Linux sem filtro de entrada aceita a primeira sonda do outro lado; o conntrack ocupa a tupla de volta e o NAT troca a porta, virando simétrico para aquele destino.
- **Prevenção:** provar a perfuração com e sem o `DROP` de `NEW` na wan e ler o conntrack; nesse caso o túnel fica no repasse (TTL curto por sonda segue pendente).
- Evidência: `phxvpn/provas/perfuracao/resultados.json`; commit `d0b2685`

## [RED que falha pelo motivo errado: o binário velho que o `nobody` não executa](cognicao_red-que-falha-pelo-motivo-errado_20260924_1125.md)

- **Causa:** o `tls-crypt-v2-verify` executa o `phxvpn` como o usuário sem privilégio do OpenVPN, e o binário do RED estava numa pasta 0700; e duas esperas em sequência mediam o segundo tempo do fim da primeira.
- **Prevenção:** binário de RED numa pasta 0755 (`/tmp/phxvpn-velho-bin/`), conferir no log que o RED chegou ao ponto medido, e medir os tempos paralelos de um laço só a partir do mesmo instante.
- Evidência: `phxvpn/provas/ciclo-openvpn/resultados.json`; `phxvpn/provas/ciclo-openvpn/rodar.sh`

## [`grep -c` sem casamento mata a prova — justamente no caso bom](cognicao_grep-c-sem-casamento-mata-a-prova-no-caso-bom_20260924_0406.md)

- **Causa:** `grep -c` sem casamento sai com 1; sob `set -e` e `pipefail` isso mata o roteiro justo no caso bom, sem dizer onde.
- **Prevenção:** todo `grep` cujo «não achei» é o resultado bom leva `|| true` dentro do cano.
- Evidência: `phxvpn/provas/mfa/rodar.sh`; commit `0e5352c`

