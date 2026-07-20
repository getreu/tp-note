//! Syntax highlighting for (inline) source code blocks in Markdown input.

use crate::config::EmbeddedContentErrorPolicy;
#[cfg(any(feature = "mermaid", feature = "latex"))]
use crate::error::NoteError;
use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
#[cfg(any(feature = "mermaid", feature = "latex"))]
use std::cell::RefCell;
#[cfg(any(feature = "mermaid", feature = "latex"))]
use std::rc::Rc;
use syntect::highlighting::ThemeSet;
use syntect::html::css_for_theme_with_class_style;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Marker that `latex2mathml` embeds in otherwise-`Ok` output when a formula
/// fails to parse leniently (e.g. missing arguments, unknown commands) instead
/// of returning `Err`. Its presence is treated as a render failure so such
/// formulas honor the `EmbeddedContentErrorPolicy` too.
#[cfg(feature = "latex")]
const LATEX_PARSE_ERROR_MARKER: &str = "[PARSE ERROR:";

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

/// Builds the inline HTML fallback shown in place of a LaTeX formula that failed
/// to render under the `Inline` error policy. Like `mermaid_error_html`, the
/// output is emitted un-escaped via `Event::Html`, so both parts are escaped.
#[cfg(feature = "latex")]
fn math_error_html(msg: &str, code: &str) -> String {
    format!(
        "<div class=\"math-error\"><p><em>LaTeX render error: {}</em></p>\
         <pre><code class=\"language-math\">{}</code></pre></div>",
        html_escape::encode_text(msg),
        html_escape::encode_text(code),
    )
}

/// Inline counterpart of `math_error_html` for a failed `$…$` formula. A
/// block-level `<div>`/`<pre>` box mid-paragraph is invalid and breaks the text
/// flow, so an inline `<span>` (styled with the same red left border as the
/// block boxes) is used instead.
#[cfg(feature = "latex")]
fn math_error_html_inline(msg: &str, code: &str) -> String {
    format!(
        "<span class=\"math-error-inline\"><em>LaTeX error: {}</em> \
         <code class=\"language-math\">{}</code></span>",
        html_escape::encode_text(msg),
        html_escape::encode_text(code),
    )
}

/// Extracts the first `[PARSE ERROR: …]` marker that `latex2mathml` embeds in
/// otherwise-`Ok` output and pairs it with the source formula, for a useful
/// diagnostic message (shown on the viewer error page / `--export` abort).
#[cfg(feature = "latex")]
fn parse_error_message(mathml: &str, code: &str) -> String {
    let marker = mathml
        .find(LATEX_PARSE_ERROR_MARKER)
        .map(|start| {
            let rest = &mathml[start..];
            let end = rest.find(']').map_or(rest.len(), |i| i + 1);
            &rest[..end]
        })
        .unwrap_or("[PARSE ERROR]");
    format!("{marker} in formula: {code}")
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
    /// How a failed embedded renderer (e.g. Mermaid or LaTeX) is surfaced.
    #[cfg(any(feature = "mermaid", feature = "latex"))]
    error_policy: EmbeddedContentErrorPolicy,
    /// Side channel for `HardError` mode: `next()` returns `Option<Event>` and
    /// cannot fail, so the first `NoteError` is parked here for
    /// `MarkupLanguage::render()` to pick up. Single-threaded, hence
    /// `Rc<RefCell<…>>`.
    #[cfg(any(feature = "mermaid", feature = "latex"))]
    error_sink: Rc<RefCell<Option<NoteError>>>,
}

/// Constructor.
impl<'a, I: Iterator<Item = Event<'a>>> SyntaxPreprocessor<'a, I> {
    #[cfg_attr(
        not(any(feature = "mermaid", feature = "latex")),
        allow(unused_variables)
    )]
    pub fn new(parent: I, error_policy: EmbeddedContentErrorPolicy) -> Self {
        Self {
            parent,
            #[cfg(any(feature = "mermaid", feature = "latex"))]
            error_policy,
            #[cfg(any(feature = "mermaid", feature = "latex"))]
            error_sink: Rc::new(RefCell::new(None)),
        }
    }

    /// Returns a clone of the shared error sink so `MarkupLanguage::render()` can
    /// retrieve a `HardError` captured while the iterator was consumed.
    #[cfg(any(feature = "mermaid", feature = "latex"))]
    pub(crate) fn error_sink(&self) -> Rc<RefCell<Option<NoteError>>> {
        self.error_sink.clone()
    }

    /// Applies the configured `EmbeddedContentErrorPolicy` to a failed embedded
    /// renderer (Mermaid or LaTeX). Under `HardError` the first `NoteError` is
    /// parked in the sink (`render()` turns it into `Err`) and an empty event is
    /// returned; under `Inline` a `log::warn!` is emitted and the caller's
    /// pre-built error box (`inline_html`) is returned. Shared by all embedded
    /// renderers so their error handling stays identical.
    #[cfg(any(feature = "mermaid", feature = "latex"))]
    fn embedded_error_event(
        &self,
        renderer: &str,
        inline_html: String,
        msg: String,
    ) -> Event<'static> {
        match self.error_policy {
            EmbeddedContentErrorPolicy::HardError => {
                // Park the first error; `render()` returns the sink's `Err`.
                let mut sink = self.error_sink.borrow_mut();
                if sink.is_none() {
                    *sink = Some(NoteError::RenderError {
                        renderer: renderer.to_string(),
                        msg,
                    });
                }
                // Discarded — `render()` returns the sink's `Err`.
                Event::Html(String::new().into())
            }
            EmbeddedContentErrorPolicy::Inline => {
                log::warn!("{renderer} failed to render: {msg}");
                Event::Html(inline_html.into())
            }
        }
    }

    /// Renders a LaTeX formula to MathML. On a parse error the configured
    /// `EmbeddedContentErrorPolicy` applies (via `embedded_error_event`), so a
    /// broken formula behaves like a broken Mermaid diagram.
    #[cfg(feature = "latex")]
    fn render_math(&self, latex: &str, style: latex2mathml::DisplayStyle) -> Event<'static> {
        let inline = matches!(style, latex2mathml::DisplayStyle::Inline);
        match latex2mathml::latex_to_mathml(latex, style) {
            // `latex2mathml` is lenient: rather than `Err`, many malformed inputs
            // (missing arguments, unknown commands) return `Ok` carrying an
            // embedded `<mtext>[PARSE ERROR: …]</mtext>` marker. Detect it so such
            // formulas honor the error policy too — otherwise a broken formula
            // would ship silently even under `HardError`. Under `Inline` the
            // marker-annotated MathML is kept (it pinpoints the fault in the
            // formula); under `HardError` the marker text becomes the abort
            // message.
            Ok(mathml) if mathml.contains(LATEX_PARSE_ERROR_MARKER) => {
                let msg = parse_error_message(&mathml, latex);
                // Tag the embedded `<mtext>[PARSE ERROR: …]` marker(s) with a
                // class so the CSS can colour them red where they appear in the
                // formula (only shown under the `Inline` policy).
                let tagged = mathml.replace(
                    &format!("<mtext>{LATEX_PARSE_ERROR_MARKER}"),
                    &format!("<mtext class=\"math-parse-error\">{LATEX_PARSE_ERROR_MARKER}"),
                );
                self.embedded_error_event("LaTeX", tagged, msg)
            }
            Ok(mathml) => Event::Html(mathml.into()),
            Err(e) => {
                let msg = e.to_string();
                let html = if inline {
                    math_error_html_inline(&msg, latex)
                } else {
                    math_error_html(&msg, latex)
                };
                self.embedded_error_event("LaTeX", html, msg)
            }
        }
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
            #[cfg(feature = "latex")]
            Event::Code(c) if c.len() > 1 && c.starts_with('$') && c.ends_with('$') => {
                return Some(self.render_math(&c[1..c.len() - 1], latex2mathml::DisplayStyle::Inline));
            }
            #[cfg(feature = "latex")]
            Event::InlineMath(c) => {
                return Some(self.render_math(c.as_ref(), latex2mathml::DisplayStyle::Inline));
            }
            #[cfg(feature = "latex")]
            Event::DisplayMath(c) => {
                return Some(self.render_math(c.as_ref(), latex2mathml::DisplayStyle::Block));
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

        #[cfg(feature = "latex")]
        if lang.as_ref() == "math" {
            return Some(self.render_math(&code, latex2mathml::DisplayStyle::Block));
        }

        #[cfg(feature = "mermaid")]
        if lang.as_ref() == "mermaid" {
            // The crate is young (v0.3.x): isolate a possible parser panic so a
            // bad diagram can never abort the viewer thread or the export.
            //
            // Work around a renderer defect (mermaid-rs-renderer 0.3.1): node
            // label text is auto-wrapped at every space, not only at explicit
            // `\n`. This mis-sizes nodes and — because Dagre then routes edges
            // against wrong widths — also corrupts inter-node edge paths. The
            // crate exposes no wrap on/off switch (`measure_label` hard-codes
            // `wrap = true`), but its `wrap_line()` only breaks a line once it
            // exceeds `max_label_width_chars * avg_char`; a very large width
            // therefore suppresses auto-wrapping and yields one rendered line
            // per `\n`-delimited segment. Trade-off: genuinely long single-line
            // labels no longer wrap either. Remove once the crate wraps only on
            // explicit `\n` (upstream bug filed).
            const NO_AUTO_WRAP_LABEL_CHARS: usize = 100_000;
            let render = || {
                let mut options = mermaid_rs_renderer::RenderOptions::default();
                options.layout.max_label_width_chars = NO_AUTO_WRAP_LABEL_CHARS;
                mermaid_rs_renderer::render_with_options(&code, options)
            };
            let result = match std::panic::catch_unwind(render) {
                Ok(r) => r.map_err(|e| e.to_string()),
                Err(_) => Err("internal renderer panic".to_string()),
            };
            return Some(match result {
                // The SVG is inserted un-escaped via `Event::Html` (pulldown-cmark
                // does not escape it). Low practical risk for local, user-authored
                // notes; consistent with the existing raw-HTML passthrough.
                // Sanitization deferred.
                Ok(svg) => Event::Html(format!("<div class=\"mermaid\">{svg}</div>").into()),
                Err(msg) => self.embedded_error_event("Mermaid", mermaid_error_html(&msg, &code), msg),
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

    #[cfg(feature = "latex")]
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
