use crate::config::LIB_CFG;

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum TemplateKind {
    New,
    FromClipboardYaml,
    FromClipboard,
    FromTextFile,
    AnnotateFile,
    SyncFilename,
    #[default]
    None,
}

/// TODO
impl TemplateKind {
    /// Returns the content template string as it is defined in the configuration file.
    pub fn get_content_template(&self) -> String {
        match self {
            Self::New => LIB_CFG.read().unwrap().tmpl.new_content.clone(),
            Self::FromClipboardYaml => LIB_CFG
                .read()
                .unwrap()
                .tmpl
                .from_clipboard_yaml_content
                .clone(),
            Self::FromClipboard => LIB_CFG.read().unwrap().tmpl.from_clipboard_content.clone(),
            Self::FromTextFile => LIB_CFG.read().unwrap().tmpl.from_text_file_content.clone(),
            Self::AnnotateFile => LIB_CFG.read().unwrap().tmpl.annotate_file_content.clone(),
            Self::SyncFilename => String::new(),
            Self::None => String::new(),
        }
    }

    /// Returns the content template variable name as it is used in the configuration file.
    pub fn get_content_template_name(&self) -> &str {
        match self {
            Self::New => "[tmpl] new_content",
            Self::FromClipboardYaml => "[tmpl] from_clipboard_yaml_content",
            Self::FromClipboard => "[tmpl] from_clipboard_content",
            Self::FromTextFile => "[tmpl] from_text_file_content",
            Self::AnnotateFile => "[tmpl] annotate_file_content",
            Self::SyncFilename => "error: there is no `sync_content` template",
            Self::None => "error: no content template defined yet",
        }
    }

    /// Returns the file template string as it is defined in the configuration file.
    pub fn get_filename_template(&self) -> String {
        match self {
            Self::New => LIB_CFG.read().unwrap().tmpl.new_filename.clone(),
            Self::FromClipboardYaml => LIB_CFG
                .read()
                .unwrap()
                .tmpl
                .from_clipboard_yaml_filename
                .clone(),
            Self::FromClipboard => LIB_CFG.read().unwrap().tmpl.from_clipboard_filename.clone(),
            Self::FromTextFile => LIB_CFG.read().unwrap().tmpl.from_text_file_filename.clone(),
            Self::AnnotateFile => LIB_CFG.read().unwrap().tmpl.annotate_file_filename.clone(),
            Self::SyncFilename => LIB_CFG.read().unwrap().tmpl.sync_filename.clone(),
            Self::None => String::new(),
        }
    }

    /// Returns the content template variable name as it is used in the configuration file.
    pub fn get_filename_template_name(&self) -> &str {
        match self {
            Self::New => "[tmpl] new_filename",
            Self::FromClipboardYaml => "[tmpl] from_clipboard_yaml_filename",
            Self::FromClipboard => "[tmpl] from_clipboard_filename",
            Self::FromTextFile => "[tmpl] from_text_file_filename",
            Self::AnnotateFile => "[tmpl] annotate_file_filename",
            Self::SyncFilename => "[tmpl] sync_filename",
            Self::None => "error: no filename template defined yet",
        }
    }
}
