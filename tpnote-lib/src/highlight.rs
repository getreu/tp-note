//! Syntax highlighting for (inline) source code blocks in Markdown input.

use pulldown_cmark::{CodeBlockKind, Event, Tag};
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Get the corresponding `CSS`, which can be inlined or stored in a file.
/// TODO: make this configurable.
pub fn get_css() -> String {
    let ts = ThemeSet::load_defaults();
    let light_theme = &ts.themes["Solarized (light)"];
    css_for_theme_with_class_style(light_theme, syntect::html::ClassStyle::Spaced).unwrap()
}

/// A wraper for a `pulldown_cmark` event iterator.
#[derive(Debug, Default)]
pub struct SyntaxPreprocessor<'a, I: Iterator<Item = Event<'a>>> {
    parent: I,
}

/// Constructor.
impl<'a, I: Iterator<Item = Event<'a>>> SyntaxPreprocessor<'a, I> {
    pub fn new(parent: I) -> Self {
        Self { parent }
    }
}

/// Implement `Iterator` for wrapper `SyntaxPreprocessor`.
impl<'a, I: Iterator<Item = Event<'a>>> Iterator for SyntaxPreprocessor<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let lang = match self.parent.next()? {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => lang,
            // Detect inline LaTeX.
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
            other => return Some(other),
        };

        let mut code = String::new();
        let mut event = self.parent.next();
        while let Some(Event::Text(ref code_block)) = event {
            code.push_str(code_block);
            event = self.parent.next();
        }

        debug_assert!(matches!(event, Some(Event::End(Tag::CodeBlock(_))),));

        if lang.as_ref() == "math" {
            return Some(Event::Html(
                latex2mathml::latex_to_mathml(&code, latex2mathml::DisplayStyle::Block)
                    .unwrap_or_else(|e| e.to_string())
                    .into(),
            ));
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
    use crate::highlight::SyntaxPreprocessor;
    use pulldown_cmark::{html, Options, Parser};

    #[test]
    fn test_latex_math() {
        // Inline math.
        let input: &str = "casual `$\\sum_{n=0}^\\infty \\frac{1}{n!}$` text";

        let expected = "<p>casual <math xmlns=";

        let parser = Parser::new(input);
        let processed = SyntaxPreprocessor::new(parser);

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        assert!(rendered.starts_with(expected));

        // Block math
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

        let parser = Parser::new(input);
        let processed = SyntaxPreprocessor::new(parser);

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
        let processed = SyntaxPreprocessor::new(parser);

        let mut rendered = String::new();
        html::push_html(&mut rendered, processed);
        assert!(rendered.starts_with(expected));
    }

    #[test]
    fn test_plain_text() {
        let input: &str = "```\nSome\nText\n```";

        let expected = "<pre><code><span class=\"text plain\">\
            Some\nText\n</span></code></pre>";

        let parser = Parser::new(input);
        let processed = SyntaxPreprocessor::new(parser);

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
        let processed = SyntaxPreprocessor::new(parser);

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
        let parser = SyntaxPreprocessor::new(parser);

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
        let parser = SyntaxPreprocessor::new(parser);

        // Write to String buffer.
        let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
        html::push_html(&mut html_output, parser);
        assert!(html_output.starts_with(expected));
    }
}
