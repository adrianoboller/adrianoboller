<?php
// Conexao unica com o MySQL. Parametros no config.php, que nao vai para o
// versionamento (cada filial tem o seu).
require_once __DIR__ . '/../config.php';

function conectar() {
    static $con = null;
    if ($con === null) {
        $con = mysqli_connect(DB_HOST, DB_USER, DB_PASS, DB_NAME);
        if (!$con) {
            die('Erro ao conectar no banco: ' . mysqli_connect_error());
        }
        mysqli_set_charset($con, 'utf8');
    }
    return $con;
}

// Consulta com escape manual - o codigo e de 2009 e nunca migrou para
// prepared statements. Ponto conhecido de divida tecnica.
function q($sql) {
    $r = mysqli_query(conectar(), $sql);
    if (!$r) {
        die('Erro na consulta: ' . mysqli_error(conectar()) . '<br>' . $sql);
    }
    return $r;
}

function esc($s) {
    return mysqli_real_escape_string(conectar(), $s);
}

function maior_atraso_do_cliente($id_cliente) {
    $sql = "SELECT MAX(DATEDIFF(CURDATE(), VENCIMENTO)) AS atraso
              FROM titulo
             WHERE ID_CLIENTE = " . (int) $id_cliente . "
               AND SITUACAO = 'ABERTO'
               AND VENCIMENTO < CURDATE()";
    $row = mysqli_fetch_assoc(q($sql));
    return (int) $row['atraso'];
}
