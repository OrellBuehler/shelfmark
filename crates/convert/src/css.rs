pub type Replacer<'a> = &'a mut dyn FnMut(&str) -> Option<String>;

fn starts_with_ci(s: &[u8], prefix: &str) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn string_end(b: &[u8], start: usize) -> usize {
    let quote = b[start];
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            q if q == quote => return i + 1,
            _ => i += 1,
        }
    }
    b.len()
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn bare(s: &str) -> bool {
    !s.is_empty() && !s.contains(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '(' | ')'))
}

fn write_target(
    out: &mut String,
    quote: Option<char>,
    original: &str,
    replacement: Option<String>,
) {
    match replacement {
        Some(r) if bare(&r) => out.push_str(&r),
        Some(r) => {
            out.push('"');
            out.push_str(&r);
            out.push('"');
        }
        None => match quote {
            Some(q) => {
                out.push(q);
                out.push_str(original);
                out.push(q);
            }
            None => out.push_str(original),
        },
    }
}

fn parse_url(css: &str, open: usize) -> Option<(usize, Option<char>, &str)> {
    let b = css.as_bytes();
    let start = skip_ws(b, open);
    if start >= b.len() {
        return None;
    }
    if b[start] == b'"' || b[start] == b'\'' {
        let end = string_end(b, start);
        let close = skip_ws(b, end);
        if b.get(close) != Some(&b')') || end < start + 2 {
            return None;
        }
        Some((close + 1, Some(b[start] as char), &css[start + 1..end - 1]))
    } else {
        let close = css[start..].find(')')? + start;
        Some((close + 1, None, css[start..close].trim()))
    }
}

pub fn rewrite(css: &str, urls: Replacer, imports: Replacer) -> String {
    let b = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    let mut copied = 0;
    macro_rules! flush {
        ($to:expr) => {
            out.push_str(&css[copied..$to]);
        };
    }
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = css[i + 2..]
                    .find("*/")
                    .map(|n| i + 2 + n + 2)
                    .unwrap_or(b.len());
            }
            b'"' | b'\'' => i = string_end(b, i),
            b'@' if starts_with_ci(&b[i..], "@import") => {
                let after = skip_ws(b, i + 7);
                if after < b.len() && (b[after] == b'"' || b[after] == b'\'') {
                    let end = string_end(b, after);
                    if end >= after + 2 && b[end - 1] == b[after] {
                        let original = &css[after + 1..end - 1];
                        flush!(after);
                        let quote = b[after] as char;
                        out.push(quote);
                        out.push_str(&imports(original).unwrap_or_else(|| original.to_string()));
                        out.push(quote);
                        copied = end;
                    }
                    i = end;
                } else if starts_with_ci(&b[after.min(b.len())..], "url(") {
                    if let Some((end, quote, original)) = parse_url(css, after + 4) {
                        flush!(after);
                        out.push_str("url(");
                        write_target(&mut out, quote, original, imports(original));
                        out.push(')');
                        copied = end;
                        i = end;
                    } else {
                        i = after + 4;
                    }
                } else {
                    i += 7;
                }
            }
            c if (c == b'u' || c == b'U')
                && starts_with_ci(&b[i..], "url(")
                && (i == 0
                    || !(b[i - 1].is_ascii_alphanumeric()
                        || b[i - 1] == b'-'
                        || b[i - 1] == b'_')) =>
            {
                if let Some((end, quote, original)) = parse_url(css, i + 4) {
                    flush!(i);
                    out.push_str("url(");
                    write_target(&mut out, quote, original, urls(original));
                    out.push(')');
                    copied = end;
                    i = end;
                } else {
                    i += 4;
                }
            }
            _ => i += 1,
        }
    }
    out.push_str(&css[copied..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(css: &str) -> String {
        rewrite(
            css,
            &mut |u| {
                u.strip_prefix("img/")
                    .map(|r| format!("kindle:embed:{r}?mime=image/png"))
            },
            &mut |u| (u == "other.css").then(|| "kindle:flow:0002?mime=text/css".to_string()),
        )
    }

    #[test]
    fn rewrites_urls_and_imports() {
        let css = "@import url(other.css); @import 'other.css';\n/* url(img/no) */ a{background:url( 'img/a.png' )}\nb{content:\"url(img/x)\";background:URL(img/b.png) no-repeat;src:url(data:image/png;base64,AA==)}";
        let out = run(css);
        assert!(out.contains("@import url(kindle:flow:0002?mime=text/css);"));
        assert!(
            out.contains("@import \"kindle:flow:0002?mime=text/css\";")
                || out.contains("@import 'kindle:flow:0002?mime=text/css';")
        );
        assert!(out.contains("/* url(img/no) */"));
        assert!(out.contains("url(kindle:embed:a.png?mime=image/png)"));
        assert!(out.contains("content:\"url(img/x)\""));
        assert!(out.contains("url(kindle:embed:b.png?mime=image/png) no-repeat"));
        assert!(out.contains("url(data:image/png;base64,AA==)"));
    }

    #[test]
    fn untouched_css_is_identical() {
        let css =
            "p { margin: 0 } /* x */ h1 { font: 12px \"a b\" } @media print { a { color: red } }";
        assert_eq!(run(css), css);
    }
}
