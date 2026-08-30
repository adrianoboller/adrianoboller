# A prova do `acrescentar_coluna`, pelo soquete

```bash
cargo build --release -p phxsql-server --bin phxsqld
python3 bancada/alter/provar.py
```

Sobe **dois** `phxsqld` de verdade — source na porta 7150, réplica na 7152 —,
monta uma tabela com dado, acrescenta uma coluna e só então pergunta se tudo
continua funcionando. Mata só os PIDs que ela mesma subiu, pelo PID; nunca
`pkill -f`.

## O que só ela acha

Os dezessete testes de `crates/phxsql-store/tests/acrescentar-coluna.rs`
provam o **formato**: rowid preservado, ordem preservada, índice intocado,
espelho acompanhando, e a queda no meio da troca. Três coisas deste script
nenhum deles alcança:

1. **a operação existe pelo protocolo**, com o portão de permissão certo e as
   três recusas chegando como erro de esquema (obrigatória sem padrão, nome
   repetido, nome de coluna do motor);
2. **o backup** feito depois da alteração volta com a coluna nova **e com os
   mesmos rowids** — é o `backup` + `restaurar_backup` reais, com ZIP e
   manifesto;
3. **a réplica**. E a resposta não é óbvia, e é a parte que vale ler:

   - com o esquema mudado **de um lado só**, a réplica **para** de aplicar em
     vez de aceitar um payload de outra largura. O `conferir_payload` do
     `.reg` recusa pelo tamanho, e a réplica prefere parar a divergir — é o
     mesmo comportamento da thread SQL do MySQL(R) parando num erro;
   - assim que a **mesma** alteração chega à réplica, ela volta a andar
     **sozinha, do ponto em que parou**: o evento que ficou para trás é
     aplicado, e os dois lados terminam com o mesmo conjunto de rowids.

   `acrescentar_coluna` **não se replica**: é uma operação local, e quem
   administra o par tem de alterá-lo dos dois lados.

## As portas

7150 (source) e 7152 (réplica). A faixa 7150–7199 é desta frente; fora dela
não se encosta. O `provar.py` da raiz confere se as duas estão livres antes de
chamar, e **pula** a parte se alguma estiver ocupada — há outras frentes na
mesma máquina, e uma bateria que acusa a vizinha de defeito é pior que uma que
não roda.

## O que ela deixa para trás

Nada. `/tmp/phx-alter-prova` é recriado a cada rodada e os dois servidores
caem no `finally`, inclusive quando um passo falha.
