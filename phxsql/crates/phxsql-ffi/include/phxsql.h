/* phxsql.h -- PhxSql embutido, a ABI de C.
 *
 * O motor de dados do PhxSql dentro do processo do seu aplicativo: sem porta,
 * sem daemon, sem localhost. E a forma que o Android e o iOS apoiam -- os dois
 * sistemas nao aceitam um servidor de longa duracao escutando porta, e por
 * isso a resposta para "banco no celular" e biblioteca, e nao mini servidor.
 *
 * Ligue contra:
 *     libphxsql_ffi.so    (Android, Linux)   -- cdylib
 *     libphxsql_ffi.a     (iOS, estatico)    -- staticlib
 *
 * O desenho e o porque de cada decisao estao em docs/EMBUTIDO.md.
 *
 * ---------------------------------------------------------------------------
 * AS SEIS REGRAS DESTA FRONTEIRA
 * ---------------------------------------------------------------------------
 *
 * 1. NENHUM PANICO ATRAVESSA. Toda funcao daqui e blindada. Um panico interno
 *    vira PHX_ERRO_PANICO e ENVENENA o punho: as chamadas seguintes nele sao
 *    recusadas com PHX_ERRO_ENVENENADO, porque capturar o panico salva o
 *    processo e nao conserta o objeto. So o "fechar" continua passando. O
 *    conserto e fechar e reabrir.
 *
 * 2. O ERRO VOLTA NO RETORNO. Zero e sucesso; 1 (PHX_NAO_HA) e "nao ha o que
 *    devolver" e NAO e erro; 1001..6001 sao os codigos do PhxSql, os MESMOS da
 *    porta de dados (3002 e chave duplicada nos dois); negativos sao problemas
 *    da fronteira. A mensagem fica numa vaga POR THREAD: phx_ultimo_erro.
 *
 * 3. QUEM ALOCOU, LIBERA. Esta biblioteca NUNCA devolve ponteiro para voce
 *    chamar free(): numa DLL do Windows os dois lados podem ter CRTs
 *    diferentes e isso derruba o processo. O que ela devolve e punho, com o
 *    phx_*_liberar correspondente. O resto vai em buffer SEU, com capacidade.
 *
 * 4. TEXTO E UTF-8 COM TAMANHO. Nunca NUL-terminado (a unica excecao e o
 *    phx_erro_nome, que devolve literal estatico). Motivo: dado de cliente tem
 *    byte zero -- um Bin e binario, um Memo colado de arquivo pode ter \0 no
 *    meio -- e strlen o truncaria em silencio. Use a macro PHX_T para
 *    literais.
 *
 * 5. UM PUNHO, UMA THREAD POR VEZ. Punhos diferentes em threads diferentes
 *    sobre TABELAS diferentes: pode, e ha teste. O mesmo punho em duas
 *    threads: nao. Dois punhos sobre a mesma tabela em threads diferentes:
 *    NAO TESTADO -- e nao prometemos o que nao medimos.
 *
 * 6. A ETIQUETA DO PUNHO E UMA REDE, NAO UM CONTRATO. Todo punho carrega uma
 *    marca conferida a cada chamada, e o "fechar" a zera. Isso PEGA punho ja
 *    liberado e punho do tipo errado no lugar errado. Isso NAO pega memoria
 *    liberada e reocupada por outra coisa. Trate como diagnostico, nao como
 *    garantia de seguranca.
 */

#ifndef PHXSQL_H
#define PHXSQL_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* --------------------------------------------------------------- codigos */

#define PHX_OK                 0
/* Nao ha o que devolver: a linha nao existe, o cursor acabou. NAO e erro. */
#define PHX_NAO_HA             1

#define PHX_ERRO_PANICO       (-1)
#define PHX_ERRO_PONTEIRO     (-2)
#define PHX_ERRO_UTF8         (-3)
#define PHX_ERRO_BUFFER       (-4)
#define PHX_ERRO_USO          (-5)
#define PHX_ERRO_ENVENENADO   (-6)

/* Os codigos do motor, iguais aos da porta de dados. Numero nunca muda e
 * numero aposentado nunca volta -- e o que faz tratar por codigo valer mais
 * que comparar a frase da mensagem. */
#define PHX_CORROMPIDO          1001
#define PHX_ASSINATURA_INVALIDA 1002
#define PHX_VERSAO_NAO_SUPORTADA 1003
#define PHX_ESQUEMA_INVALIDO    2001
#define PHX_TIPO_INVALIDO       2002
#define PHX_NAO_ENCONTRADO      3001
#define PHX_DUPLICADO           3002
#define PHX_LIMITE_EXCEDIDO     3003
#define PHX_CONFLITO            3004
#define PHX_SINAL               3005
#define PHX_ACESSO_NEGADO       4001
#define PHX_EM_CARGA            4002
#define PHX_REDIRECIONA         4003
#define PHX_SPARE_EM_ESPERA     4004
#define PHX_ERRO_DE_ES          5001
#define PHX_CANCELADO           6001

/* ------------------------------------------------------------- bandeiras */

/* phx_base_abrir */
#define PHX_CRIAR              1u

/* phx_esquema_coluna */
#define PHX_COL_OBRIGATORIA    1u

/* phx_esquema_indice */
#define PHX_IDX_UNICO          1u
#define PHX_IDX_PRIMARIA       2u

/* phx_esquema_indice_coluna */
#define PHX_IDX_DESC           1u
#define PHX_IDX_SEM_CAIXA      2u

/* O que uma varredura ou contagem enxerga. */
#define PHX_VISAO_ATIVAS       0u
#define PHX_VISAO_EXCLUIDAS    1u
#define PHX_VISAO_TODAS        2u

/* As operacoes do diario. Os numeros sao os que o .log grava. */
#define PHX_OP_INCLUSAO        1u
#define PHX_OP_ALTERACAO       2u
#define PHX_OP_EXCLUSAO        3u

/* ---------------------------------------------------------------- tipos */

/* Tipo de um valor que atravessa a fronteira. */
#define PHX_NULO       0
#define PHX_BOOL       1
#define PHX_INT        2
#define PHX_UINT       3
#define PHX_REAL       4
#define PHX_DECIMAL    5   /* 16 bytes little-endian de um inteiro escalado */
#define PHX_DATA       6   /* dias desde 1970-01-01                          */
#define PHX_HORA       7   /* centesimos de segundo desde a meia-noite       */
#define PHX_DATAHORA   8   /* milissegundos desde 1970-01-01T00:00:00Z       */
#define PHX_TEXTO      9
#define PHX_BIN       10
#define PHX_MEMO      11
#define PHX_UUID      12   /* 16 bytes crus */
#define PHX_UUID256   13   /* 32 bytes crus */

/* Tipo de uma coluna do esquema. */
#define PHX_COL_BOOL       1
#define PHX_COL_INT1       2
#define PHX_COL_INT2       3
#define PHX_COL_INT4       4
#define PHX_COL_INT8       5
#define PHX_COL_UINT1      6
#define PHX_COL_UINT2      7
#define PHX_COL_UINT4      8
#define PHX_COL_UINT8      9
#define PHX_COL_REAL4     10
#define PHX_COL_REAL8     11
#define PHX_COL_DECIMAL   12
#define PHX_COL_DATA      13
#define PHX_COL_HORA      14
#define PHX_COL_DATAHORA  15
#define PHX_COL_STR       16
#define PHX_COL_BIN       17
#define PHX_COL_MEMO      18
#define PHX_COL_UUID      19
#define PHX_COL_UUID256   20
#define PHX_COL_SEQUENCIA 21

/* --------------------------------------------------------------- punhos */

typedef struct PhxBase    PhxBase;
typedef struct PhxEsquema PhxEsquema;
typedef struct PhxTabela  PhxTabela;
typedef struct PhxLinha   PhxLinha;
typedef struct PhxCursor  PhxCursor;
typedef struct PhxImagem  PhxImagem;

/* Um valor.
 *
 * Na ENTRADA os campos `dados`/`tam` apontam para a SUA memoria, e sao
 * copiados antes de a chamada voltar -- pode ser pilha.
 *
 * Na SAIDA eles apontam para dentro do punho da linha, e valem ate o
 * phx_linha_liberar.
 *
 * Atencao ao PHX_UINT: o valor vem em `numero`, que tem sinal, guardando o
 * PADRAO DE BITS do uint64. Leia com (uint64_t)v.numero. Um contador acima de
 * 2^63 aparece negativo se voce esquecer -- e nao e defeito, e a unica forma
 * de nao perder o topo da faixa sem inflar a struct.
 */
typedef struct PhxValor {
    int32_t     tipo;
    uint32_t    reservado;     /* sempre 0 */
    int64_t     numero;
    double      real;
    const uint8_t *dados;
    size_t      tam;
} PhxValor;

/* O que phx_verificar devolve. */
typedef struct PhxRelatorio {
    uint64_t registros;
    uint64_t slots;
    uint64_t marcadas;
    uint64_t eventos;
    uint64_t descartadas;
    uint64_t motivos;
    uint64_t indices;
    uint64_t trilha;
} PhxRelatorio;

/* Um evento do diario -- o que a sincronia envia e recebe. */
typedef struct PhxEvento {
    int64_t  carimbo;      /* ms desde 1970-01-01T00:00:00Z */
    uint64_t rowid;
    uint64_t versao;
    uint32_t operacao;     /* PHX_OP_* */
    uint32_t usuario;
    uint32_t origem;       /* de que servidor a escrita NASCEU; 0 = local */
    uint32_t tam_imagem;
} PhxEvento;

/* Acucar para literais: phx_base_abrir(PHX_T("/data"), PHX_T("app"), ...) */
#define PHX_T(s) ((const uint8_t *)(s)), (sizeof(s) - 1)

/* ----------------------------------------------------------------- casa */

int32_t phx_versao(uint8_t *destino, size_t cap, size_t *precisa);

/* A mensagem do ultimo erro DESTA thread. Vazia quando nada falhou. */
int32_t phx_ultimo_erro(uint8_t *destino, size_t cap, size_t *precisa);

/* O nome simbolico de um codigo ("DUPLICADO"). Literal estatico,
 * NUL-terminado, nunca se libera. */
const char *phx_erro_nome(int32_t codigo);

/* ----------------------------------------------------------------- base */

int32_t phx_base_abrir(const uint8_t *caminho, size_t caminho_tam,
                       const uint8_t *nome, size_t nome_tam,
                       uint32_t sinalizadores, PhxBase **saida);
int32_t phx_base_fechar(PhxBase *base);

/* Relista e devolve quantas. Chame antes de phx_base_tabela_nome. */
int32_t phx_base_tabelas_qtd(PhxBase *base, size_t *qtd);
int32_t phx_base_tabela_nome(PhxBase *base, size_t i,
                             uint8_t *destino, size_t cap, size_t *precisa);

/* -------------------------------------------------------------- esquema */

int32_t phx_esquema_novo(const uint8_t *nome, size_t nome_tam, PhxEsquema **saida);

/* `largura` so vale para PHX_COL_STR; `precisao`/`escala` so para
 * PHX_COL_DECIMAL. Os demais tipos ignoram os tres. */
int32_t phx_esquema_coluna(PhxEsquema *esq, const uint8_t *nome, size_t nome_tam,
                           int32_t tipo, uint32_t largura,
                           uint8_t precisao, uint8_t escala,
                           uint32_t sinalizadores);

int32_t phx_esquema_indice(PhxEsquema *esq, const uint8_t *nome, size_t nome_tam,
                           uint32_t sinalizadores);
/* Acrescenta ao ULTIMO indice aberto. `coluna` e a posicao no esquema. */
int32_t phx_esquema_indice_coluna(PhxEsquema *esq, size_t coluna,
                                  uint32_t sinalizadores);
int32_t phx_esquema_liberar(PhxEsquema *esq);

/* --------------------------------------------------------------- tabela */

/* O esquema NAO e consumido: quem o criou continua dono e o libera.
 * `schema` e o subdiretorio logico; passe NULL, 0 para a raiz. */
int32_t phx_tabela_criar(PhxBase *base, const uint8_t *schema, size_t schema_tam,
                         PhxEsquema *esq, PhxTabela **saida);
int32_t phx_tabela_abrir(PhxBase *base, const uint8_t *nome, size_t nome_tam,
                         PhxTabela **saida);
int32_t phx_tabela_fechar(PhxTabela *tab);

/* Quantas linhas a VISAO enxerga. A visao e parametro porque, com exclusao
 * suave, "quantas linhas tem a tabela" tem tres respostas. */
int32_t phx_tabela_registros(PhxTabela *tab, uint32_t visao, uint64_t *qtd);

int32_t phx_tabela_colunas(PhxTabela *tab, size_t *qtd);
int32_t phx_tabela_coluna_nome(PhxTabela *tab, size_t i,
                               uint8_t *destino, size_t cap, size_t *precisa);
int32_t phx_tabela_coluna_tipo(PhxTabela *tab, size_t i, int32_t *tipo);

int32_t phx_sincronizar(PhxTabela *tab);
int32_t phx_verificar(PhxTabela *tab, PhxRelatorio *rel);

/* ----------------------------------------------------------------- dado */

int32_t phx_inserir(PhxTabela *tab, const PhxValor *vals, size_t qtd,
                    uint64_t *rowid);

/* Grava por cima, sem conferir versao. E o comportamento de sempre. */
int32_t phx_atualizar(PhxTabela *tab, uint64_t rowid,
                      const PhxValor *vals, size_t qtd);

/* Grava SE a linha ainda estiver na versao lida; senao devolve PHX_CONFLITO.
 * Guarda pedida, nao imposta: quem usa phx_atualizar continua como antes. */
int32_t phx_atualizar_se(PhxTabela *tab, uint64_t rowid,
                         const PhxValor *vals, size_t qtd,
                         uint64_t versao_esperada);
int32_t phx_versao_da_linha(PhxTabela *tab, uint64_t rowid, uint64_t *versao);

int32_t phx_excluir(PhxTabela *tab, uint64_t rowid,
                    const uint8_t *motivo, size_t motivo_tam, uint8_t *saiu);
int32_t phx_excluir_suave(PhxTabela *tab, uint64_t rowid,
                          const uint8_t *motivo, size_t motivo_tam, uint8_t *saiu);
int32_t phx_restaurar(PhxTabela *tab, uint64_t rowid,
                      const uint8_t *motivo, size_t motivo_tam, uint8_t *saiu);

/* PHX_NAO_HA quando a linha nao existe -- e isso nao e erro. */
int32_t phx_ler(PhxTabela *tab, uint64_t rowid, PhxLinha **saida);
/* Vista, nao copia: os ponteiros valem ate o phx_linha_liberar. */
int32_t phx_linha_valores(PhxLinha *linha, const PhxValor **vals, size_t *qtd);
int32_t phx_linha_liberar(PhxLinha *linha);

/* Buffer pequeno devolve PHX_ERRO_BUFFER com o total em `achados`. */
int32_t phx_buscar(PhxTabela *tab, const uint8_t *indice, size_t indice_tam,
                   const PhxValor *chave, size_t chave_qtd,
                   uint64_t *rowids, size_t cap, size_t *achados);

/* --------------------------------------------------------------- cursor */

/* Ordem de digitacao. Anda em lotes pelo keyset do .reg: uma tabela de um
 * milhao de linhas nunca vira um vetor de um milhao de rowids na memoria. */
int32_t phx_cursor_abrir(PhxTabela *tab, uint32_t visao, PhxCursor **saida);

/* Ordem de um indice. Este MATERIALIZA a ordem do .ndx de uma vez -- a arvore
 * nao tem "continue depois desta chave" barato como o .reg tem. Num aparelho
 * pequeno com tabela grande, prefira o de cima. */
int32_t phx_cursor_abrir_indice(PhxTabela *tab, const uint8_t *indice,
                                size_t indice_tam, PhxCursor **saida);

/* PHX_NAO_HA quando acabou.
 *
 * Recebe os DOIS punhos de proposito: assim o cursor nao guarda ponteiro para
 * a tabela, e um cursor que sobreviva a ela nao tem para onde apontar. Cruzar
 * os punhos devolve PHX_ERRO_USO -- um erro diagnosticavel no lugar de um
 * uso-depois-de-liberar. */
int32_t phx_cursor_proximo(PhxTabela *tab, PhxCursor *cur, uint64_t *rowid);
int32_t phx_cursor_liberar(PhxCursor *cur);

/* ---------------------------------------------------------- replicacao */

/* Sem imagem o diario diz QUE o rowid mudou e nao diz PARA QUE: basta para
 * auditoria e nao basta para replicar. Ligue antes de gravar o que vai ser
 * sincronizado. */
int32_t phx_imagem_no_diario(PhxTabela *tab, uint8_t ligado);

/* Quantos eventos ja existem. E a POSICAO da sincronia. */
int32_t phx_diario_qtd(PhxTabela *tab, uint64_t *qtd);

int32_t phx_diario_ler(PhxTabela *tab, uint64_t pular,
                       PhxEvento *saida, size_t cap, size_t *lidos);

/* Um evento COM os bytes que a outra ponta vai gravar. `saida_img` recebe
 * NULL quando o evento nao tem imagem (exclusao nao leva: o rowid basta). */
int32_t phx_diario_evento_com_imagem(PhxTabela *tab, uint64_t pular,
                                     PhxEvento *ev, PhxImagem **saida_img);
int32_t phx_imagem_bytes(PhxImagem *img, const uint8_t **dados, size_t *tam);
int32_t phx_imagem_liberar(PhxImagem *img);

/* Aplica um evento vindo da outra ponta.
 *
 * O .reg nunca reaproveita slot, entao aplicar todos os eventos NA ORDEM
 * produz rowids identicos aos da origem, sem negociar nada. Se o rowid que
 * sair aqui nao bater com o do evento, esta ponta ja divergiu -- e a chamada
 * PARA, em vez de espalhar a divergencia. */
int32_t phx_aplicar_evento(PhxTabela *tab, uint32_t operacao, uint64_t rowid,
                           const uint8_t *imagem, size_t imagem_tam,
                           uint64_t *saiu);

/* Carimbo e ORIGEM do proximo evento. A origem e o que mata o laco infinito
 * do bidirecional: o que nasceu la nao volta para la. */
int32_t phx_forcar_proximo_evento(PhxTabela *tab, int64_t carimbo, uint32_t origem);

/* ------------------------------------------------------ atalhos de valor */

static inline PhxValor phx_nulo(void) {
    PhxValor v; v.tipo = PHX_NULO; v.reservado = 0; v.numero = 0;
    v.real = 0.0; v.dados = 0; v.tam = 0; return v;
}
static inline PhxValor phx_int(int64_t n) {
    PhxValor v = phx_nulo(); v.tipo = PHX_INT; v.numero = n; return v;
}
static inline PhxValor phx_uint(uint64_t n) {
    PhxValor v = phx_nulo(); v.tipo = PHX_UINT; v.numero = (int64_t)n; return v;
}
static inline PhxValor phx_bool(int b) {
    PhxValor v = phx_nulo(); v.tipo = PHX_BOOL; v.numero = b ? 1 : 0; return v;
}
static inline PhxValor phx_real(double r) {
    PhxValor v = phx_nulo(); v.tipo = PHX_REAL; v.real = r; return v;
}
static inline PhxValor phx_bytes(int32_t tipo, const void *p, size_t tam) {
    PhxValor v = phx_nulo(); v.tipo = tipo;
    v.dados = (const uint8_t *)p; v.tam = tam; return v;
}
/* Texto com tamanho, que e a forma certa. Note que NAO ha um phx_cstr: seria
 * o strlen escondido que a regra 4 existe para evitar. */
static inline PhxValor phx_texto(const void *p, size_t tam) {
    return phx_bytes(PHX_TEXTO, p, tam);
}
static inline PhxValor phx_memo(const void *p, size_t tam) {
    return phx_bytes(PHX_MEMO, p, tam);
}
static inline PhxValor phx_bin(const void *p, size_t tam) {
    return phx_bytes(PHX_BIN, p, tam);
}

#ifdef __cplusplus
}
#endif

#endif /* PHXSQL_H */
