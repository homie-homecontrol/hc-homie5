/// Cuts a string at a UTF-8 boundary after at most `max_bytes` bytes and appends the omitted
/// size as `…(+N bytes)`.
///
/// Recorded as a plain field (`payload = truncate(&body, PAYLOAD_MAX)`), the text is quoted and
/// newlines are escaped.
pub fn truncate(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…(+{} bytes)", &value[..end], value.len() - end)
}

/// Renders an error and all its sources on one line: `outer: inner: root`.
///
/// A source whose message is already part of the text (an error that embeds its source in its
/// own message) is not repeated.
pub fn chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(err) = source {
        let part = err.to_string();
        if !text.contains(&part) {
            text.push_str(": ");
            text.push_str(&part);
        }
        source = err.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_values() {
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("", 0), "");
    }

    #[test]
    fn truncate_cuts_at_char_boundary() {
        assert_eq!(truncate("abcdef", 4), "abcd…(+2 bytes)");
        // "ä" is two bytes; a cut inside it moves back to the boundary.
        assert_eq!(truncate("aäb", 2), "a…(+3 bytes)");
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
        assert_eq!(chain(&err), "outer: inner: root cause");
    }

    #[derive(Debug, thiserror::Error)]
    #[error("request failed: {0}")]
    struct Embedding(#[source] Inner);

    #[test]
    fn chain_skips_sources_already_in_the_message() {
        let err = Embedding(Inner(std::io::Error::other("root cause")));
        assert_eq!(chain(&err), "request failed: inner: root cause");
    }
}
