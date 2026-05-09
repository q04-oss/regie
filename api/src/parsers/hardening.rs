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
//! Strikethrough handling:
//! - `~~entire description~~` (no text after) → fully resolved, skip.
//! - `~~old framing~~ — active follow-up text` → keep only the text after the
//!   closing `~~`. Useful for HARDENING.md rows that note something *was*
//!   shipped but called out a residual follow-up still owed.
//!
//! Priority is inferred from keywords scanned across the *whole row* (table:
//! all cells joined; bullet: the entire bullet line). Restricting to the first
//! cell would miss hints like "would let X" or "production" living in the
//! `Where the TODO lives` column.

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

    let raw = cells[0].trim();
    if raw.is_empty() {
        return None;
    }
    let description = extract_active(raw)?;

    let section = cells.get(1).map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    let file_ref = cells.get(2).map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());

    // Priority sees every column joined — keywords like "would let" often
    // sit in the `Where the TODO lives` cell, not the title.
    let row_text = cells.join(" ");

    Some(DeferredItem {
        repo_id: repo_id.to_owned(),
        id: format!("def-{idx:03}"),
        description: clean_inline(description),
        file_ref,
        section,
        priority: infer_priority(&row_text),
    })
}

fn parse_bullet(repo_id: &str, idx: usize, rest: &str) -> Option<DeferredItem> {
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return None;
    }
    let active = extract_active(trimmed)?;

    // Try to peel a trailing parenthesised file ref: "description (path:line)"
    let (desc, file_ref) = match (active.rfind('('), active.ends_with(')')) {
        (Some(p), true) => {
            let inner = &active[p + 1..active.len() - 1];
            let head = active[..p].trim().to_owned();
            (head, Some(inner.trim().to_owned()))
        }
        _ => (active.clone(), None),
    };
    // Priority sees the full bullet line — including any pre-strikethrough
    // framing and the file-ref suffix — to maximise keyword recall.
    Some(DeferredItem {
        repo_id: repo_id.to_owned(),
        id: format!("def-{idx:03}"),
        description: clean_inline(desc),
        file_ref,
        section: None,
        priority: infer_priority(trimmed),
    })
}

/// Decide whether a description is fully resolved or has live follow-up
/// work, and return only the live portion.
///
/// - No leading `~~` → return the input as-is.
/// - `~~text~~` with nothing meaningful after → `None` (skip).
/// - `~~text~~ — follow-up work` → strip up to and including the closing
///   `~~`, then strip a leading separator (em-dash, hyphen, colon, or
///   whitespace) and return the residual text.
fn extract_active(desc: &str) -> Option<String> {
    let trimmed = desc.trim();
    if !trimmed.starts_with("~~") {
        return Some(trimmed.to_owned());
    }
    let after_open = &trimmed[2..];
    let close_pos = after_open.find("~~")?;
    let after_close = after_open[close_pos + 2..].trim();
    if after_close.is_empty() {
        return None;
    }
    let stripped = after_close
        .trim_start_matches(|c: char| {
            c == '—' || c == '-' || c == ':' || c.is_whitespace()
        })
        .trim();
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_owned())
    }
}

fn clean_inline(mut s: String) -> String {
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
| ~~Stripe billing webhook~~ — Apple-root x5c chain remains a follow-up | §10 | `domain/src/domain/billing/service.rs` |
| ~~Fully done item~~ | §X | foo |
| `record_consent` at background-check initiation | §9 | `background_checks/service.rs` |

## Other section
"#;

    const BULLET_SAMPLE: &str = r#"## Deferred items (tracked)

- **§9 record_consent**: function exists, call site is TODO. (`domain/src/domain/background_checks/service.rs`)
- ~~**§10 Stripe billing webhook**~~ — shipped Grade A item 3.
- ~~Fully done~~
- **§11 utility crate**: would let `integrations` import canonical impl.
"#;

    #[test]
    fn table_keeps_partial_strike_drops_full_strike() {
        let items = parse("repo", TABLE_SAMPLE);
        // 4 rows: RLS (kept), partial-strike (kept as residual), fully-struck
        // (skipped), record_consent (kept). 3 results expected.
        assert_eq!(items.len(), 3);
        assert!(items[0].description.starts_with("RLS"));
        assert_eq!(items[0].section.as_deref(), Some("§2c"));
        assert_eq!(
            items[1].description,
            "Apple-root x5c chain remains a follow-up"
        );
        assert_eq!(items[1].section.as_deref(), Some("§10"));
        // Priority should pick up "follow-up" from the description column
        // (was previously missed because we only scanned the title cell).
        assert!(matches!(items[1].priority, DeferredItemPriority::Low));
        assert!(items[2].description.contains("record_consent"));
    }

    #[test]
    fn bullets_partial_strike_extracts_active_text() {
        let items = parse("repo", BULLET_SAMPLE);
        // 4 lines: record_consent, partial-strike, fully-struck (skip), utility crate.
        assert_eq!(items.len(), 3);
        assert!(items[0].file_ref.is_some());
        assert_eq!(items[1].description, "shipped Grade A item 3.");
        assert!(matches!(items[2].priority, DeferredItemPriority::Low));
    }
}
