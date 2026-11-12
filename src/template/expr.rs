//! Placeholder parsing shared by check and expand.

/// A piece of a scalar string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Piece<'s> {
    Text(&'s str),
    Expr(&'s str),
}

/// Parsed form of a scalar string.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Scalar<'s> {
    /// No `{{`: the string is kept verbatim.
    Literal,
    /// Exactly one placeholder, optionally surrounded by whitespace: yields a typed value.
    Single(&'s str),
    /// Text with placeholders: renders a string.
    Mixed(Vec<Piece<'s>>),
}

/// Splits `s` at `{{ … }}` placeholders. The first `}}` after a `{{` closes it, so an
/// expression cannot contain a literal `}}`.
pub(crate) fn parse_scalar(s: &str) -> Result<Scalar<'_>, String> {
    if !s.contains("{{") {
        return Ok(Scalar::Literal);
    }
    let mut pieces = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        if start > 0 {
            pieces.push(Piece::Text(&rest[..start]));
        }
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .ok_or_else(|| format!("unterminated placeholder in `{s}`"))?;
        let expr = &after[..end];
        if expr.contains("{{") {
            return Err(format!("nested `{{{{` in placeholder in `{s}`"));
        }
        if expr.trim().is_empty() {
            return Err(format!("empty placeholder in `{s}`"));
        }
        pieces.push(Piece::Expr(expr));
        rest = &after[end + 2..];
    }
    if !rest.is_empty() {
        pieces.push(Piece::Text(rest));
    }
    let exprs: Vec<&str> = pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Expr(e) => Some(*e),
            Piece::Text(_) => None,
        })
        .collect();
    let only_whitespace = pieces.iter().all(|p| match p {
        Piece::Text(t) => t.trim().is_empty(),
        Piece::Expr(_) => true,
    });
    if exprs.len() == 1 && only_whitespace {
        return Ok(Scalar::Single(exprs[0]));
    }
    Ok(Scalar::Mixed(pieces))
}

/// The expression of a `when:` string: a bare expression, or one `{{ … }}` placeholder.
pub(crate) fn when_expression(s: &str) -> Result<&str, String> {
    match parse_scalar(s)? {
        Scalar::Literal => Ok(s),
        Scalar::Single(expr) => Ok(expr),
        Scalar::Mixed(_) => Err(format!("`when` must be one expression, got `{s}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars() {
        assert_eq!(parse_scalar("plain").unwrap(), Scalar::Literal);
        assert_eq!(
            parse_scalar(" {{ a.b }} ").unwrap(),
            Scalar::Single(" a.b ")
        );
        assert_eq!(
            parse_scalar("x {{ a }}y").unwrap(),
            Scalar::Mixed(vec![
                Piece::Text("x "),
                Piece::Expr(" a "),
                Piece::Text("y")
            ])
        );
        assert!(matches!(
            parse_scalar("{{ a }}{{ b }}").unwrap(),
            Scalar::Mixed(_)
        ));
        assert!(parse_scalar("{{ a").is_err());
        assert!(parse_scalar("{{ }}").is_err());
        assert_eq!(when_expression("a == 1").unwrap(), "a == 1");
        assert_eq!(when_expression("{{ a }}").unwrap(), " a ");
    }
}
