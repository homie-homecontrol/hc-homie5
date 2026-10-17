use std::fmt;

/// Display adapter that cuts a string at a UTF-8 boundary after at most `max_bytes` bytes.
///
/// The omitted size is appended as `…(+N bytes)`.
pub fn truncate(value: &str, max_bytes: usize) -> Truncated<'_> {
    Truncated { value, max_bytes }
}

#[derive(Debug, Clone, Copy)]
pub struct Truncated<'a> {
    value: &'a str,
    max_bytes: usize,
}

impl fmt::Display for Truncated<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.value.len() <= self.max_bytes {
            return f.write_str(self.value);
        }
        let mut end = self.max_bytes;
        while !self.value.is_char_boundary(end) {
            end -= 1;
        }
        write!(
            f,
            "{}…(+{} bytes)",
            &self.value[..end],
            self.value.len() - end
        )
    }
}

/// Display adapter that renders an error and all its sources on one line: `outer: inner: root`.
pub fn chain<'a>(err: &'a (dyn std::error::Error + 'static)) -> ErrorChain<'a> {
    ErrorChain { err }
}

#[derive(Debug, Clone, Copy)]
pub struct ErrorChain<'a> {
    err: &'a (dyn std::error::Error + 'static),
}

impl fmt::Display for ErrorChain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.err)?;
        let mut source = self.err.source();
        while let Some(err) = source {
            write!(f, ": {err}")?;
            source = err.source();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_values() {
        assert_eq!(truncate("abc", 3).to_string(), "abc");
        assert_eq!(truncate("", 0).to_string(), "");
    }

    #[test]
    fn truncate_cuts_at_char_boundary() {
        assert_eq!(truncate("abcdef", 4).to_string(), "abcd…(+2 bytes)");
        // "ä" is two bytes; a cut inside it moves back to the boundary.
        assert_eq!(truncate("aäb", 2).to_string(), "a…(+3 bytes)");
    }

    #[derive(Debug, thiserror::Error)]
    #[error("outer")]
    struct Outer(#[source] Inner);

    #[derive(Debug, thiserror::Error)]
    #[error("inner")]
    struct Inner(#[source] std::io::Error);

    #[test]
    fn chain_joins_all_sources() {
        let err = Outer(Inner(std::io::Error::other("root cause")));
        assert_eq!(chain(&err).to_string(), "outer: inner: root cause");
    }
}
