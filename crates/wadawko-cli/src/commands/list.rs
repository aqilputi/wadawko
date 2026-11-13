//! `list` — lista as músicas do banco.

use anyhow::Result;
use wadawko_store::list_projects;

use super::Command;
use crate::args::Args;
use crate::context::Context;

pub const COMMAND: Command = Command {
    name: "list",
    usage: "list",
    about: "lista as músicas do banco",
    help: "Lista as músicas do workspace (a atual marcada com *), com id, data da última\ngravação (UTC), andamento, linhas, compassos e tamanho. Chaves de projeto que o\nworkspace não lista aparecem marcadas com ? (órfãs). Só lê o banco.",
    run,
};

fn run(ctx: &Context, args: Args) -> Result<()> {
    args.finish()?;
    let db = ctx.open_read_only()?;
    let projects = list_projects(&db)?;
    println!("banco: {}", ctx.db_path().display());
    if projects.is_empty() {
        println!("(nenhuma música)");
        return Ok(());
    }
    let id_w = projects.iter().map(|p| p.meta.id.chars().count()).max().unwrap_or(2).max(2);
    let title_w = projects.iter().map(|p| p.meta.title.chars().count()).max().unwrap_or(6).max(6);
    println!("  {:<id_w$}  {:<title_w$}  {:<16}  {:>5}  {:>6}  {:>9}  {:>8}", "id", "título", "atualizado (UTC)", "bpm", "linhas", "compassos", "bytes");
    let mut unreadable = 0;
    for p in &projects {
        let mark = if p.current {
            '*'
        } else if p.listed {
            ' '
        } else {
            '?'
        };
        let bytes = p.bytes.map_or_else(|| "-".to_string(), |b| b.to_string());
        let when = format_utc(p.meta.updated_at);
        match &p.song {
            Ok(song) => println!(
                "{mark} {:<id_w$}  {:<title_w$}  {when:<16}  {:>5}  {:>6}  {:>9}  {bytes:>8}",
                p.meta.id,
                p.meta.title,
                song.tempo,
                song.tracks.len(),
                song.measures.len()
            ),
            Err(why) => {
                unreadable += 1;
                println!("{mark} {:<id_w$}  {:<title_w$}  {when:<16}  ILEGÍVEL: {why}", p.meta.id, p.meta.title);
            }
        }
    }
    let orphans = projects.iter().filter(|p| !p.listed).count();
    let mut summary = format!("{} música(s)", projects.len());
    if orphans > 0 {
        summary.push_str(&format!(", {orphans} fora do workspace (?)"));
    }
    if unreadable > 0 {
        summary.push_str(&format!(", {unreadable} ilegível(is)"));
    }
    println!("{summary}");
    Ok(())
}

/// `ms` desde a época → `AAAA-MM-DD HH:MM` em UTC.
fn format_utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // dias → data civil (algoritmo de Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", rem / 3600, rem % 3600 / 60)
}

#[cfg(test)]
mod tests {
    use super::format_utc;

    #[test]
    fn utc_dates() {
        assert_eq!(format_utc(0), "1970-01-01 00:00");
        assert_eq!(format_utc(951_782_400_000), "2000-02-29 00:00");
        assert_eq!(format_utc(1_790_862_502_440), "2026-10-01 13:48");
    }
}
