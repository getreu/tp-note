//! Syntax highlighting for (inline) source code blocks in Markdown input.

use crate::config::EmbeddedContentErrorPolicy;
#[cfg(feature = "mermaid")]
use crate::error::NoteError;
use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
#[cfg(feature = "mermaid")]
use std::cell::RefCell;
#[cfg(feature = "mermaid")]
use std::rc::Rc;
use syntect::highlighting::ThemeSet;
use syntect::html::css_for_theme_with_class_style;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Builds the inline HTML fallback shown in place of a Mermaid diagram that
/// failed to render under the `Inline` error policy. The result is emitted via
/// `Event::Html`, which pulldown-cmark does **not** escape, so both the error
/// message and the offending source are HTML-escaped here.
#[cfg(feature = "mermaid")]
fn mermaid_error_html(msg: &str, code: &str) -> String {
    format!(
        "<div class=\"mermaid-error\"><p><em>Mermaid render error: {}</em></p>\
         <pre><code class=\"language-mermaid\">{}</code></pre></div>",
        html_escape::encode_text(msg),
        html_escape::encode_text(code),
    )
}

/// Get the viewer syntax highlighting CSS configuration.
pub(crate) fn get_highlighting_css(theme_name: &str) -> String {
    let ts = ThemeSet::load_defaults();

    ts.themes
        .get(theme_name)
        .and_then(|theme| {
            css_for_theme_with_class_style(theme, syntect::html::ClassStyle::Spaced).ok()
        })
        .unwrap_or_default()
}

/// A wrapper for a `pulldown_cmark` event iterator.
#[derive(Debug, Default)]
pub struct SyntaxPreprocessor<'a, I: Iterator<Item = Event<'a>>> {
    parent: I,
    /// How a failed embedded renderer (e.g. Mermaid) is surfaced.
    #[cfg(feature = "mermaid")]
    error_policy: EmbeddedContentErrorPolicy,
    /// Side channel for `HardError` mode: `next()` returns `Option<Event>` and
    /// cannot fail, so the first `NoteError` is parked here for
    /// `MarkupLanguage::render()` to pick up. Single-threaded, hence
    /// `Rc<RefCell<…>>`.
    #[cfg(feature = "mermaid")]
    error_sink: Rc<RefCell<Option<NoteError>>>,
}

/// Constructor.
impl<'a, I: Iterator<Item = Event<'a>>> SyntaxPreprocessor<'a, I> {
    #[cfg_attr(not(feature = "mermaid"), allow(unused_variables))]
    pub fn new(parent: I, error_policy: EmbeddedContentErrorPolicy) -> Self {
        Self {
            parent,
            #[cfg(feature = "mermaid")]
            error_policy,
            #[cfg(feature = "mermaid")]
            error_sink: Rc::new(RefCell::new(None)),
        }
    }

    /// Returns a clone of the shared error sink so `MarkupLanguage::render()` can
    /// retrieve a `HardError` captured while the iterator was consumed.
    #[cfg(feature = "mermaid")]
    pub(crate) fn error_sink(&self) -> Rc<RefCell<Option<NoteError>>> {
        self.error_sink.clone()
    }
}

/// Implement `Iterator` for wrapper `SyntaxPreprocessor`.
impl<'a, I: Iterator<Item = Event<'a>>> Iterator for SyntaxPreprocessor<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // Detect inline LaTeX.
        let lang = match self.parent.next()? {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) if !lang.is_empty() => lang,
            // This is the depreciated inline math syntax.
            // It is kept here for backwards compatibility.
            Event::Code(c) if c.len() > 1 && c.starts_with('$') && c.ends_with('$') => {
                return Some(Event::Html(
                    latex2mathml::latex_to_mathml(
                        &c[1..c.len() - 1],
                        latex2mathml::DisplayStyle::Inline,
                    )
                    .unwrap_or_else(|e| e.to_string())
                    .into(),
                ));
            }
            Event::InlineMath(c) => {
                return Some(Event::Html(
                    latex2mathml::latex_to_mathml(c.as_ref(), latex2mathml::DisplayStyle::Inline)
                        .unwrap_or_else(|e| e.to_string())
                        .into(),
                ));
            }
            Event::DisplayMath(c) => {
                return Some(Event::Html(
                    latex2mathml::latex_to_mathml(c.as_ref(), latex2mathml::DisplayStyle::Block)
                        .unwrap_or_else(|e| e.to_string())
                        .into(),
                ));
            }
            other => return Some(other),
        };

        let mut code = String::new();
        let mut event = self.parent.next();
        while let Some(Event::Text(ref code_block)) = event {
            code.push_str(code_block);
            event = self.parent.next();
        }

        debug_assert!(matches!(event, Some(Event::End(TagEnd::CodeBlock))));

        if lang.as_ref() == "math" {
            return Some(Event::Html(
                latex2mathml::latex_to_mathml(&code, latex2mathml::DisplayStyle::Block)
                    .unwrap_or_else(|e| e.to_string())
                    .into(),
            ));
        }

        #[cfg(feature = "mermaid")]
        if lang.as_ref() == "mermaid" {
            // The crate is young (v0.3.x): isolate a possible parser panic so a
            // bad diagram can never abort the viewer thread or the export.
            let result = match std::panic::catch_unwind(|| mermaid_rs_renderer::render(&code)) {
                Ok(r) => r.map_err(|e| e.to_string()),
                Err(_) => Err("internal renderer panic".to_string()),
            };
            return Some(match result {
                // The SVG is inserted un-escaped via `Event::Html` (pulldown-cmark
                // does not escape it). Low practical risk for local, user-authored
                // notes; consistent with the existing raw-HTML passthrough.
                // Sanitization deferred.
                Ok(svg) => Event::Html(format!("<div class=\"mermaid\">{svg}</div>").into()),
                Err(msg) => match self.error_policy {
                    EmbeddedContentErrorPolicy::HardError => {
                        // Park the first error; `render()` turns it into `Err`.
                        let mut sink = self.error_sink.borrow_mut();
                        if sink.is_none() {
                            *sink = Some(NoteError::RenderError {
                                renderer: "Mermaid".to_string(),
                                msg,
                            });
                        }
                        // Discarded — `render()` returns the sink's `Err`.
                        Event::Html(String::new().into())
                    }
                    EmbeddedContentErrorPolicy::Inline => {
                        log::warn!("Mermaid diagram failed to render: {msg}");
                        Event::Html(mermaid_error_html(&msg, &code).into())
                    }
                },
            });
        }

        let mut html = String::with_capacity(code.len() + code.len() * 3 / 2 + 20);

        // Use default syntax styling.
        let ss = SyntaxSet::load_defaults_newlines();
        let sr = match ss.find_syntax_by_token(lang.as_ref()) {
            Some(sr) => {
                html.push_str("<pre><code class=\"language-");
                html.push_str(lang.as_ref());
                html.push_str("\">");
                sr
            }
            None => {
                log::debug!(
                    "renderer: no syntax definition found for: `{}`",
                    lang.as_ref()
                );
                html.push_str("<pre><code>");
                ss.find_syntax_plain_text()
            }
        };
        let mut html_generator =
            ClassedHTMLGenerator::new_with_class_style(sr, &ss, ClassStyle::Spaced);
        for line in LinesWithEndings::from(&code) {
            html_generator
                .parse_html_for_line_which_includes_newline(line)
                .unwrap_or_default();
        }
        html.push_str(html_generator.finalize().as_str());

        html.push_str("</code></pre>");

        Some(Event::Html(html.into()))
    }
}

#[cfg(test)]
mod test {
    #[cfg(feature = "mermaid")]
    use crate::config::EmbeddedContentErrorPolicy;
    use crate::highlight::SyntaxPreprocessor;
    use pulldown_cmark::{Options, Parser, html};

    #[test]
    fn test_latex_math() {
        // Inline math.
        let input: &str = "casual $\\sum_{n=0}^\\infty \\frac{1}{n!}$ text";

        let expected = "<p>casual <math xmlns=";

        let options = Options::all();
        let parser = Parser::new_ext(input, options);
        let processed = SyntaxPreprocessor::new(parser, Default::default());

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        println!("Rendered: {}", rendered);
        assert!(rendered.starts_with(expected));

        //
        // Depreciated inline math.
        // This code might be removed later.
        let input: &str = "casual `$\\sum_{n=0}^\\infty \\frac{1}{n!}$` text";

        let expected = "<p>casual <math xmlns=";

        let options = Options::all();
        let parser = Parser::new_ext(input, options);
        let processed = SyntaxPreprocessor::new(parser, Default::default());

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        assert!(rendered.starts_with(expected));

        //
        // Block math 1
        let input = "text\n$$\nR(X, Y)Z = \\nabla_X\\nabla_Y Z - \
            \\nabla_Y \\nabla_X Z - \\nabla_{[X, Y]} Z\n$$";

        let expected = "<p>text\n\
            <math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\">\
            <mi>R</mi><mo>(</mo><mi>X</mi><mo>,</mo><mi>Y</mi><mo>)</mo>\
            <mi>Z</mi><mo>=</mo><msub><mo>∇</mo><mi>X</mi></msub><msub><mo>∇</mo>\
            <mi>Y</mi></msub><mi>Z</mi><mo>-</mo><msub><mo>∇</mo><mi>Y</mi></msub>\
            <msub><mo>∇</mo><mi>X</mi></msub><mi>Z</mi><mo>-</mo><msub><mo>∇</mo>\
            <mrow><mo>[</mo><mi>X</mi><mo>,</mo><mi>Y</mi><mo>]</mo></mrow></msub>\
            <mi>Z</mi></math></p>\n";

        let options = Options::all();
        let parser = Parser::new_ext(input, options);
        let processed = SyntaxPreprocessor::new(parser, Default::default());

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        assert_eq!(rendered, expected);

        // Block math 2
        let input = "text\n```math\nR(X, Y)Z = \\nabla_X\\nabla_Y Z - \
            \\nabla_Y \\nabla_X Z - \\nabla_{[X, Y]} Z\n```";

        let expected = "<p>text</p>\n\
            <math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\">\
            <mi>R</mi><mo>(</mo><mi>X</mi><mo>,</mo><mi>Y</mi><mo>)</mo>\
            <mi>Z</mi><mo>=</mo><msub><mo>∇</mo><mi>X</mi></msub><msub><mo>∇</mo>\
            <mi>Y</mi></msub><mi>Z</mi><mo>-</mo><msub><mo>∇</mo><mi>Y</mi></msub>\
            <msub><mo>∇</mo><mi>X</mi></msub><mi>Z</mi><mo>-</mo><msub><mo>∇</mo>\
            <mrow><mo>[</mo><mi>X</mi><mo>,</mo><mi>Y</mi><mo>]</mo></mrow></msub>\
            <mi>Z</mi></math>";

        let options = Options::all();
        let parser = Parser::new_ext(input, options);
        let processed = SyntaxPreprocessor::new(parser, Default::default());

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        assert_eq!(rendered, expected);
    }

    #[test]
    fn test_rust_source() {
        let input: &str = "```rust\n\
            fn main() {\n\
                println!(\"Hello, world!\");\n\
            }\n\
            ```";

        let expected = "<pre><code class=\"language-rust\">\
            <span class=\"source rust\">";

        let parser = Parser::new(input);
        let processed = SyntaxPreprocessor::new(parser, Default::default());

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        assert!(rendered.starts_with(expected));
    }

    #[test]
    fn test_plain_text() {
        let input: &str = "```\nSome\nText\n```";

        let expected = "<pre><code>\
            Some\nText\n</code></pre>\n";

        let parser = Parser::new(input);
        let processed = SyntaxPreprocessor::new(parser, Default::default());

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        assert_eq!(rendered, expected);
    }

    #[test]
    fn test_unkown_source() {
        let input: &str = "```abc\n\
            fn main() {\n\
                println!(\"Hello, world!\");\n\
            }\n\
            ```";

        let expected = "<pre><code>\
            <span class=\"text plain\">fn main()";

        let parser = Parser::new(input);
        let processed = SyntaxPreprocessor::new(parser, Default::default());

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        assert!(rendered.starts_with(expected));
    }

    #[test]
    fn test_md() {
        let markdown_input = "# Titel\n\nBody";
        let expected = "<h1>Titel</h1>\n<p>Body</p>\n";

        let options = Options::all();
        let parser = Parser::new_ext(markdown_input, options);
        let parser = SyntaxPreprocessor::new(parser, Default::default());

        // Write to String buffer.
        let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
        html::push_html(&mut html_output, parser);
        assert_eq!(html_output, expected);
    }

    #[test]
    fn test_indented() {
        let markdown_input = r#"
1. test

   ```bash
   wget getreu.net
   echo test
   ```
"#;

        let expected = "<ol>\n<li>\n<p>test</p>\n<pre>\
            <code class=\"language-bash\">\
            <span class=\"source shell bash\">\
            <span class=\"meta function-call shell\">\
            <span class=\"variable function shell\">wget</span></span>";
        let options = Options::all();
        let parser = Parser::new_ext(markdown_input, options);
        let parser = SyntaxPreprocessor::new(parser, Default::default());

        // Write to String buffer.
        let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
        html::push_html(&mut html_output, parser);
        assert!(html_output.starts_with(expected));
    }

    #[cfg(feature = "mermaid")]
    #[test]
    fn test_mermaid_diagram() {
        let input = "```mermaid\ngraph TD\n    A --> B\n```";

        let parser = Parser::new_ext(input, Options::all());
        let parser = SyntaxPreprocessor::new(parser, Default::default());

        let mut rendered = String::new();
        html::push_html(&mut rendered, parser);
        assert!(rendered.contains("<div class=\"mermaid\">"));
        assert!(rendered.contains("<svg"));
    }

    #[cfg(feature = "mermaid")]
    #[test]
    fn test_mermaid_invalid_inline() {
        // Default policy is `Inline`: a malformed diagram must not panic; the
        // output carries the inline error box instead of a diagram.
        let input = "```mermaid\nthis is not a valid mermaid diagram !!!\n```";

        let parser = Parser::new_ext(input, Options::all());
        let parser = SyntaxPreprocessor::new(parser, EmbeddedContentErrorPolicy::Inline);

        let mut rendered = String::new();
        html::push_html(&mut rendered, parser);
        assert!(rendered.contains("mermaid-error"));
    }
}
