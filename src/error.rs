use crate::range::Range;
use std::fmt::{self, Display, Formatter};

pub struct Report {
    pub message: String,
    pub range: Range,
}

impl Report {
    pub fn display<'a, 'b>(&'a self, source: &'b str) -> ReportDisplay<'a, 'b> {
        ReportDisplay {
            report: self,
            source,
        }
    }
}

pub struct ReportDisplay<'a, 'b> {
    report: &'a Report,
    source: &'b str,
}

fn line_number(source: &str, offset: usize) -> usize {
    source[..offset].chars().filter(|&c| c == '\n').count()
}

fn line_start(source: &str, offset: usize) -> usize {
    source[..=offset].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(source: &str, offset: usize) -> usize {
    source[offset..]
        .find('\n')
        .map_or(source.len(), |i| offset + i)
}

fn lines(source: &str, range: Range) -> Vec<&str> {
    let start = line_start(source, range.0);
    let end = line_end(source, range.1);
    source[start..end].lines().collect()
}

fn lines_format(source: &str, range: Range) -> Vec<String> {
    let first_line_number = line_number(source, range.0);
    let last_line_number = line_number(source, range.1 - 1);
    let width = last_line_number.to_string().len();

    if first_line_number == last_line_number {
        return vec![
            format!(
                "{:width$} |",
                "",
                width = width
            ),
            format!(
                "{:width$} | {}",
                first_line_number + 1,
                lines(source, range)[0],
                width = width
            ),
            format!(
                "{} | {}{}",
                " ".repeat(width),
                " ".repeat(range.0 - line_start(source, range.0)),
                "^".repeat(range.1 - range.0)
            ),
        ];
    }

    lines(source, range)
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            format!(
                "{:width$} >| {}",
                first_line_number + i + 1,
                line,
                width = width
            )
        })
        .collect::<Vec<_>>()
}

impl Display for ReportDisplay<'_, '_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "error: {}", self.report.message)?;
        lines_format(self.source, self.report.range).iter().try_for_each(|line| {
            writeln!(f, "{}", line)?;
            Ok(())
        })?;
        Ok(())
    }
}
