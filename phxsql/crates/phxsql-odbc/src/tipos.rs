//! Os tipos e constantes do ODBC 3.x que este driver usa.
//!
//! Os nomes seguem o cabecalho `sql.h`/`sqlext.h` da especificacao, porque e
//! por eles que qualquer pessoa confere um driver ODBC -- traduzir os nomes
//! aqui obrigaria o leitor a manter uma tabela de/para na cabeca. Os valores
//! sao fixos pela especificacao e nao mudam entre plataformas.
//!
//! Larguras que importam num driver de 64 bits: `SQLLEN`/`SQLULEN` tem o
//! tamanho do ponteiro (e o unixODBC de 64 bits e compilado assim, tal como o
//! ODBC do Windows x64). Errar isso nao da erro de compilacao -- da lixo de
//! memoria no aplicativo cliente.

use std::os::raw::c_void;

pub type SqlReturn = i16;
pub type SqlSmallint = i16;
pub type SqlUSmallint = u16;
pub type SqlInteger = i32;
pub type SqlLen = isize;
pub type SqlULen = usize;
pub type SqlHandle = *mut c_void;
pub type SqlPointer = *mut c_void;
pub type SqlChar = u8;

// Codigos de retorno.
pub const SQL_SUCCESS: SqlReturn = 0;
pub const SQL_SUCCESS_WITH_INFO: SqlReturn = 1;
pub const SQL_ERROR: SqlReturn = -1;
pub const SQL_INVALID_HANDLE: SqlReturn = -2;
pub const SQL_NO_DATA: SqlReturn = 100;

// Tipos de handle.
pub const SQL_HANDLE_ENV: SqlSmallint = 1;
pub const SQL_HANDLE_DBC: SqlSmallint = 2;
pub const SQL_HANDLE_STMT: SqlSmallint = 3;

// Comprimento "string termina em NUL".
pub const SQL_NTS: SqlInteger = -3;

// Indicador de valor nulo em SQLBindCol/SQLGetData.
pub const SQL_NULL_DATA: SqlLen = -1;

// Atributos de ambiente.
pub const SQL_ATTR_ODBC_VERSION: SqlInteger = 200;

// Opcoes do SQLFreeStmt.
pub const SQL_CLOSE: SqlUSmallint = 0;
pub const SQL_UNBIND: SqlUSmallint = 2;
pub const SQL_RESET_PARAMS: SqlUSmallint = 3;

// Tipos SQL (o que o SQLDescribeCol declara).
pub const SQL_CHAR: SqlSmallint = 1;
pub const SQL_DECIMAL: SqlSmallint = 3;
pub const SQL_INTEGER: SqlSmallint = 4;
pub const SQL_SMALLINT: SqlSmallint = 5;
pub const SQL_REAL: SqlSmallint = 7;
pub const SQL_DOUBLE: SqlSmallint = 8;
pub const SQL_VARCHAR: SqlSmallint = 12;
pub const SQL_LONGVARCHAR: SqlSmallint = -1;
pub const SQL_BIGINT: SqlSmallint = -5;
pub const SQL_TINYINT: SqlSmallint = -6;
pub const SQL_BIT: SqlSmallint = -7;
pub const SQL_TYPE_DATE: SqlSmallint = 91;
pub const SQL_TYPE_TIME: SqlSmallint = 92;
pub const SQL_TYPE_TIMESTAMP: SqlSmallint = 93;

// Tipos C (o que o SQLGetData/SQLBindCol entrega ao aplicativo).
pub const SQL_C_CHAR: SqlSmallint = 1;
pub const SQL_C_LONG: SqlSmallint = 4;
pub const SQL_C_SHORT: SqlSmallint = 5;
pub const SQL_C_FLOAT: SqlSmallint = 7;
pub const SQL_C_DOUBLE: SqlSmallint = 8;
pub const SQL_C_DEFAULT: SqlSmallint = 99;
pub const SQL_C_SSHORT: SqlSmallint = -15;
pub const SQL_C_SLONG: SqlSmallint = -16;
pub const SQL_C_SBIGINT: SqlSmallint = -25;

// Nulabilidade no SQLDescribeCol.
pub const SQL_NO_NULLS: SqlSmallint = 0;
pub const SQL_NULLABLE: SqlSmallint = 1;
pub const SQL_NULLABLE_UNKNOWN: SqlSmallint = 2;

// O subconjunto de SQLGetInfo que este driver responde.
pub const SQL_DATA_SOURCE_NAME: SqlUSmallint = 2;
pub const SQL_DRIVER_NAME: SqlUSmallint = 6;
pub const SQL_DRIVER_VER: SqlUSmallint = 7;
pub const SQL_SERVER_NAME: SqlUSmallint = 13;
pub const SQL_DBMS_NAME: SqlUSmallint = 17;
pub const SQL_DBMS_VER: SqlUSmallint = 18;
pub const SQL_TXN_CAPABLE: SqlUSmallint = 46;
pub const SQL_USER_NAME: SqlUSmallint = 47;
pub const SQL_DRIVER_ODBC_VER: SqlUSmallint = 77;
pub const SQL_GETDATA_EXTENSIONS: SqlUSmallint = 81;

// SQLColAttribute: os campos que ferramentas de linha de comando pedem para
// montar a grade. Os pares antigo/novo (SQL_COLUMN_* / SQL_DESC_*) valem os
// dois, porque ha cliente de cada epoca.
pub const SQL_COLUMN_NAME: SqlUSmallint = 1;
pub const SQL_COLUMN_TYPE: SqlUSmallint = 2;
pub const SQL_COLUMN_LENGTH: SqlUSmallint = 3;
pub const SQL_COLUMN_NULLABLE: SqlUSmallint = 7;
/// SQL_COLUMN_LABEL do ODBC 2 e SQL_DESC_LABEL do 3 sao o mesmo 18 -- e e o
/// que o isql pede para escrever o cabecalho da grade.
pub const SQL_DESC_LABEL: SqlUSmallint = 18;
pub const SQL_DESC_DISPLAY_SIZE: SqlUSmallint = 6;
pub const SQL_DESC_TYPE: SqlUSmallint = 1002;
pub const SQL_DESC_LENGTH: SqlUSmallint = 1003;
pub const SQL_DESC_NULLABLE: SqlUSmallint = 1008;
pub const SQL_DESC_NAME: SqlUSmallint = 1011;
