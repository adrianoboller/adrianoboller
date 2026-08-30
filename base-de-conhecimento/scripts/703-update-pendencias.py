# Update PENDENCIAS
# 28/08 19:08

import io
p='docs/PENDENCIAS.md'
s=io.open(p,encoding='utf-8').read()
alvo='''| ◐ | 101 | **Cifrar e compactar `.log`, `.trash` e `.reason`** |'''
i=s.index(alvo)
fim=s.index('\n', i)
novo = '''
| ☑️ | 102 | **Paginação de Big Table por cursor (keyset)** | `depois`/`antes` no `varrer`, cursor bidirecional na grade, `pular` como compatibilidade. E o defeito que estava embaixo: o `varrer` lia a **tabela inteira com os anexos** para devolver 200 linhas — 3.176 ms numa tabela de 800 mil. Pelo cursor, não mensurável |
| ☑️ | 103 | **Campo `rownum` sequencial e automático em todas as tabelas** | coluna de sistema, o motor preenche, nunca reaproveita número, alterar não renumera. `rowid_do_rownum` acha por bissecção — 20 leituras num milhão, sem índice |
| ☑️ | 104 | **Partição alfanumérica: `Clientes_A.reg` … `Clientes_Outros.reg`** | 37 volumes fixos, o rowid sai de `(balde−1) × rpa + slot` — a inversa exata da conta de sempre, então nenhum caminho de leitura mudou. A ordem de digitação sai do rowid e vai para o `rownum` |
| ☑️ | 105 | **Arquivo `.pag` com a instrução da partição em JSON** | descritor **gerado**, com a conta do endereço por extenso; o motor nunca o lê. Segunda cópia seria segunda verdade |
| ◐ | 106 | **Integrar o MULTILINK — segunda análise, agora com os fontes** | o motivo anterior caiu: os fontes vieram. O novo é maior e medido: o `Cargo.lock` resolve **596 pacotes, 14 locais → 582 crates externas**, e cinco são obrigatórias mesmo sem nenhuma *feature* (`serde`, `serde_json`, `log`, `tokio`, `ml-driver-api`). Linkar traria um runtime assíncrono inteiro para dentro do `phxsqld`. Há um caminho novo que os fontes abrem: os `ml-driver-*-ffi` são `cdylib` com ABI C limpa, e ABI C se chama da `std` sem crate nenhuma — mas põe código proprietário com licença por máquina dentro do processo do banco. O caminho recomendado continua sendo **por protocolo**, agora como executável separado; `docs/MULTILINK.md` |
| ☐ | 107 | **Salto para uma página específica** | o cursor sabe ir e voltar; ir direto para «a página 500» exigiria contar a tabela, que é o que foi removido. Quem precisa de ponto certo usa `rownum` com a bissecção |'''
s = s[:fim] + novo + s[fim:]
io.open(p,'w',encoding='utf-8').write(s)
