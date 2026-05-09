//! Parse the `## Deferred items` section of a HARDENING.md file.
//!
//! Two layouts are recognised:
//!
//! 1. Markdown table:
//!        | Item | Section | Where the TODO lives |
//!        |------|---------|---------------------|
//!        | description | §X | file_ref |
//!
//! 2. Bullet list:
//!        - description (file_ref)
//!
//! Rows whose first cell is wrapped in `~~...~~` are treated as resolved and
//! skipped. Priority is inferred from keywords in the description.

use regie_shared::types::{DeferredItem, DeferredItemPriority};

pub fn parse(repo_id: &str, source: &str) -> Vec<DeferredItem> {
    let Some(start) = find_section_start(source) else {
        return Vec::new();
    };
    let body = end_at_next_h2(&source[start..]);

    let mut out = Vec::new();
    let mut idx: usize = 0;

    for raw in body.lines() {
        let line = raw.trim_start();
        if line.starts_with("|") && !is_table_separator(line) {
            if let Some(item) = parse_table_row(repo_id, idx, line) {
                out.push(item);
                idx += 1;
            }
        } else if let Some(rest) = line.strip_prefix("- ") {
            if let Some(item) = parse_bullet(repo_id, idx, rest) {
                out.push(item);
                idx += 1;
            }
        }
    }

    out
}

fn find_section_start(source: &str) -> Option<usize> {
    // Tolerate any heading level that ends with "Deferred items".
    for (i, line) in source.lines().enumerate() {
        let t = line.trim_start();
        if (t.starts_with("## ") || t.starts_with("### "))
            && t.to_ascii_lowercase().contains("deferred items")
        {
            // Return byte offset of the line AFTER this heading so we don't
            // re-include the heading itself.
            return Some(skip_lines(source, i + 1));
        }
    }
    None
}

fn skip_lines(source: &str, n: usize) -> usize {
    let mut count = 0;
    for (offset, c) in source.char_indices() {
        if count == n {
            return offset;
        }
        if c == '\n' {
            count += 1;
        }
    }
    source.len()
}

fn end_at_next_h2(body: &str) -> &str {
    let mut last_offset = body.len();
    for (i, line) in body.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let t = line.trim_start();
        if t.starts_with("## ") {
            // find the byte offset of this line in body
            let mut acc = 0usize;
            for (k, l) in body.lines().enumerate() {
                if k == i {
                    last_offset = acc;
                    break;
                }
                acc += l.len() + 1;
            }
            break;
        }
    }
    &body[..last_offset]
}

fn is_table_separator(line: &str) -> bool {
    line.chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
        && line.contains('-')
}

fn parse_table_row(repo_id: &str, idx: usize, line: &str) -> Option<DeferredItem> {
    let cells: Vec<&str> = line
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect();
    // Header row?
    if cells
        .first()
        .map(|c| c.eq_ignore_ascii_case("item"))
        .unwrap_or(false)
    {
        return None;
    }
    if cells.is_empty() {
        return None;
    }

    let description = cells[0].trim().to_owned();
    if description.is_empty() {
        return None;
    }
    if is_strikethrough(&description) {
        // Resolved item — skip.
        return None;
    }

    let section = cells.get(1).map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    let file_ref = cells.get(2).map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());

    Some(DeferredItem {
        repo_id: repo_id.to_owned(),
        id: format!("def-{idx:03}"),
        description: clean_inline(description),
        file_ref,
        section,
        priority: infer_priority(cells.first().copied().unwrap_or("")),
    })
}

fn parse_bullet(repo_id: &str, idx: usize, rest: &str) -> Option<DeferredItem> {
    let trimmed = rest.trim();
    if trimmed.is_empty() || is_strikethrough(trimmed) {
        return None;
    }
    // Try to peel a trailing parenthesised file ref: "description (path:line)"
    let (desc, file_ref) = match (trimmed.rfind('('), trimmed.ends_with(')')) {
        (Some(p), true) => {
            let inner = &trimmed[p + 1..trimmed.len() - 1];
            let head = trimmed[..p].trim().to_owned();
            (head, Some(inner.trim().to_owned()))
        }
        _ => (trimmed.to_owned(), None),
    };
    Some(DeferredItem {
        repo_id: repo_id.to_owned(),
        id: format!("def-{idx:03}"),
        description: clean_inline(desc),
        file_ref,
        section: None,
        priority: infer_priority(trimmed),
    })
}

fn is_strikethrough(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("~~") && t.contains("~~")
}

fn clean_inline(mut s: String) -> String {
    // Strip wrapping `~~...~~` if any survive (resolved items already filtered,
    // but a description that mentions a strikethrough mid-text is fine).
    while s.starts_with("**") && s.ends_with("**") && s.len() > 4 {
        s = s[2..s.len() - 2].to_owned();
    }
    s
}

fn infer_priority(text: &str) -> DeferredItemPriority {
    let lower = text.to_ascii_lowercase();
    let high_markers = [
        "must", "production", "blocker", "security", "exploit", "p0",
    ];
    let low_markers = [
        "would let",
        "would be",
        "deferred until",
        "follow-up",
        "nice to have",
        "doc",
    ];
    if high_markers.iter().any(|k| lower.contains(k)) {
        DeferredItemPriority::High
    } else if low_markers.iter().any(|k| lower.contains(k)) {
        DeferredItemPriority::Low
    } else {
        DeferredItemPriority::Medium
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE_SAMPLE: &str = r#"# Hardening

## §10 Operational

- [x] something done

## Deferred items

| Item | Section | Where the TODO lives |
|------|---------|---------------------|
| RLS per-request transaction wiring | §2c | `server/src/app.rs` |
| ~~Stripe billing webhook~~ — shipped | §10 | `domain/src/domain/billing/service.rs` |
| `record_consent` at background-check initiation | §9 | `background_checks/service.rs` |

## Other section
"#;

    const BULLET_SAMPLE: &str = r#"## Deferred items (tracked)

- **§9 record_consent**: function exists, call site is TODO. (`domain/src/domain/background_checks/service.rs`)
- ~~**§10 Stripe billing webhook**~~ — shipped.
- **§11 utility crate**: would let `integrations` import canonical impl.
"#;

    #[test]
    fn table_skips_strikethrough_and_keeps_two() {
        let items = parse("repo", TABLE_SAMPLE);
        assert_eq!(items.len(), 2);
        assert!(items[0].description.starts_with("RLS"));
        assert_eq!(items[0].section.as_deref(), Some("§2c"));
        assert!(items[1].description.contains("record_consent"));
    }

    #[test]
    fn bullets_extract_file_refs_and_skip_strikethrough() {
        let items = parse("repo", BULLET_SAMPLE);
        assert_eq!(items.len(), 2);
        assert!(items[0].file_ref.is_some());
        assert!(matches!(items[1].priority, DeferredItemPriority::Low));
    }
}
