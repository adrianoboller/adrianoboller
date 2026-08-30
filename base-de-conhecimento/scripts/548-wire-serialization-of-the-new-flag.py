# Wire serialization of the new flag
# 28/08 17:23

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()

velho = '''        let (tag, coluna) = p.modo.tag();
        out.push(tag);
        out.extend_from_slice(&coluna.to_le_bytes());
        out
    }'''
novo = '''        let (tag, coluna) = p.modo.tag();
        out.push(tag);
        out.extend_from_slice(&coluna.to_le_bytes());
        // v4: exigir motivo escrito na exclusao. Vem no fim porque quem le uma
        // v3 simplesmente para antes daqui.
        out.push(self.motivo_obrigatorio as u8);
        out
    }'''
assert velho in s
s = s.replace(velho, novo, 1)

velho2 = '''        if versao >= 3 {
            paginacao.modo = ModoParticao::de_tag(leitor.u8()?, leitor.u16()?)?;
        }

        Schema::new(nome, colunas, indices)?
            .com_chaves_estrangeiras(fks)
            .map(|e| e.com_paginacao_do_disco(paginacao))
    }'''
novo2 = '''        if versao >= 3 {
            paginacao.modo = ModoParticao::de_tag(leitor.u8()?, leitor.u16()?)?;
        }
        let motivo_obrigatorio = versao >= 4 && leitor.u8()? != 0;

        // `do_disco`, e nao `new`: a lista de colunas gravada e a verdade
        // inteira. Ver a nota em `VERSAO_ESQUEMA`.
        Schema::do_disco(nome, colunas, indices)?
            .com_chaves_estrangeiras(fks)
            .map(|e| e.com_paginacao_do_disco(paginacao))
            .map(|e| e.com_motivo_obrigatorio(motivo_obrigatorio))
    }'''
assert velho2 in s
s = s.replace(velho2, novo2, 1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
