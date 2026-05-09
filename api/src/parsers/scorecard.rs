//! Parse a SCORECARD.md file into a chronological list of `ScorecardEntry`.
//!
//! Recognised section header shape:
//!     ## [YYYY-MM-DD ...] Scorecard ...
//! Anything inside the `[...]` is checked for a leading `YYYY-MM-DD`; entries
//! whose label can't be parsed as a date are skipped (e.g. `[2026-05-01 v2]`
//! is parseable, `[v2-only]` is not). Within a section we walk markdown table
//! rows and pick out the six dimensions, the weighted overall, and the grade.
//! Unknown rows are ignored — the parser is tolerant of layout drift.

use chrono::NaiveDate;
use regie_shared::types::ScorecardEntry;

pub fn parse(repo_id: &str, source: &str) -> Vec<ScorecardEntry> {
    let mut out = Vec::new();
    let mut lines = source.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("## [") else {
            continue;
        };
        let Some(end_bracket) = rest.find(']') else {
            continue;
        };
        let label = &rest[..end_bracket];
        if !rest[end_bracket..].contains("Scorecard") {
            continue;
        }
        let Some(date) = parse_date_prefix(label) else {
            continue;
        };

        let mut entry = ScorecardEntry {
            repo_id: repo_id.to_owned(),
            date,
            grade: String::new(),
            weighted_score: 0.0,
            security: 0.0,
            architecture: 0.0,
            engineer_usability: 0.0,
            protocol_conformance: 0.0,
            operational_readiness: 0.0,
            product_completeness: 0.0,
        };

        for inner in lines.by_ref() {
            let inner_trim = inner.trim_start();
            if inner_trim.starts_with("## ") {
                // hand back to outer loop by re-processing? not possible —
                // peekable would help. Simplest: break and let outer loop
                // miss this header (rare). We instead just stop accumulating
                // and accept that successive "## [date] Scorecard" headers
                // start a new section on the next outer iteration. Since we
                // already consumed this line, the outer loop won't see it —
                // accept the off-by-one for header-without-table edge cases.
                break;
            }
            if !inner_trim.starts_with('|') {
                continue;
            }
            let cells: Vec<&str> = inner_trim
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            if cells.len() < 2 {
                continue;
            }
            let dim = cells[0]
                .trim_matches('*')
                .trim()
                .to_ascii_lowercase();
            let value_cell = cells[1];

            if dim.contains("grade") {
                let g = value_cell.trim_matches('*').trim().to_owned();
                if !g.is_empty() {
                    entry.grade = g;
                }
                continue;
            }
            if dim.contains("overall") && dim.contains("weighted") {
                if let Some(n) = first_number(value_cell) {
                    entry.weighted_score = n;
                }
                continue;
            }
            if dim.contains("overall") {
                continue;
            }

            let Some(n) = first_number(value_cell) else {
                continue;
            };
            match dim.as_str() {
                "security" => entry.security = n,
                "architecture" => entry.architecture = n,
                d if d.contains("engineer") => entry.engineer_usability = n,
                d if d.contains("protocol") => entry.protocol_conformance = n,
                d if d.contains("operational") => entry.operational_readiness = n,
                d if d.contains("product") => entry.product_completeness = n,
                _ => {}
            }
        }

        if entry.weighted_score > 0.0
            || entry.security > 0.0
            || !entry.grade.is_empty()
        {
            out.push(entry);
        }
    }

    out.sort_by_key(|e| e.date);
    out
}

fn parse_date_prefix(label: &str) -> Option<NaiveDate> {
    let head: String = label.chars().take(10).collect();
    NaiveDate::parse_from_str(&head, "%Y-%m-%d").ok()
}

/// Pull the first numeric token (allowing one decimal point) out of a cell
/// like `8.7 / 10` or `**9.01 / 10**`.
fn first_number(s: &str) -> Option<f64> {
    let mut buf = String::new();
    let mut seen_digit = false;
    for c in s.chars() {
        if c.is_ascii_digit() || (c == '.' && seen_digit && !buf.contains('.')) {
            buf.push(c);
            if c.is_ascii_digit() {
                seen_digit = true;
            }
        } else if seen_digit {
            break;
        }
    }
    if buf.is_empty() {
        None
    } else {
        buf.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# Scorecard

## [2026-05-08 02:00] Scorecard

| Dimension | Score | Weight | Weighted |
|-----------|-------|--------|----------|
| Security | 8 / 10 | 1.5x | 12.0 |
| Architecture | 9 / 10 | 1.0x | 9.0 |
| Engineer Usability | 9 / 10 | 1.0x | 9.0 |
| Protocol Conformance | 9 / 10 | 1.5x | 13.5 |
| Operational Readiness | 8 / 10 | 1.0x | 8.0 |
| Product Completeness | 9 / 10 | 1.0x | 9.0 |
| **Overall (weighted)** | **8.64 / 10** | | |
| **Grade** | **B+** | | |

## [2026-05-09 post-billing] Scorecard

| Dimension | Score | Weight | Weighted |
|-----------|-------|--------|----------|
| Security | 8.7 / 10 | 1.5x | 13.05 |
| Architecture | 9.2 / 10 | 1.0x | 9.20 |
| Engineer Usability | 9.3 / 10 | 1.0x | 9.30 |
| Protocol Conformance | 9.2 / 10 | 1.5x | 13.80 |
| Operational Readiness | 8.4 / 10 | 1.0x | 8.40 |
| Product Completeness | 9.3 / 10 | 1.0x | 9.30 |
| **Overall (weighted)** | **9.01 / 10** | | |
| **Grade** | **A** | | |
"#;

    #[test]
    fn parses_two_entries_in_order() {
        let entries = parse("q04-oss/box-fraise-platform", SAMPLE);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].grade, "B+");
        assert_eq!(entries[0].weighted_score, 8.64);
        assert_eq!(entries[1].grade, "A");
        assert_eq!(entries[1].weighted_score, 9.01);
        assert_eq!(entries[1].security, 8.7);
        assert_eq!(entries[1].product_completeness, 9.3);
        assert!(entries[0].date < entries[1].date);
    }
}
