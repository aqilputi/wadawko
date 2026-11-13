//! O que todo subcomando recebe: qual banco usar e como abri-lo.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use wadawko_store::{Db, default_db_path};

pub struct Context {
    db_path: PathBuf,
}

impl Context {
    /// `--db`, senão `$WADAWKO_DB`, senão `$XDG_DATA_HOME/wadawko/wadawko.db`.
    pub fn new(db_override: Option<PathBuf>) -> Result<Context> {
        let db_path = match db_override.or_else(|| std::env::var_os("WADAWKO_DB").filter(|v| !v.is_empty()).map(PathBuf::from)) {
            Some(p) => p,
            None => default_db_path()?,
        };
        Ok(Context { db_path })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Abre o banco em modo somente leitura (comandos que só consultam: não
    /// criam arquivo, não gravam, não fazem checkpoint).
    pub fn open_read_only(&self) -> Result<Db> {
        if !self.db_path.is_file() {
            bail!("banco não encontrado: {} (use --db CAMINHO)", self.db_path.display());
        }
        Db::open_read_only(&self.db_path)
    }

    /// Abre o banco para escrita, criando-o se não existir.
    pub fn open_read_write(&self) -> Result<Db> {
        Db::open(&self.db_path)
    }
}
