//! Migração de primeira execução: copia um banco existente (ex.: o
//! `data/wadawko.db` do repositório) para o caminho XDG, se lá ainda não houver
//! banco.
//!
//! A origem é aberta em modo somente leitura e copiada com `VACUUM INTO`, que
//! lê o banco JÁ com o conteúdo do WAL e grava um arquivo único e consolidado:
//! o efeito de um checkpoint, sem escrever na origem (que não é alterada nem
//! apagada).

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use rusqlite::{Connection, OpenFlags};

use crate::db::Db;

/// Resultado de [`migrate_db_if_missing`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Migration {
    /// o banco foi copiado para o destino
    Copied { keys: usize, bytes: u64 },
    /// já havia banco no destino: nada foi feito
    DestinationExists,
    /// não há banco na origem: nada foi feito
    SourceMissing,
}

/// Copia o banco `source` para `dest` se `dest` não existir. Nunca altera nem
/// apaga `source`; nunca sobrescreve `dest`.
pub fn migrate_db_if_missing(source: impl AsRef<Path>, dest: impl AsRef<Path>) -> Result<Migration> {
    let (source, dest) = (source.as_ref(), dest.as_ref());
    if dest.exists() {
        return Ok(Migration::DestinationExists);
    }
    if !source.is_file() {
        return Ok(Migration::SourceMissing);
    }
    if let Some(dir) = dest.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("criar a pasta {}", dir.display()))?;
    }

    // copia para um temporário ao lado e renomeia: um destino pela metade nunca
    // é confundido com um banco pronto
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".migrating");
    let tmp = PathBuf::from(tmp);
    let _ = std::fs::remove_file(&tmp);

    let result = (|| -> Result<usize> {
        let src = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).with_context(|| format!("abrir {} (somente leitura)", source.display()))?;
        src.busy_timeout(std::time::Duration::from_secs(5))?;
        let expected = count_keys(&src).with_context(|| format!("ler {}", source.display()))?;
        let tmp_str = tmp.to_str().context("caminho de destino não é UTF-8")?;
        src.execute("VACUUM INTO ?1", [tmp_str]).with_context(|| format!("copiar {} para {}", source.display(), tmp.display()))?;
        drop(src);

        let copy = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        let check: String = copy.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        let copied = count_keys(&copy)?;
        if check != "ok" || copied != expected {
            bail!("a cópia não confere com a origem ({copied} de {expected} chaves, verificação: {check})");
        }
        Ok(copied)
    })();

    let keys = match result {
        Ok(k) => k,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    };
    std::fs::rename(&tmp, dest).with_context(|| format!("mover a cópia para {}", dest.display()))?;
    let bytes = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    // deixa o destino no modo WAL e com a tabela garantida, como o app espera
    drop(Db::open(dest)?);
    Ok(Migration::Copied { keys, bytes })
}

fn count_keys(conn: &Connection) -> Result<usize> {
    let has_table: i64 = conn.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'kv'", [], |r| r.get(0))?;
    if has_table == 0 {
        return Ok(0);
    }
    Ok(conn.query_row("SELECT count(*) FROM kv", [], |r| r.get::<_, i64>(0))? as usize)
}
