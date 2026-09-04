package br.phxsql;

/**
 * Wrapper Java para a biblioteca PhxSql embutida.
 *
 * Carrega libphxsql_ffi.so e expoe as funcoes FFI via JNI.
 */
public class PhxSql {

    static {
        System.loadLibrary("phxsql_ffi");
    }

    /**
     * Executa o teste de prova do PhxSql.
     *
     * @param dir Diretorio onde a base de dados sera criada
     * @return 0 se sucesso, codigo de erro caso contrario
     */
    public static native int testar(String dir);

    /**
     * Retorna a versao do PhxSql.
     *
     * @return string com a versao
     */
    public static native String versao();

    public static void main(String[] args) {
        if (args.length < 1) {
            System.err.println("uso: java br.phxsql.PhxSql <diretorio>");
            System.exit(1);
        }

        System.out.println("PhxSql versao: " + versao());
        int resultado = testar(args[0]);
        System.out.println("Teste: " + (resultado == 0 ? "OK" : "FALHA " + resultado));
        System.exit(resultado);
    }
}
