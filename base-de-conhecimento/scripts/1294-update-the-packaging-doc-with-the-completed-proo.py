# Update the packaging doc with the completed proof
# 30/08 15:43

p='docs/EMPACOTAMENTO.md'
s=open(p,encoding='utf-8').read()
velho='''### 7.3 A fronteira honesta

**Os binários ARM não foram executados.** Esta máquina é x86 e não tem
emulador; o que se conferiu é que o arquivo é um ELF ARM estático válido
(`file` confirma arquitetura, ligação estática e ABI). Compilar não é rodar, e
a diferença tem de ficar escrita — quem tiver uma placa na mão fecha essa conta
em cinco minutos.

O que dá confiança apesar disso: o alvo `x86_64-unknown-linux-musl`, que usa o
**mesmo** caminho estático, é exercitado de verdade na bancada Docker.'''
novo='''### 7.3 A fronteira, que durou uma hora

A primeira versão desta seção dizia, com todas as letras, que **os binários ARM
não tinham sido executados** — esta máquina é x86 e não havia emulador. Estava
certo enquanto durou, e a distinção era o ponto: compilar não é rodar.

Durou até alguém perguntar se dava para testar. **Não era preciso VM.** Uma VM
completa exige `/dev/kvm`, que este ambiente não tem (é ele próprio uma máquina
virtual sem aninhamento). Mas o `qemu-user-static` emula o **binário**, não a
máquina, e por isso não depende de KVM nenhum:

```bash
sudo apt install qemu-user-static
bancada/arm/provar.sh
```

O que a bancada faz, e o resultado:

| Passo | Resultado |
|---|---|
| gerar o hash da senha **com o binário ARM** | `pbkdf2-sha256$210000$…` — o PBKDF2 roda em ARM |
| subir o `phxsqld` aarch64 sob emulação | no ar, **11,9 MiB** de RSS |
| `ping` com token | `ok` |
| `login` com a senha cujo hash o próprio ARM gerou | `ok` — a criptografia fecha dos dois lados |
| `criar_database` + `criar_tabela` | `ok` |
| inserir 50 linhas | **50 de 50**, 1,1 ms por linha *sob emulação* |
| `varrer` de volta | **50 registros, 50 devolvidas** |

A primeira linha lida de volta:

```json
{"rowid": 1, "sensor": "s0", "valor": "20.00", "softdeleted": false, "rownum": 1}
```

Então **«compila» virou «gravou e leu»**. O que continua sem prova é o
desempenho real: 1,1 ms por linha é o custo da *emulação*, não o de uma placa
— o `qemu-user` traduz instrução por instrução. Numa placa de verdade o número
é outro, e provavelmente melhor; **medir isso continua exigindo a placa.**

O RSS de 11,9 MiB também é sob emulação e inclui o custo do próprio `qemu`;
o número nativo medido em x86 é o da §7.2 — 4,9 MiB.'''
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print("secao 7.3 refeita com a prova")
