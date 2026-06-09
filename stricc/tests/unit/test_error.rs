use ariadne::ReportKind;
use stricc::error::{Diagnostic, Span};

#[test]
fn test_span_new() {
    let span = Span::new(5, 10);
    assert_eq!(span.start, 5);
    assert_eq!(span.end, 10);
}

#[test]
fn test_span_to_range() {
    let span = Span::new(5, 10);
    assert_eq!(span.to_range(), 5..10);
}

#[test]
fn test_span_union() {
    let span1 = Span::new(5, 10);
    let span2 = Span::new(8, 15);
    let union_span = span1.union(span2);
    assert_eq!(union_span.start, 5);
    assert_eq!(union_span.end, 15);

    let span3 = Span::new(12, 20);
    let union_span2 = span1.union(span3);
    assert_eq!(union_span2.start, 5);
    assert_eq!(union_span2.end, 20);
}

#[test]
fn test_diagnostic_error() {
    let diag = Diagnostic::error("simple error message", "main.c");
    assert_eq!(diag.message, "simple error message");
    assert_eq!(diag.filename, "main.c");
    assert!(diag.span.is_none());
    assert!(diag.label.is_none());
    assert!(matches!(diag.severity, ReportKind::Error));
}

#[test]
fn test_diagnostic_error_with_span() {
    let span = Span::new(10, 15);
    let diag = Diagnostic::error_with_span("span error", span, "label description", "main.c");
    assert_eq!(diag.message, "span error");
    assert_eq!(diag.filename, "main.c");
    assert_eq!(diag.span, Some(span));
    assert_eq!(diag.label, Some("label description".to_string()));
    assert!(matches!(diag.severity, ReportKind::Error));
}

#[test]
fn test_diagnostic_warning_with_span() {
    let span = Span::new(20, 25);
    let diag = Diagnostic::warning_with_span("span warning", span, "warning label", "main.c");
    assert_eq!(diag.message, "span warning");
    assert_eq!(diag.filename, "main.c");
    assert_eq!(diag.span, Some(span));
    assert_eq!(diag.label, Some("warning label".to_string()));
    assert!(matches!(diag.severity, ReportKind::Warning));
}

#[test]
fn test_diagnostic_print() {
    // Print simple diagnostic
    let diag1 = Diagnostic::error("basic error", "main.c");
    diag1.print("int main() { return 0; }");

    // Print diagnostic with span
    let span = Span::new(4, 8);
    let diag2 = Diagnostic::error_with_span("expected type", span, "here", "main.c");
    diag2.print("int main() { return 0; }");

    // Print diagnostic with warning severity
    let diag3 = Diagnostic::warning_with_span("unused variable", span, "unused", "main.c");
    diag3.print("int main() { return 0; }");
}
