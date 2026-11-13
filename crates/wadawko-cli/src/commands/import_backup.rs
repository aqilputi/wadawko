//! `import-backup` — importa um backup do localStorage (dicionário chave → texto).

use anyhow::Result;
use wadawko_store::{BackupOptions, import_backup_file};

use super::Command;
use crate::args::Args;
use crate::context::Context;

pub const COMMAND: Command = Command {
    name: "import-backup",
    usage: "import-backup <arquivo> [--overwrite]",
    about: "importa um backup do localStorage (chave → JSON)",
    help: "Importa um backup como data/backup-localstorage-AAAA-MM-DD.json: um objeto JSON\ncujas chaves são as do banco (wadawko:workspace, wadawko:project:<id>,\nwadawko:theme) e cujos valores são textos.\n\nMúsicas que o banco ainda não tem são acrescentadas (e entram na lista); as que\njá existem ficam como estão.\n\n  --overwrite   substitui as músicas (e o tema) que diferem do backup\n\nCria o banco se ele não existir.",
    run,
};

fn run(ctx: &Context, mut args: Args) -> Result<()> {
    let overwrite = args.flag("--overwrite");
    let file = args.positional("o arquivo de backup")?;
    args.finish()?;

    if !std::path::Path::new(&file).is_file() {
        anyhow::bail!("arquivo não encontrado: {file}");
    }
    let db = ctx.open_read_write()?;
    let report = import_backup_file(&db, &file, BackupOptions { overwrite })?;

    for (id, title) in &report.added {
        println!("+ {id}  {title}");
    }
    for (id, title) in &report.overwritten {
        println!("~ {id}  {title} (substituída)");
    }
    for id in &report.unchanged {
        println!("= {id}  (já estava igual)");
    }
    for id in &report.kept_existing {
        println!("! {id}  (já existe com conteúdo diferente; mantida — use --overwrite para substituir)");
    }
    for key in &report.other_set {
        println!("+ {key}");
    }
    for (key, why) in &report.invalid {
        println!("x {key}  inválida: {why}");
    }
    for key in &report.ignored {
        println!("- {key}  (ignorada)");
    }
    println!(
        "{} acrescentada(s), {} substituída(s), {} igual(is), {} mantida(s), {} inválida(s) → {}",
        report.added.len(),
        report.overwritten.len(),
        report.unchanged.len(),
        report.kept_existing.len(),
        report.invalid.len(),
        ctx.db_path().display()
    );
    Ok(())
}
