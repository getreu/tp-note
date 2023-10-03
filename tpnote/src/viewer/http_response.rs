//! HTTP response renderer and sender for all documents with one exception:
//! The content type `text/event-stream` is generated in the module
//! `sse_server`.

use super::sse_server::ServerThread;
use crate::config::CFG;
use crate::config::VIEWER_SERVED_MIME_TYPES_MAP;
use crate::viewer::error::ViewerError;
use std::borrow::Cow;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::time::SystemTime;
use tpnote_lib::config::LocalLinkKind;
use tpnote_lib::config::LIB_CFG;
use tpnote_lib::config::LIB_CFG_CACHE;
use tpnote_lib::config::TMPL_HTML_VAR_DOC_ERROR;
use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE;
use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE;
use tpnote_lib::content::Content;
use tpnote_lib::content::ContentString;
use tpnote_lib::filename::NotePath;
use tpnote_lib::html::rewrite_links;
use tpnote_lib::markup_language::MarkupLanguage;
use tpnote_lib::workflow::render_erroneous_content_html;
use tpnote_lib::workflow::render_viewer_html;

/// Content from files are served in chunks.
const TCP_WRITE_BUFFER_SIZE: usize = 0x1000;

/// Time in seconds the browsers should keep static pages in cache.
const MAX_AGE: usize = 604800;

/// Modern browser request a small icon image.
pub const FAVICON: &[u8] = include_bytes!("favicon.ico");
/// The path where the favicon is requested.
pub const FAVICON_PATH: &str = "/favicon.ico";

pub(crate) trait HttpResponse {
    /// Renders the HTTP response and sends it into `self.stream`.
    fn respond(&mut self, request: &str) -> Result<(), ViewerError>;
    /// Read file from `abspath` and insert its content into an HTTP OK
    /// response.
    fn respond_file_ok(
        &mut self,
        abspath: &Path,
        max_age: usize,
        mime_type: &str,
    ) -> Result<(), ViewerError>;
    /// Send and HTTP response with `content`.
    fn respond_content_ok(
        &mut self,
        reqpath: &Path,
        max_age: usize,
        mime_type: &str,
        content: &[u8],
    ) -> Result<(), ViewerError>;
    // fn respond_forbidden(&mut self, reqpath: &Path) -> Result<(), ViewerError>;
    // fn respond_no_content_ok(&mut self) -> Result<(), ViewerError>;
    /// Write HTTP "not found" response.
    fn respond_not_found(&mut self, reqpath: &Path) -> Result<(), ViewerError>;
    /// Write HTTP method "not allowed" response.
    fn respond_method_not_allowed(&mut self, method: &str) -> Result<(), ViewerError>;
    /// Write HTTP method "too many requests" response.
    fn respond_too_many_requests(&mut self) -> Result<(), ViewerError>;
    /// Write HTTP service unavailable response.
    fn respond_service_unavailable(&mut self) -> Result<(), ViewerError>;
    /// Helper function to send HTTP error responses.
    fn respond_http_error(
        &mut self,
        http_error_code: u16,
        html_msg: &str,
        log_msg: &str,
    ) -> Result<(), ViewerError>;

    /// Renders the error page with the `HTML_VIEWER_ERROR_TMPL`.
    /// `abspath` points to the document with markup that should be rendered
    /// to HTML.
    /// The function injects `self.context` before rendering the template.
    fn render_content_and_error(&self, abspath_doc: &Path) -> Result<String, ViewerError>;
}

impl HttpResponse for ServerThread {
    fn respond(&mut self, path: &str) -> Result<(), ViewerError> {
        match path {
            // Serve icon.
            FAVICON_PATH => {
                self.respond_content_ok(
                    Path::new(&FAVICON_PATH),
                    MAX_AGE,
                    "image/x-icon",
                    FAVICON,
                )?;
            }

            // Serve document CSS file.
            TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE => {
                self.respond_content_ok(
                    Path::new(&TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE),
                    MAX_AGE,
                    "text/css",
                    LIB_CFG.read_recursive().tmpl_html.viewer_doc_css.as_bytes(),
                )?;
            }

            // Serve highlighting CSS file.
            TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE => {
                self.respond_content_ok(
                    Path::new(&TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE),
                    MAX_AGE,
                    "text/css",
                    LIB_CFG_CACHE
                        .read_recursive()
                        .viewer_highlighting_css
                        .as_bytes(),
                )?;
            }

            // The client wants the rendered note.
            "/" => {
                // Renders a content page or an error page for the current note.
                // Tera template errors.
                // The contains JavaScript code to subscribe to `EVENT_PATH`, which
                // reloads this document on request of `self.rx`.
                let html = self.render_content_and_error(&self.context.path)?;

                self.respond_content_ok(Path::new("/"), 0, "text/html", html.as_bytes())?;
                // `self.rx` was not used and is dropped here.
            }

            // Serve all other documents.
            _ => {
                // Assert starting with `/`.
                let relpath = Path::new(path);
                if !relpath.starts_with("/") {
                    return Err(ViewerError::UrlMustStartWithSlash);
                }

                //
                // Condition 1: Only serve files that explicitly appear in
                // `self.allowed_urls`.
                let allowed_urls = self.allowed_urls.read_recursive();
                // Is the request in our `allowed_urls` list?
                if !allowed_urls.contains(relpath) {
                    log::warn!(
                            "TCP port local {} to peer {}: target not referenced in note file, rejecting: '{}'",
                            self.stream.local_addr()?.port(),
                            self.stream.peer_addr()?.port(),
                            relpath.to_str().unwrap_or(""),
                        );
                    // Release the `RwLockReadGuard`.
                    drop(allowed_urls);
                    self.respond_not_found(relpath)?;
                    return Ok(());
                }
                // Release the `RwLockReadGuard`.
                drop(allowed_urls);

                // We prepend `root_path` to `abspath` before accessing the file system.
                let abspath = self
                    .context
                    .root_path
                    .join(relpath.strip_prefix("/").unwrap_or(relpath));
                let mut abspath = Cow::Borrowed(abspath.as_path());
                // From here on, we only work with `abspath`.
                #[allow(dropping_references)]
                drop(relpath);

                //
                // Condition 2: Does this link only contain a sort-tag?
                if let Some(sort_tag) = abspath.filename_contains_only_sort_tag_chars() {
                    if let Some(dir) = abspath.parent() {
                        if let Ok(files) = dir.read_dir() {
                            // If more than one file starts with `sort_tag`, retain the
                            // alphabetic first.
                            let mut minimum = PathBuf::new();
                            'file_loop: for file in files.flatten() {
                                let file = file.path();
                                if matches!(
                                    file.extension()
                                        .unwrap_or_default()
                                        .to_str()
                                        .unwrap_or_default()
                                        .into(),
                                    MarkupLanguage::None
                                ) {
                                    continue 'file_loop;
                                }
                                // Does this sort-tag short link correspond to
                                // any sort-tag of a file in the same directory?
                                if file.parent() == abspath.parent()
                                    && file.disassemble().0.starts_with(sort_tag)
                                {
                                    // Before the first assignment `minimum` is empty.
                                    // Finds the minimum.
                                    if minimum == Path::new("") || minimum > file {
                                        minimum = file;
                                    }
                                }
                            } // End of loop.
                            if minimum != Path::new("") {
                                log::info!(
                                    "File `{}` referenced by sort-tag match `{}`.",
                                    minimum.to_str().unwrap_or_default(),
                                    sort_tag,
                                );
                                abspath = Cow::Owned(minimum);
                            }
                        };
                    }
                };
                // From here on `abspath is immutable.`
                let abspath = abspath;

                //
                // Condition 3: Check if we serve this kind of extension
                let extension = &*abspath
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .to_lowercase();

                // Find the corresponding mime type of this file extension.
                let mime_type = VIEWER_SERVED_MIME_TYPES_MAP
                    .get(extension)
                    .map_or_else(|| MarkupLanguage::from(extension).mine_type(), |&mt| mt);

                if mime_type.is_empty() {
                    // Reject all files with extensions not listed.
                    log::warn!(
                        "TCP port local {} to peer {}: \
                                files with extension '{}' are not served. Rejecting: '{}'",
                        self.stream.local_addr()?.port(),
                        self.stream.peer_addr()?.port(),
                        abspath
                            .extension()
                            .unwrap_or_default()
                            .to_str()
                            .unwrap_or_default(),
                        abspath.display(),
                    );
                    self.respond_not_found(&abspath)?;
                    return Ok(());
                };

                //
                // Condition 4: If this is a Tp-Note file, check the maximum
                // of delivered documents, then deliver.
                if !matches!(extension.into(), MarkupLanguage::None) {
                    if abspath.is_file() {
                        let delivered_docs_count =
                            self.delivered_tpnote_docs.read_recursive().len();
                        if delivered_docs_count < CFG.viewer.displayed_tpnote_count_max {
                            let html = self.render_content_and_error(&abspath)?;
                            self.respond_content_ok(&abspath, 0, "text/html", html.as_bytes())?;
                        } else {
                            self.respond_too_many_requests()?;
                        }
                        return Ok(());
                    } else {
                        log::info!("Referenced Tp-Note file not found: {}", abspath.display());
                        self.respond_not_found(&abspath)?;
                        return Ok(());
                    }
                }

                //
                // Condition 5: Is the file readable?
                if abspath.is_file() {
                    self.respond_file_ok(&abspath, 0, mime_type)?;
                } else {
                    self.respond_not_found(&abspath)?;
                }
            }
        }; // end of match path
        Ok(())
    }

    fn respond_file_ok(
        &mut self,
        abspath: &Path,
        max_age: usize,
        mime_type: &str,
    ) -> Result<(), ViewerError> {
        let cache_control = if max_age == 0 {
            "no-cache".to_string()
        } else {
            format!("private, max-age={}", max_age)
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Date: {}\r\n\
             Cache-Control: {}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\r\n",
            httpdate::fmt_http_date(SystemTime::now()),
            cache_control,
            mime_type,
            fs::metadata(abspath)?.len(),
        );
        self.stream.write_all(response.as_bytes())?;

        // Serve file in chunks.
        let mut buffer = [0; TCP_WRITE_BUFFER_SIZE];
        let mut file = fs::File::open(abspath)?;

        while let Ok(n) = file.read(&mut buffer[..]) {
            if n == 0 {
                break;
            };
            self.stream.write_all(&buffer[..n])?;
        }

        log::debug!(
            "TCP port local {} to peer {}: 200 OK, served file: '{}'",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            abspath.display()
        );

        Ok(())
    }

    fn respond_content_ok(
        &mut self,
        reqpath: &Path,
        max_age: usize,
        mime_type: &str,
        content: &[u8],
    ) -> Result<(), ViewerError> {
        let cache_control = if max_age == 0 {
            "no-cache".to_string()
        } else {
            format!("private, max-age={}", max_age)
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Date: {}\r\n\
             Cache-Control: {}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\r\n",
            httpdate::fmt_http_date(SystemTime::now()),
            cache_control,
            mime_type,
            content.len(),
        );
        self.stream.write_all(response.as_bytes())?;
        self.stream.write_all(content)?;
        log::debug!(
            "TCP port local {} to peer {}: 200 OK, served file: '{}'",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            reqpath.display()
        );

        Ok(())
    }

    // /// Write HTTP not found response.
    // fn respond_forbidden(&mut self, reqpath: &Path) -> Result<(), ViewerError> {
    //     self.respond_http_error(403, "Forbidden", &reqpath.display().to_string())
    // }

    // fn respond_no_content_ok(&mut self) -> Result<(), ViewerError> {
    //     self.respond_http_error(204, "", "Ok, served header")
    // }

    fn respond_not_found(&mut self, reqpath: &Path) -> Result<(), ViewerError> {
        self.respond_http_error(404, "Not found", &reqpath.display().to_string())
    }

    fn respond_method_not_allowed(&mut self, method: &str) -> Result<(), ViewerError> {
        self.respond_http_error(405, "Method Not Allowed", method)
    }

    fn respond_too_many_requests(&mut self) -> Result<(), ViewerError> {
        let mut log_msg;
        {
            let delivered_tpnote_docs = self.delivered_tpnote_docs.read_recursive();

            // Prepare the log entry.
            log_msg = format!(
                "Error: too many requests. You have exceeded \n\
            `viewer.displayed_tpnote_count_max = {}` by browsing:\n",
                CFG.viewer.displayed_tpnote_count_max
            );
            for p in delivered_tpnote_docs.iter() {
                log_msg.push_str("- ");
                log_msg.push_str(&p.display().to_string());
                log_msg.push('\n');
            }
        }
        // Prepare the HTML output.
        let content = format!(
            "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"></head>
             <body><h2>Too many requests</h2>
             <p>For security reasons, Tp-Note's internal viewer only displays
             a limited number ({}) of Tp-Note files. This limit can be raised
             by setting the configuration file variable:</p>
            <p> <pre>viewer.displayed_tpnote_count_max</pre></p>
             </body></html>
             ",
            CFG.viewer.displayed_tpnote_count_max
        );

        self.respond_http_error(439, &content, &log_msg)
    }

    fn respond_service_unavailable(&mut self) -> Result<(), ViewerError> {
        self.respond_http_error(503, "Service unavailable", "")
    }

    fn respond_http_error(
        &mut self,
        http_error_code: u16,
        html_msg: &str,
        log_msg: &str,
    ) -> Result<(), ViewerError> {
        let response = format!(
            "HTTP/1.1 {}\r\n\
             Date: {}\r\n\
             Cache-Control: private, no-cache\r\n\
             Content-Type: text/html\r\n\
             Content-Length: {}\r\n\r\n",
            http_error_code,
            httpdate::fmt_http_date(SystemTime::now()),
            html_msg.len(),
        );
        self.stream.write_all(response.as_bytes())?;
        self.stream.write_all(html_msg.as_bytes())?;
        log::debug!(
            "TCP port local {} to peer {}: {} {}: {}",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            http_error_code,
            html_msg,
            log_msg
        );

        Ok(())
    }

    fn render_content_and_error(&self, abspath_doc: &Path) -> Result<String, ViewerError> {
        // First decompose header and body, then deserialize header.
        let content = ContentString::open(abspath_doc)?;
        let abspath_dir = abspath_doc.parent().unwrap_or_else(|| Path::new("/"));
        let root_path = &self.context.root_path;

        // Only the first base document is live updated.
        let mut context = self.context.clone();
        if context.path != abspath_doc {
            context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, "");
        }
        match render_viewer_html::<ContentString>(context, content)
            // Now scan the HTML result for links and store them in a Map
            // accessible to all threads.
            // Secondly, convert all relative links to absolute links.
            .map(|html| {
                rewrite_links(
                    html,
                    root_path,
                    abspath_dir,
                    // Do convert rel. to abs. links.
                    // Do not convert abs. links.
                    LocalLinkKind::Short,
                    // Do not append `.html` to `.md` links.
                    false,
                    // We clone only the RWlock, not the data.
                    self.allowed_urls.clone(),
                )
            }) {
            // If the rendition went well, return the HTML.
            Ok(html) => {
                let mut delivered_tpnote_docs = self.delivered_tpnote_docs.write();
                delivered_tpnote_docs.insert(abspath_doc.to_owned());
                log::debug!(
                    "Viewer: so far served Tp-Note documents: {}",
                    delivered_tpnote_docs
                        .iter()
                        .map(|p| {
                            let mut s = "\n    '".to_string();
                            s.push_str(&p.as_path().display().to_string());
                            s
                        })
                        .collect::<String>()
                );
                Ok(html)
            }
            // We could not render the note properly. Instead we will render a
            // special error page and return this instead.
            Err(e) => {
                // Render error page providing all information we havStringe.
                let mut context = self.context.clone();
                context.insert(TMPL_HTML_VAR_DOC_ERROR, &e.to_string());
                let note_erroneous_content = <ContentString as Content>::open(&context.path)?;
                render_erroneous_content_html::<ContentString>(context, note_erroneous_content)
                    .map_err(|e| ViewerError::RenderErrorPage {
                        tmpl: "tmpl_html.viewer_error".to_string(),
                        source: e,
                    })
            }
        }
    }
}
