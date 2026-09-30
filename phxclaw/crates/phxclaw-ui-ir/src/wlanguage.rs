//! Backend WLanguage das regras de negocio, para rodar no WinDev/WebDev enquanto o
//! destino final e o Rust (`rust.rs`). Os dois leem `regras::de`: mesmos nomes de
//! procedimento, mesmas mensagens.
//!
//! ESTADO: UNVERIFIED. Nao ha WinDev neste ambiente; o texto usa so o nucleo da
//! linguagem (PROCEDURE, IF/THEN/END, WHILE, RESULT) e as funcoes HFSQL de leitura por
//! chave (HReadSeekFirst, HReadNext, HFound, HAdd, HDelete, HErrorInfo). A prova e
//! compilar no WinDev e rodar os mesmos casos do teste do crate Rust.
//!
//! As telas nao se geram aqui: o RAD do WinDev as monta da analise, que se importa do
//! mesmo SQL. O que o RAD nao sabe e a regra -- e isso que este arquivo traz.

use crate::ir::App;
use crate::regras::{self, Campo, Entidade, Tipo};

/// Literal WLanguage: aspas dobradas dentro da cadeia.
fn lit(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub fn render(app: &App) -> Vec<(String, String)> {
    let ents = regras::de(app);
    vec![
        ("Regras.wl".into(), procedimentos(app, &ents)),
        ("LEIA-ME.md".into(), leia_me(app, &ents)),
    ]
}

fn fk_obrigatoria(c: &Campo) -> bool {
    c.obrigatorio && !c.somente_leitura
}

fn procedimentos(app: &App, ents: &[Entidade]) -> String {
    let ent = |n: &str| ents.iter().find(|e| e.nome == n);
    let mut s = format!(
        "// GERADO do UI-IR v{} pelo PhxClaw: regras de negocio de {}.\n\
         // UNVERIFIED: nao compilado aqui. Espelho do crate Rust gerado do mesmo modelo.\n\
         // Cada procedimento devolve \"\" quando esta tudo certo, ou a mensagem do erro.\n\n",
        app.ir_version, app.name
    );
    for e in ents {
        let t = &e.nome;
        // Validar_<entidade>: le o registro corrente do arquivo
        s.push_str(&format!("PROCEDURE Validar_{t}()\n"));
        for c in &e.campos {
            if c.nome == e.chave || c.somente_leitura {
                continue;
            }
            let f = format!("{t}.{}", c.nome);
            match c.tipo {
                Tipo::Texto => {
                    if c.exige_preenchimento() {
                        s.push_str(&format!(
                            "IF NoSpace({f}) = \"\" THEN RESULT {}\n",
                            lit(&regras::msg_obrigatorio(c))
                        ));
                    }
                    if let Some(n) = c.max_len {
                        s.push_str(&format!(
                            "IF Length({f}) > {n} THEN RESULT {}\n",
                            lit(&regras::msg_tamanho(c, n))
                        ));
                    }
                    if !c.opcoes.is_empty() {
                        let ou = c
                            .opcoes
                            .iter()
                            .map(|o| format!("{f} = {}", lit(o)))
                            .collect::<Vec<_>>()
                            .join(" OR ");
                        s.push_str(&format!(
                            "IF {f} <> \"\" AND NOT ({ou}) THEN RESULT {}\n",
                            lit(&regras::msg_opcao(c))
                        ));
                    }
                }
                Tipo::Data | Tipo::DataHora => {
                    // HFSQL guarda data como "AAAAMMDD" e data/hora como "AAAAMMDDHHMM..."
                    let invalida = if c.tipo == Tipo::Data {
                        format!("NOT DateValid({f})")
                    } else {
                        format!(
                            "NOT DateValid(Left({f}, 8)) OR Val(Middle({f}, 9, 2)) > 23 OR Val(Middle({f}, 11, 2)) > 59"
                        )
                    };
                    if c.exige_preenchimento() {
                        s.push_str(&format!(
                            "IF {f} = \"\" THEN RESULT {}\n",
                            lit(&regras::msg_obrigatorio(c))
                        ));
                        s.push_str(&format!(
                            "IF {invalida} THEN RESULT {}\n",
                            lit(&regras::msg_data(c))
                        ));
                    } else {
                        s.push_str(&format!(
                            "IF {f} <> \"\" AND ({invalida}) THEN RESULT {}\n",
                            lit(&regras::msg_data(c))
                        ));
                    }
                }
                _ => {}
            }
        }
        s.push_str("RESULT \"\"\n\n");

        // Incluir_<entidade>: valida, confere cada pai, grava
        s.push_str(&format!(
            "PROCEDURE Incluir_{t}()\nsErro is string = Validar_{t}()\nIF sErro <> \"\" THEN RESULT sErro\n"
        ));
        for c in &e.campos {
            if let Some((mae, chave_mae)) = &c.mae
                && let Some(m) = ent(mae)
            {
                let msg = lit(&regras::msg_sem_pai(c, m));
                let busca = format!(
                    "HReadSeekFirst({}, {chave_mae}, {t}.{})\nIF NOT HFound({}) THEN RESULT {msg}\n",
                    m.nome, c.nome, m.nome
                );
                if fk_obrigatoria(c) {
                    s.push_str(&busca);
                } else {
                    s.push_str(&format!("IF {t}.{} <> 0 THEN\n{busca}END\n", c.nome));
                }
            }
        }
        s.push_str(&format!(
            "IF NOT HAdd({t}) THEN RESULT HErrorInfo()\nRESULT \"\"\n\n"
        ));

        // Excluir_<entidade>: restringir, nunca cascata
        s.push_str(&format!("PROCEDURE Excluir_{t}(nCodigo)\n"));
        for (filha, fk) in &e.filhas {
            if let Some(f) = ent(filha) {
                s.push_str(&format!(
                    "HReadSeekFirst({filha}, {fk}, nCodigo)\nIF HFound({filha}) THEN RESULT {}\n",
                    lit(&regras::msg_tem_filhos(e, f))
                ));
            }
        }
        s.push_str(&format!(
            "HReadSeekFirst({t}, {}, nCodigo)\nIF NOT HFound({t}) THEN RESULT {}\nIF NOT HDelete({t}) THEN RESULT HErrorInfo()\nRESULT \"\"\n\n",
            e.chave,
            lit(&regras::msg_nao_encontrado(e))
        ));

        for tot in &e.totais {
            s.push_str(&format!(
                "// {}\nPROCEDURE Total_{t}_{}(nCodigo)\nnTotal is currency = 0\n\
                 HReadSeekFirst({f}, {fk}, nCodigo)\n\
                 WHILE HFound({f}) AND {f}.{fk} = nCodigo\n\
                 \tnTotal += {f}.{c}\n\
                 \tHReadNext({f}, {fk})\nEND\nRESULT nTotal\n\n",
                tot.rotulo,
                tot.campo,
                f = tot.filha,
                fk = tot.fk,
                c = tot.campo
            ));
        }
    }
    s
}

fn leia_me(app: &App, ents: &[Entidade]) -> String {
    let chaves: Vec<String> = ents
        .iter()
        .flat_map(|e| {
            e.filhas
                .iter()
                .map(|(f, fk)| format!("`{f}.{fk}`"))
                .chain(e.totais.iter().map(|t| format!("`{}.{}`", t.filha, t.fk)))
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    format!(
        "# Regras de {} em WLanguage\n\n\
         **Estado: UNVERIFIED** — gerado pelo PhxClaw sem WinDev para compilar. O crate Rust\n\
         gerado do mesmo modelo está compilado e testado; este arquivo é o espelho dele.\n\n\
         ## Como usar\n\n\
         1. Importe o mesmo SQL na análise (HFSQL).\n\
         2. Declare como **chave com duplicatas** os itens que as regras leem por chave: {}.\n\
         3. Cole `Regras.wl` numa coleção de procedimentos globais.\n\
         4. No botão Gravar da janela gerada pelo RAD: `sErro = Incluir_<arquivo>()` e mostre\n   `sErro` se não vier vazio. No Excluir: `Excluir_<arquivo>(codigo)`.\n\n\
         ## Diferença conhecida para o Rust\n\n\
         Chave repetida: o Rust devolve «código já existe»; aqui quem recusa é o `HAdd`, com\n\
         a mensagem do HFSQL (`HErrorInfo`). Conferir a chave antes leria o próprio arquivo e\n\
         trocaria o registro que está sendo gravado.\n",
        app.name,
        if chaves.is_empty() {
            "nenhum".into()
        } else {
            chaves.join(", ")
        }
    )
}
