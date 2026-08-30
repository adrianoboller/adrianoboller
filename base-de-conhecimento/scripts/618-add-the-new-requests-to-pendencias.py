# Add the new requests to PENDENCIAS
# 28/08 18:06

import io
p='docs/PENDENCIAS.md'
s=io.open(p,encoding='utf-8').read()
alvo='''| ◐ | 95 | **Integrar o MULTILINK no DbLink** |'''
i=s.index(alvo)
fim=s.index('\n', s.index('\n', i)+0)
linha_fim = s.index('\n', i)
novo = '''
| ☑️ | 96 | **Registro apagado fisicamente vai para o `.trash` antes de sair do `.reg`** | e o disco **confirma** antes de o slot ser liberado. Guarda o *payload* byte a byte **mais o conteúdo dos anexos** — com ponteiro, a foto voltaria sendo a de outra linha, porque o bloco do `.bin` é liberado na exclusão. Só quem tem `administrar` lê |
| ☑️ | 97 | **Coluna `SOFTDELETED` em todas as tabelas** | entra sozinha na criação, no fim da lista para não deslocar as colunas do usuário. Marcar tira a linha das listas e ela continua inteira no `.reg`; `restaurar` desfaz. Esquema `PSCH` v3 → v4, e tabela v3 continua abrindo |
| ☑️ | 98 | **`.reason` com UUID, data, hora, motivo e quem excluiu** | UUID v7 do próprio evento, e a identidade da linha em texto — «rowid 4173» não diz nada seis meses depois. Sobrevive à linha: o expurgo é registrado antes de o dado sair. Só `administrar` |
| ☑️ | 99 | **Motivo de exclusão obrigatório, marcado na criação da tabela** | caixa na tela de Nova tabela; marcada, o motor recusa qualquer exclusão sem frase escrita, **antes** de qualquer gravação |
| ☑️ | 100 | **Botões e combos no ambiente** | diálogo de exclusão com os dois modos e o campo do motivo (não um `confirm()`, que só sabe perguntar sim ou não); par «ativas / excluídas» na grade com botão de restaurar; telas de Lixeira e de Motivos no menu Tabelas e na barra |
| ◐ | 101 | **Cifrar e compactar `.log`, `.trash` e `.reason`** | **não feito, e o motivo é técnico**: compactar arquivo *append-only* exige rotacionar e reescrever, e cifrar exige uma cifra de bloco que o projeto não tem — há SHA-256, HMAC e PBKDF2 escritos aqui, nenhum AES. Hoje a proteção é a permissão: as três operações exigem `administrar`, e no disco vale a permissão do sistema de arquivos |'''
s = s[:linha_fim] + novo + s[linha_fim:]
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
