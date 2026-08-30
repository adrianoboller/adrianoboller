# Update README
# 27/08 19:55

s=open('README.md').read()
s = s.replace('''dependência externa (só a `std`), o que faz o projeto compilar offline.''',
              '''dependência externa (só a `std`), o que faz o projeto compilar offline.''')
s = s.replace('''O motor de armazenamento está completo e testado: **195 testes**,''',
              '''O motor de armazenamento está completo e testado: **214 testes**,''')
s = s.replace('''| Blacklist com bloqueio automático e gancho de firewall | pronto |''',
              '''| Blacklist com bloqueio automático e gancho de firewall | pronto |
| Centro de Controle — interface web embutida no `phxsqld` | pronto |''')
s = s.replace('''./target/release/phxsql tabelas /tmp/dados Z
```''','''./target/release/phxsql tabelas /tmp/dados Z
```

### O servidor e o Centro de Controle

```bash
./target/release/phxsqld --exemplo 1 > config.json
$EDITOR config.json          # troque o token e ligue "web"
./target/release/phxsqld --config config.json
```

Com `"web": { "ligado": true }` o próprio `phxsqld` passa a servir o Centro
de Controle em `http://127.0.0.1:5001` — a página está embutida no binário,
não há servidor web para instalar. A árvore mostra bancos, schemas e tabelas;
cada tabela abre em cinco abas (Estrutura, Conteúdo, Índices, Diário,
Integridade) e há três telas de administração (Usuários, Acessos, Bloqueios).

Em `127.0.0.1` e em `https://` o login usa desafio-resposta e a senha **não
sai da máquina de quem entra**. Fora de contexto seguro o navegador não
oferece a cifra: a página cai em Base64 e diz isso na tela. Detalhes na
seção 9 do [`MANUAL.txt`](MANUAL.txt).''')
open('README.md','w').write(s)
