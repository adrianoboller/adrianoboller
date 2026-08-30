# Record the object in the access log
# 28/08 16:24

p='crates/phxsql-server/src/acesso.rs'
s=open(p).read()
a='''    pub duracao_ms: u64,
    pub erro: Option<String>,
}'''
b='''    pub duracao_ms: u64,
    pub erro: Option<String>,
    /// Sobre QUAL objeto a operacao foi, quando ela nomeia um.
    ///
    /// O log dizia so o que foi feito, e nao em que. "varrer levou 4 s" sem o
    /// nome da tabela e quase inutil para quem opera: nao da para somar por
    /// tabela, nem para achar a que custa caro. Vazio quando a operacao nao
    /// fala de tabela nenhuma -- `ping`, `config`, `usuarios`.
    pub database: String,
    pub tabela: String,
    /// Codigo do erro, para agrupar por causa em vez de por texto.
    pub codigo: u16,
}'''
assert a in s; s=s.replace(a,b,1)

a='''            ("ms", Json::de_u64(self.duracao_ms)),
        ];
        if let Some(e) = &self.erro {
            pares.push(("erro", Json::texto_de(e)));
        }'''
b='''            ("ms", Json::de_u64(self.duracao_ms)),
        ];
        // So entram quando existem: o log e uma linha por acesso, e campo
        // vazio em toda linha e peso morto num arquivo que cresce sozinho.
        if !self.database.is_empty() {
            pares.push(("database", Json::texto_de(&self.database)));
        }
        if !self.tabela.is_empty() {
            pares.push(("tabela", Json::texto_de(&self.tabela)));
        }
        if let Some(e) = &self.erro {
            pares.push(("erro", Json::texto_de(e)));
            pares.push(("codigo", Json::de_u64(self.codigo as u64)));
        }'''
assert a in s; s=s.replace(a,b,1)

a='''            erro: j.campo("erro").and_then(Json::texto).map(str::to_string),
        })'''
b='''            erro: j.campo("erro").and_then(Json::texto).map(str::to_string),
            // Linha antiga nao tem estes campos, e ler o log antigo tem de
            // continuar funcionando: ausente vira vazio, nao erro.
            database: j.texto_ou("database", "").to_string(),
            tabela: j.texto_ou("tabela", "").to_string(),
            codigo: j.inteiro_ou("codigo", 0).clamp(0, 65_535) as u16,
        })'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
