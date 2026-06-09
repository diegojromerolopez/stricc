use ariadne::{Color, Label, Report, ReportKind, Source};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn to_range(&self) -> Range<usize> {
        self.start..self.end
    }

    pub fn union(&self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: ReportKind<'static>,
    pub message: String,
    pub span: Option<Span>,
    pub label: Option<String>,
    pub filename: String,
}

impl Diagnostic {
    pub fn error<S: Into<String>>(message: S, filename: &str) -> Self {
        Self {
            severity: ReportKind::Error,
            message: message.into(),
            span: None,
            label: None,
            filename: filename.to_string(),
        }
    }

    pub fn error_with_span<S1: Into<String>, S2: Into<String>>(
        message: S1,
        span: Span,
        label: S2,
        filename: &str,
    ) -> Self {
        Self {
            severity: ReportKind::Error,
            message: message.into(),
            span: Some(span),
            label: Some(label.into()),
            filename: filename.to_string(),
        }
    }

    pub fn warning_with_span<S1: Into<String>, S2: Into<String>>(
        message: S1,
        span: Span,
        label: S2,
        filename: &str,
    ) -> Self {
        Self {
            severity: ReportKind::Warning,
            message: message.into(),
            span: Some(span),
            label: Some(label.into()),
            filename: filename.to_string(),
        }
    }

    pub fn print(&self, source_code: &str) {
        let source_id = self.filename.clone();
        if let Some(span) = self.span {
            let mut report = Report::build(self.severity, source_id.clone(), span.start)
                .with_message(&self.message);

            let label_text = self.label.as_deref().unwrap_or("here");
            let color = match self.severity {
                ReportKind::Error => Color::Red,
                ReportKind::Warning => Color::Yellow,
                _ => Color::White,
            };

            report = report.with_label(
                Label::new((source_id.clone(), span.to_range()))
                    .with_message(label_text)
                    .with_color(color),
            );

            report
                .finish()
                .eprint((source_id, Source::from(source_code)))
                .unwrap_or_else(|e| eprintln!("Error printing diagnostic: {e}"));
        } else {
            let prefix = match self.severity {
                ReportKind::Error => "error",
                ReportKind::Warning => "warning",
                _ => "info",
            };
            eprintln!("{}: {}", prefix, self.message);
        }
    }
}
