//! `wadawko-cli` — utilitário de linha de comando do WaDAWko (sem UI).
//!
//! ```text
//! wadawko-cli [--db CAMINHO] <comando> [argumentos]
//! ```
//!
//! Cada subcomando mora num arquivo de `commands/`; para acrescentar um, crie
//! o arquivo com um `pub const COMMAND: Command` e registre-o em
//! `commands::COMMANDS`.

mod args;
mod commands;
mod context;

use std::process::ExitCode;

use args::{Args, UsageError};
use context::Context;

const ABOUT: &str = "wadawko-cli — músicas do WaDAWko pela linha de comando";

fn help() -> String {
    let mut out = format!("{ABOUT}\n\nUso: wadawko-cli [--db CAMINHO] <comando> [argumentos]\n\nComandos:\n");
    let width = commands::COMMANDS.iter().map(|c| c.usage.chars().count()).max().unwrap_or(0);
    for c in commands::COMMANDS {
        let pad = " ".repeat(width - c.usage.chars().count());
        out.push_str(&format!("  {}{pad}  {}\n", c.usage, c.about));
    }
    out.push_str(
        "\nOpções globais:\n  --db CAMINHO   banco a usar (padrão: $WADAWKO_DB, senão $XDG_DATA_HOME/wadawko/wadawko.db)\n  -h, --help     esta ajuda (ou a do comando)\n  -V, --version  versão\n",
    );
    out
}

fn run() -> anyhow::Result<()> {
    let mut args = Args::new(std::env::args().skip(1).collect());
    // --db vale em qualquer posição, antes ou depois do comando
    let db = args.option("--db")?;
    let Some(name) = args.next_positional() else {
        if args.flag("-V") || args.flag("--version") {
            println!("wadawko-cli {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        let wants_help = args.flag("-h") || args.flag("--help");
        args.finish()?;
        if wants_help {
            print!("{}", help());
            return Ok(());
        }
        return Err(UsageError(format!("faltou o comando\n\n{}", help())).into());
    };
    let Some(command) = commands::COMMANDS.iter().find(|c| c.name == name) else {
        let names: Vec<&str> = commands::COMMANDS.iter().map(|c| c.name).collect();
        return Err(UsageError(format!("comando desconhecido: {name} (comandos: {})", names.join(", "))).into());
    };
    if args.flag("-h") || args.flag("--help") {
        println!("Uso: wadawko-cli [--db CAMINHO] {}\n\n{}", command.usage, command.help.trim_end());
        return Ok(());
    }
    let ctx = Context::new(db.map(Into::into))?;
    (command.run)(&ctx, args).map_err(|e| match e.downcast::<UsageError>() {
        Ok(u) => UsageError(format!("{}\nUso: wadawko-cli [--db CAMINHO] {}", u.0, command.usage)).into(),
        Err(e) => e,
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if let Some(u) = e.downcast_ref::<UsageError>() {
                eprintln!("erro: {}", u.0);
                return ExitCode::from(2);
            }
            eprintln!("erro: {e:#}");
            ExitCode::FAILURE
        }
    }
}
