//! Subcomandos: um arquivo por comando, registrados em [`COMMANDS`].

use crate::args::Args;
use crate::context::Context;

mod export;
mod import;
mod import_backup;
mod list;
mod render;

/// Um subcomando do `wadawko-cli`.
pub struct Command {
    /// nome digitado na linha de comando
    pub name: &'static str,
    /// sinopse dos argumentos (começa pelo nome)
    pub usage: &'static str,
    /// uma linha para a ajuda geral
    pub about: &'static str,
    /// ajuda detalhada (`wadawko-cli <comando> --help`)
    pub help: &'static str,
    /// recebe os argumentos depois do nome; deve terminar com `args.finish()`
    pub run: fn(&Context, Args) -> anyhow::Result<()>,
}

/// Todos os subcomandos, na ordem em que aparecem na ajuda.
pub const COMMANDS: &[Command] = &[list::COMMAND, export::COMMAND, render::COMMAND, import::COMMAND, import_backup::COMMAND];
