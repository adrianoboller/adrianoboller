# Acrescenta a fase conferir ao carga.rs
# 01/09 18:20

from pathlib import Path
p = Path("crates/phxsql-store/examples/carga.rs")
s = p.read_text(encoding="utf-8")

alvo = '''        outra => {
            eprintln!("fase desconhecida: {outra}");'''

novo = '''        "conferir" => {
            // Nao mede tempo: mede TRABALHO FEITO. As outras fases dizem
            // quanto demorou; esta diz se os motores chegaram ao MESMO
            // estado. Sem ela, «PhxSql inseriu em 8 s e o MySQL(R) em 30»
            // continua sendo uma frase sobre dois trabalhos que ninguem
            // conferiu serem o mesmo.
            //
            // Sao tres totais, e nao um: a contagem pega linha faltando, a
            // soma de `valor` pega o `atualizar` que nao atualizou, e a de
            // `cadastro` pega dado DIFERENTE gravado com o mesmo tamanho --
            // que foi o defeito achado na bancada do MySQL(R), onde toda
            // linha levava a data 2024-10-04 enquanto os outros dois lados
            // gravavam o dia variavel.
            let mut t = Table::abrir(&dir, "precos")?;
            let total = t.slots();
            let mut linhas = 0u64;
            let mut soma_valor: i128 = 0;
            let mut soma_cadastro: i128 = 0;
            for r in 1..=total {
                if let Some(l) = t.ler(r)? {
                    linhas += 1;
                    if let Some(Value::Decimal(v)) = l.get(3) {
                        soma_valor += *v;
                    }
                    if let Some(Value::Date(d)) = l.get(4) {
                        soma_cadastro += *d as i128;
                    }
                }
            }
            feitas = linhas;
            println!(
                "CONFERE {{\\"linhas\\":{linhas},\\"soma_valor\\":{soma_valor},\\
                 \\"soma_cadastro\\":{soma_cadastro}}}"
            );
        }
        outra => {
            eprintln!("fase desconhecida: {outra}");'''

assert s.count(alvo) == 1, "ancora nao unica"
s = s.replace(alvo, novo)
s = s.replace(
    "//! Fases: `criar`, `inserir`, `buscar`, `varrer`, `atualizar`, `excluir`.",
    "//! Fases: `criar`, `inserir`, `buscar`, `varrer`, `atualizar`, `excluir`,\n"
    "//! e `conferir`, que nao mede tempo -- mede se os motores chegaram ao\n"
    "//! mesmo estado, que e o que faz o tempo querer dizer alguma coisa.",
)
p.write_text(s, encoding="utf-8")
print("carga.rs: fase conferir acrescentada")
