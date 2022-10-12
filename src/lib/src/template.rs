use crate::config::LIB_CFG;

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub enum TemplateKind {
    New,
    FromClipboardYaml,
    FromClipboard,
    FromTextFile,
    AnnotateFile,
    Sync,
    #[default]
    None,
}

/// TODO
impl TemplateKind {
    /// TODO document.
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
            Self::Sync => String::new(),
            Self::None => String::new(),
        }
    }
    /// TODO
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
            Self::Sync => LIB_CFG.read().unwrap().tmpl.sync_filename.clone(),
            Self::None => String::new(),
        }
    }
}
