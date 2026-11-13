//! Parser simples de argumentos: opções `--nome valor` / `--nome=valor`,
//! flags e posicionais, em qualquer ordem. `--` encerra as opções.

use std::fmt;

/// Erro de uso (argumento faltando, sobrando ou inválido): sai com código 2.
#[derive(Debug)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

pub fn usage(msg: impl Into<String>) -> anyhow::Error {
    UsageError(msg.into()).into()
}

/// Argumentos ainda não consumidos.
pub struct Args {
    rest: Vec<String>,
}

impl Args {
    pub fn new(rest: Vec<String>) -> Args {
        Args { rest }
    }

    /// onde começam os posicionais forçados (depois de `--`)
    fn options_end(&self) -> usize {
        self.rest.iter().position(|a| a == "--").unwrap_or(self.rest.len())
    }

    /// Consome a flag `name` (ex.: `--overwrite`); devolve se estava presente.
    pub fn flag(&mut self, name: &str) -> bool {
        let end = self.options_end();
        match self.rest[..end].iter().position(|a| a == name) {
            Some(i) => {
                self.rest.remove(i);
                true
            }
            None => false,
        }
    }

    /// Consome a opção com valor `name` (`--db x` ou `--db=x`).
    pub fn option(&mut self, name: &str) -> anyhow::Result<Option<String>> {
        let end = self.options_end();
        let prefix = format!("{name}=");
        for i in 0..end {
            if self.rest[i] == name {
                if i + 1 >= end {
                    return Err(usage(format!("{name} precisa de um valor")));
                }
                self.rest.remove(i);
                return Ok(Some(self.rest.remove(i)));
            }
            if let Some(v) = self.rest[i].strip_prefix(&prefix) {
                let v = v.to_string();
                self.rest.remove(i);
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    /// Próximo argumento posicional (o que não começa com `-`; `-` sozinho
    /// conta como posicional: é a saída padrão).
    pub fn next_positional(&mut self) -> Option<String> {
        let end = self.options_end();
        if let Some(i) = self.rest[..end].iter().position(|a| !a.starts_with('-') || a == "-") {
            return Some(self.rest.remove(i));
        }
        // depois de `--` tudo é posicional
        (end + 1 < self.rest.len()).then(|| self.rest.remove(end + 1))
    }

    /// Posicional obrigatório.
    pub fn positional(&mut self, what: &str) -> anyhow::Result<String> {
        self.next_positional().ok_or_else(|| usage(format!("faltou {what}")))
    }

    /// Erro se sobrou argumento não reconhecido.
    pub fn finish(mut self) -> anyhow::Result<()> {
        if self.rest.last().is_some_and(|a| a == "--") {
            self.rest.pop();
        }
        match self.rest.first() {
            None => Ok(()),
            Some(a) if a.starts_with('-') && a != "--" => Err(usage(format!("opção desconhecida: {a}"))),
            Some(a) => Err(usage(format!("argumento sobrando: {a}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Args {
        Args::new(list.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn options_in_any_position() {
        let mut a = args(&["export", "--db", "x.db", "Renshū", "-"]);
        assert_eq!(a.option("--db").unwrap().as_deref(), Some("x.db"));
        assert_eq!(a.next_positional().as_deref(), Some("export"));
        assert_eq!(a.next_positional().as_deref(), Some("Renshū"));
        assert_eq!(a.next_positional().as_deref(), Some("-"));
        a.finish().unwrap();

        let mut a = args(&["--db=y.db", "list"]);
        assert_eq!(a.option("--db").unwrap().as_deref(), Some("y.db"));
        assert_eq!(a.positional("o comando").unwrap(), "list");
    }

    #[test]
    fn errors() {
        assert!(args(&["list", "--db"]).option("--db").is_err());
        let mut a = args(&["list", "--nope"]);
        a.next_positional();
        assert!(a.finish().unwrap_err().to_string().contains("--nope"));
        let mut a = args(&["x"]);
        assert!(!a.flag("--overwrite"));
        assert!(a.finish().unwrap_err().to_string().contains("sobrando"));
    }

    #[test]
    fn double_dash_ends_options() {
        let mut a = args(&["export", "--", "--titulo-estranho"]);
        assert_eq!(a.next_positional().as_deref(), Some("export"));
        assert!(!a.flag("--titulo-estranho"));
        assert_eq!(a.next_positional().as_deref(), Some("--titulo-estranho"));
        a.finish().unwrap();
    }
}
