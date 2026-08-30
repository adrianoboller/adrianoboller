# Descrever as operacoes no catalogo
# 29/08 11:39

import io
p='crates/phxsql-server/src/catalogo.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        exemplo: r#"{"op":"dblink_consultar","dblink":"erp","sql":"SELECT 1","limite":10}"#,
        ferramenta_mcp: false,
    },
];'''
novo='''        exemplo: r#"{"op":"dblink_consultar","dblink":"erp","sql":"SELECT 1","limite":10}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_ligar",
        apelidos: &[],
        resumo: "Liga tabelas primas: cria a tabela local espelhando a remota e \\
                 registra a sincronia na ligação (é o passo 4 do assistente).",
        parametros: &[
            obr("dblink", "string", "o nome da ligação"),
            obr(
                "tabelas",
                "lista",
                "objetos {remota, local_database, local_tabela?, sentido?, dono?}; \\
                 sentido: puxar|empurrar|dois (padrão dois); dono: aqui|la (padrão aqui)",
            ),
        ],
        exemplo: r#"{"op":"dblink_ligar","dblink":"erp","tabelas":[{"remota":"clientes","local_database":"loja","sentido":"dois","dono":"aqui"}]}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_sincronizar",
        apelidos: &[],
        resumo: "Uma rodada de convergência das tabelas ligadas: puxa o que falta \\
                 aqui, empurra o que falta lá, e o dono vence o conflito. Exclusão \\
                 não viaja. É a operação que o job agenda.",
        parametros: &[
            obr("dblink", "string", "o nome da ligação"),
            opc("tabela", "string", "sincroniza só esta (nome remoto ou local)"),
        ],
        exemplo: r#"{"op":"dblink_sincronizar","dblink":"erp"}"#,
        ferramenta_mcp: false,
    },
];'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('catalogo ok')
