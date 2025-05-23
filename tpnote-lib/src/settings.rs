//! Configuration data that origins from environment variables.
//! Unlike the configuration data in `LIB_CFG` which is sourced only once when
//! Tp-Note is launched, the `SETTINGS` object may be sourced more often in
//! order to follow changes in the related environment variables.

use crate::config::{GetLang, LIB_CFG, Mode};
use crate::error::LibCfgError;
#[cfg(feature = "lang-detection")]
use lingua;
#[cfg(feature = "lang-detection")]
use lingua::IsoCode639_1;
use parking_lot::RwLock;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::env;
#[cfg(feature = "lang-detection")]
use std::str::FromStr;
#[cfg(target_family = "windows")]
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
#[cfg(target_family = "windows")]
use windows_sys::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

/// The name of the environment variable which can be optionally set to
/// overwrite the `scheme_default` configuration file setting.
pub const ENV_VAR_TPNOTE_SCHEME: &str = "TPNOTE_SCHEME";

/// The name of the environment variable which can be optionally set to
/// overwrite the `filename.extension_default` configuration file setting.
pub const ENV_VAR_TPNOTE_EXTENSION_DEFAULT: &str = "TPNOTE_EXTENSION_DEFAULT";

/// Name of the environment variable, that can be optionally
/// used to overwrite the user's default language setting, which is
/// accessible as `{{ lang }}` template variable and used in various
/// templates.
pub const ENV_VAR_TPNOTE_LANG: &str = "TPNOTE_LANG";

/// A pseudo language tag for the `get_lang_filter`. When placed in the
/// `ENV_VAR_TPNOTE_LANG` list, all available languages are selected.
pub const ENV_VAR_TPNOTE_LANG_PLUS_ALL: &str = "+all";

/// Name of the environment variable, that can be optionally
/// used to overwrite the user's `tmpl.filter.get_lang.language_candidates`
/// and `tmpl.filter.map_lang` configuration file setting.
pub const ENV_VAR_TPNOTE_LANG_DETECTION: &str = "TPNOTE_LANG_DETECTION";

/// Name of the environment variable, that can be optionally
/// used to overwrite the user's login name. The result is accessible as
/// `{{ username }}` template variable and used in various templates.
pub const ENV_VAR_TPNOTE_USER: &str = "TPNOTE_USER";

/// Name of the `LOGNAME` environment variable.
const ENV_VAR_LOGNAME: &str = "LOGNAME";

/// Name of the `USERNAME` environment variable.
const ENV_VAR_USERNAME: &str = "USERNAME";

/// Name of the `USER` environment variable.
const ENV_VAR_USER: &str = "USER";

/// Name of the `LANG` environment variable.
#[cfg(not(target_family = "windows"))]
const ENV_VAR_LANG: &str = "LANG";

/// Struct containing additional user configuration read from or depending
/// on environment variables.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct Settings {
    /// This is the index as the schemes are listed in the config file.
    pub current_scheme: usize,
    /// This has the format of a login name.
    pub author: String,
    /// Contains the content of the environment variable `TPNOTE_LANG` or if not
    /// defined- `LANG` into `SETTINGS.lang`
    /// as [RFC 5646, Tags for the Identification of Languages](http://www.rfc-editor.org/rfc/rfc5646.txt).
    /// This will be injected as `lang` variable into content templates.
    pub lang: String,
    /// The `force_lang` option of this API. When empty no language forcing
    /// happens (implemented in templates).
    /// This will be injected as `force_lang` variable into content templates.
    pub force_lang: String,
    /// Extension without dot, e.g. `md`
    pub extension_default: String,
    /// See definition of type.
    pub get_lang_filter: GetLang,
    /// The keys and values from
    /// `LIB_CFG.schemes[settings.current_scheme].tmpl.filter_btmap_lang` in the `BTreeMap`
    /// with the user's default language and region added.
    pub map_lang_filter_btmap: Option<BTreeMap<String, String>>,
}

const DEFAULT_SETTINGS: Settings = Settings {
    current_scheme: 0,
    author: String::new(),
    lang: String::new(),
    force_lang: String::new(),
    extension_default: String::new(),
    get_lang_filter: GetLang {
        mode: Mode::Disabled,
        language_candidates: vec![],
        relative_distance_min: 0.0,
        consecutive_words_min: 0,
        words_total_percentage_min: 0,
    },
    map_lang_filter_btmap: None,
};

impl Default for Settings {
    #[cfg(not(any(test, doc)))]
    /// Defaults to empty lists and values.
    fn default() -> Self {
        DEFAULT_SETTINGS
    }

    #[cfg(any(test, doc))]
    /// Defaults to test values.
    /// Do not use outside of tests.
    fn default() -> Self {
        let mut settings = DEFAULT_SETTINGS;
        settings.author = String::from("testuser");
        settings.lang = String::from("ab-AB");
        settings.extension_default = String::from("md");
        settings
    }
}

/// Global mutable variable of type `Settings`.
#[cfg(not(test))]
pub(crate) static SETTINGS: RwLock<Settings> = RwLock::new(DEFAULT_SETTINGS);

#[cfg(test)]
/// Global default for `SETTINGS` in test environments.
pub(crate) static SETTINGS: RwLock<Settings> = RwLock::new(DEFAULT_SETTINGS);

/// Like `Settings::update`, with `scheme_source = SchemeSource::Force("default")`
/// and `force_lang = None`.
/// This is used in doctests only.
pub fn set_test_default_settings() -> Result<(), LibCfgError> {
    let mut settings = SETTINGS.write();
    settings.update(SchemeSource::Force("default"), None)
}

/// How should `update_settings` collect the right scheme?
#[derive(Debug, Clone)]
pub(crate) enum SchemeSource<'a> {
    /// Ignore environment and configuration, take this.
    Force(&'a str),
    /// Take the value `lib_cfg.scheme_sync_default`.
    SchemeSyncDefault,
    /// Take env. var. `TPNOTE_SCHEME` or -if not defined- take this.
    SchemeNewDefault(&'a str),
}

impl Settings {
    /// (Re)read environment variables and store them in the global `SETTINGS`
    /// object. Some data originates from `LIB_CFG`.
    /// First set `self.current_scheme`:
    /// 1. If `SchemeSource::Force(Some(scheme))`, take `scheme`,
    ///    or,
    /// 2. if `SchemeSource::SchemeSyncDefault`, take `lib_cfg.scheme_sync_default`,
    ///    or,
    /// 3. if `SchemeSource::SchemeNewDefault(s)` take the environment variable
    ///    `TPNOTE_SCHEME`,
    ///    or, -if the above environment variable is not defined or empty-
    /// 4. take `s`.
    ///
    /// Then, it set all other fields.
    /// Reads the environment variable `TPNOTE_LANG` or if not defined- `LANG`
    /// -into `SETTINGS.lang`.
    /// If `force_lang` is `Some` then copy it into `SETTINGS.force_lang`.
    /// If `force_lang` is `Some("")` then copy `Settings.lang` into
    /// `SETTINGS.force_lang`.
    pub(crate) fn update(
        &mut self,
        scheme_source: SchemeSource,
        // TODO: `force_lang=Some("")` is a sentinal value in this API and
        // should be replaced by a proper enum variant.
        force_lang: Option<&str>,
    ) -> Result<(), LibCfgError> {
        self.update_current_scheme(scheme_source)?;
        self.update_author();
        self.update_extension_default();
        self.update_lang(force_lang);
        self.update_get_lang_filter();
        self.update_map_lang_filter_btmap();
        self.update_env_lang_detection();

        log::trace!(
            "`SETTINGS` updated (reading config + env. vars.):\n{:#?}",
            self
        );

        if let Mode::Error(e) = &self.get_lang_filter.mode {
            Err(e.clone())
        } else {
            Ok(())
        }
    }

    /// Set `SETTINGS.current_scheme`:
    /// 1. If `SchemeSource::Force(Some(scheme))`, take `scheme`,
    ///    or,
    /// 2. if `SchemeSource::SchemeSyncDefault`, take `lib_cfg.scheme_sync_default`,
    ///    or,
    /// 3. if `SchemeSource::SchemeNewDefault(s)` take the environment variable
    ///    `TPNOTE_SCHEME`,
    ///    or, -if the above environment variable is not defined or empty-
    /// 4. take `s`.
    pub(crate) fn update_current_scheme(
        &mut self,
        scheme_source: SchemeSource,
    ) -> Result<(), LibCfgError> {
        let lib_cfg = LIB_CFG.read_recursive();

        let scheme = match scheme_source {
            SchemeSource::Force(s) => Cow::Borrowed(s),
            SchemeSource::SchemeSyncDefault => Cow::Borrowed(&*lib_cfg.scheme_sync_default),
            SchemeSource::SchemeNewDefault(s) => match env::var(ENV_VAR_TPNOTE_SCHEME) {
                Ok(ed_env) if !ed_env.is_empty() => Cow::Owned(ed_env),
                Err(_) | Ok(_) => Cow::Borrowed(s),
            },
        };
        self.current_scheme = lib_cfg.scheme_idx(scheme.as_ref())?;
        Ok(())
    }

    /// Set `SETTINGS.author` to content of the first not empty environment
    /// variable: `TPNOTE_USER`, `LOGNAME` or `USER`.
    fn update_author(&mut self) {
        let author = env::var(ENV_VAR_TPNOTE_USER).unwrap_or_else(|_| {
            env::var(ENV_VAR_LOGNAME).unwrap_or_else(|_| {
                env::var(ENV_VAR_USERNAME)
                    .unwrap_or_else(|_| env::var(ENV_VAR_USER).unwrap_or_default())
            })
        });

        // Store result.
        self.author = author;
    }

    /// Read the environment variable `TPNOTE_EXTENSION_DEFAULT` or -if empty-
    /// the configuration file variable `filename.extension_default` into
    /// `SETTINGS.extension_default`.
    fn update_extension_default(&mut self) {
        // Get the environment variable if it exists.
        let ext = match env::var(ENV_VAR_TPNOTE_EXTENSION_DEFAULT) {
            Ok(ed_env) if !ed_env.is_empty() => ed_env,
            Err(_) | Ok(_) => {
                let lib_cfg = LIB_CFG.read_recursive();
                lib_cfg.scheme[self.current_scheme]
                    .filename
                    .extension_default
                    .to_string()
            }
        };
        self.extension_default = ext;
    }

    /// Reads the environment variable `TPNOTE_LANG` or if not defined- `LANG`
    /// -into `SETTINGS.lang`.
    /// If `force_lang` is `Some` then copy it into `SETTINGS.force_lang`.
    /// If `force_lang` is `Some("")` then copy `Settings.lang` into
    /// `SETTINGS.force_lang`.
    ///
    fn update_lang(&mut self, force_lang: Option<&str>) {
        // Get the user's language tag.
        // [RFC 5646, Tags for the Identification of Languages](http://www.rfc-editor.org/rfc/rfc5646.txt)
        let mut lang = String::new();
        // Get the environment variable if it exists.
        let tpnotelang = env::var(ENV_VAR_TPNOTE_LANG).ok();
        // Unix/MacOS version.
        #[cfg(not(target_family = "windows"))]
        if let Some(tpnotelang) = tpnotelang {
            lang = tpnotelang;
        } else {
            // [Linux: Define Locale and Language Settings -
            // ShellHacks](https://www.shellhacks.com/linux-define-locale-language-settings/)
            if let Ok(lang_env) = env::var(ENV_VAR_LANG) {
                if !lang_env.is_empty() {
                    // [ISO 639](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes) language code.
                    let mut language = "";
                    // [ISO 3166](https://en.wikipedia.org/wiki/ISO_3166-1#Current_codes) country code.
                    let mut territory = "";
                    if let Some((l, lang_env)) = lang_env.split_once('_') {
                        language = l;
                        if let Some((t, _codeset)) = lang_env.split_once('.') {
                            territory = t;
                        }
                    }
                    lang = language.to_string();
                    lang.push('-');
                    lang.push_str(territory);
                }
            }
        }

        // Get the user's language tag.
        // Windows version.
        #[cfg(target_family = "windows")]
        if let Some(tpnotelang) = tpnotelang {
            lang = tpnotelang;
        } else {
            let mut buf = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
            let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
            if len > 0 {
                lang = String::from_utf16_lossy(&buf[..((len - 1) as usize)]);
            }
        };

        // Store result.
        self.lang = lang;

        // Store `force_lang` argument.
        self.force_lang = match force_lang {
            Some("") => self.lang.clone(),
            Some(lang) => lang.to_owned(),
            None => String::new(),
        };
    }

    /// Copies the settings form
    /// `LIB_CFG.schemes[settings.scheme].tmpl.filter.get_lang` into
    /// `SETTINGS.get_lang_filter`. Then append the user's
    /// default language subtag to
    /// `SETTINGS.get_lang_filter.language_candidates`.
    /// Errors are stored in the `SETTINGS.filter.get_lang.mode` `Mode::Error(e)` variant.
    #[cfg(feature = "lang-detection")]
    fn update_get_lang_filter(&mut self) {
        use crate::config::Mode;

        {
            let lib_cfg = LIB_CFG.read_recursive();
            let current_scheme = &lib_cfg.scheme[self.current_scheme];

            // Start form config.
            self.get_lang_filter = current_scheme.tmpl.filter.get_lang.clone();
        } // Release lock.

        // Check if disabled in config file. Early return.
        if matches!(self.get_lang_filter.mode, Mode::Disabled) {
            return;
        }

        // Read ISO codes from config object.
        let iso_codes = &mut self.get_lang_filter.language_candidates;

        // Check if all languages are selected, then we can return early.
        if iso_codes.is_empty() {
            return;
        }

        // Add the user's language subtag as reported from the OS.
        // Silently ignore if anything goes wrong here.
        if !self.lang.is_empty() {
            if let Some((lang_subtag, _)) = self.lang.split_once('-') {
                if let Ok(iso_code) = IsoCode639_1::from_str(lang_subtag) {
                    if !iso_codes.contains(&iso_code) {
                        iso_codes.push(iso_code);
                    }
                }
            }
        }

        // Check if there are at least 2 languages in the list.
        if iso_codes.len() <= 1 {
            self.get_lang_filter.mode = Mode::Error(LibCfgError::NotEnoughLanguageCodes {
                language_code: iso_codes[0].to_string(),
            })
        }
    }

    #[cfg(not(feature = "lang-detection"))]
    /// Disable filter.
    fn update_get_lang_filter(&mut self) {
        self.get_lang_filter.mode = Mode::Disabled;
    }

    /// Read keys and values from
    /// `LIB_CFG.schemes[self.current_scheme].tmpl.filter_btmap_lang` in the
    /// `BTreeMap`. Add the user's default language and region.
    fn update_map_lang_filter_btmap(&mut self) {
        let mut btm = BTreeMap::new();
        let lib_cfg = LIB_CFG.read_recursive();
        for l in &lib_cfg.scheme[self.current_scheme].tmpl.filter.map_lang {
            if l.len() >= 2 {
                btm.insert(l[0].to_string(), l[1].to_string());
            };
        }
        // Insert the user's default language and region in the Map.
        if !self.lang.is_empty() {
            if let Some((lang_subtag, _)) = self.lang.split_once('-') {
                // Do not overwrite existing languages.
                if !lang_subtag.is_empty() && !btm.contains_key(lang_subtag) {
                    btm.insert(lang_subtag.to_string(), self.lang.to_string());
                }
            };
        }

        // Store result.
        self.map_lang_filter_btmap = Some(btm);
    }

    /// Reads the environment variable `LANG_DETECTION`. If not empty,
    /// parse the content and overwrite the `self.get_lang_filter` and the
    /// `self.map_lang_filter` variables.
    /// Finally, if `force_lang` is true, then it sets
    /// `self.get_lang_filter.mode` Mode::Disabled.
    #[cfg(feature = "lang-detection")]
    fn update_env_lang_detection(&mut self) {
        use crate::config::Mode;

        if let Ok(env_var) = env::var(ENV_VAR_TPNOTE_LANG_DETECTION) {
            if env_var.is_empty() {
                // Early return.
                self.get_lang_filter.mode = Mode::Disabled;
                self.map_lang_filter_btmap = None;
                log::debug!(
                    "Empty env. var. `{}` disables the `lang-detection` feature.",
                    ENV_VAR_TPNOTE_LANG_DETECTION
                );
                return;
            }

            // Read and convert ISO codes from config object.
            let mut hm: BTreeMap<String, String> = BTreeMap::new();
            let mut all_languages_selected = false;
            let iso_codes = env_var
                .split(',')
                .map(|t| {
                    let t = t.trim();
                    if let Some((lang_subtag, _)) = t.split_once('-') {
                        // Do not overwrite existing languages.
                        if !lang_subtag.is_empty() && !hm.contains_key(lang_subtag) {
                            hm.insert(lang_subtag.to_string(), t.to_string());
                        };
                        lang_subtag
                    } else {
                        t
                    }
                })
                // Check if this is the pseudo tag `TMPL_GET_LANG_filter_ALL `.
                .filter(|&l| {
                    if l == ENV_VAR_TPNOTE_LANG_PLUS_ALL {
                        all_languages_selected = true;
                        // Skip this string.
                        false
                    } else {
                        // Continue.
                        true
                    }
                })
                .map(|l| {
                    IsoCode639_1::from_str(l.trim()).map_err(|_| {
                        // The error path.
                        // Produce list of all available languages.
                        let mut all_langs = lingua::Language::all()
                            .iter()
                            .map(|l| {
                                let mut s = l.iso_code_639_1().to_string();
                                s.push_str(", ");
                                s
                            })
                            .collect::<Vec<String>>();
                        all_langs.sort();
                        let mut all_langs = all_langs.into_iter().collect::<String>();
                        all_langs.truncate(all_langs.len() - ", ".len());
                        // Insert data into error object.
                        LibCfgError::ParseLanguageCode {
                            language_code: l.into(),
                            all_langs,
                        }
                    })
                })
                .collect::<Result<Vec<IsoCode639_1>, LibCfgError>>();

            match iso_codes {
                // The happy path.
                Ok(mut iso_codes) => {
                    // Add the user's language subtag as reported from the OS.
                    // Continue the happy path.
                    if !self.lang.is_empty() {
                        if let Some(lang_subtag) = self.lang.split('-').next() {
                            if let Ok(iso_code) = IsoCode639_1::from_str(lang_subtag) {
                                if !iso_codes.contains(&iso_code) {
                                    iso_codes.push(iso_code);
                                }
                                // Check if there is a remainder (region code).
                                if lang_subtag != self.lang && !hm.contains_key(lang_subtag) {
                                    hm.insert(lang_subtag.to_string(), self.lang.to_string());
                                }
                            }
                        }
                    }

                    // Store result.
                    if all_languages_selected {
                        self.get_lang_filter.language_candidates = vec![];
                        if matches!(self.get_lang_filter.mode, Mode::Disabled) {
                            self.get_lang_filter.mode = Mode::Multilingual;
                        }
                    } else {
                        match iso_codes.len() {
                            0 => self.get_lang_filter.mode = Mode::Disabled,
                            1 => {
                                self.get_lang_filter.mode =
                                    Mode::Error(LibCfgError::NotEnoughLanguageCodes {
                                        language_code: iso_codes[0].to_string(),
                                    })
                            }
                            _ => {
                                self.get_lang_filter.language_candidates = iso_codes;
                                if matches!(self.get_lang_filter.mode, Mode::Disabled) {
                                    self.get_lang_filter.mode = Mode::Multilingual;
                                }
                            }
                        }
                    }
                    self.map_lang_filter_btmap = Some(hm);
                }
                // The error path.
                Err(e) =>
                // Store error.
                {
                    self.get_lang_filter.mode = Mode::Error(e);
                }
            }
        }
    }

    /// Ignore the environment variable `LANG_DETECTION`.
    #[cfg(not(feature = "lang-detection"))]
    fn update_env_lang_detection(&mut self) {
        if let Ok(env_var) = env::var(ENV_VAR_TPNOTE_LANG_DETECTION) {
            if !env_var.is_empty() {
                self.get_lang_filter.mode = Mode::Disabled;
                self.map_lang_filter_btmap = None;
                log::debug!(
                    "Ignoring the env. var. `{}`. The `lang-detection` feature \
                 is not included in this build.",
                    ENV_VAR_TPNOTE_LANG_DETECTION
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Attention: as these test-functions run in parallel, make sure that
    /// each environment variable appears in one function only!

    #[test]
    fn test_update_author_setting() {
        let mut settings = Settings::default();
        unsafe {
            env::set_var(ENV_VAR_LOGNAME, "testauthor");
        }
        settings.update_author();
        assert_eq!(settings.author, "testauthor");
    }

    #[test]
    fn test_update_extension_default_setting() {
        let mut settings = Settings::default();
        unsafe {
            env::set_var(ENV_VAR_TPNOTE_EXTENSION_DEFAULT, "markdown");
        }
        settings.update_extension_default();
        assert_eq!(settings.extension_default, "markdown");

        let mut settings = Settings::default();
        unsafe {
            std::env::remove_var(ENV_VAR_TPNOTE_EXTENSION_DEFAULT);
        }
        settings.update_extension_default();
        assert_eq!(settings.extension_default, "md");
    }

    #[test]
    #[cfg(not(target_family = "windows"))]
    fn test_update_lang_setting() {
        // Test 1
        let mut settings = Settings::default();
        unsafe {
            env::remove_var(ENV_VAR_TPNOTE_LANG);
            env::set_var(ENV_VAR_LANG, "en_GB.UTF-8");
        }
        settings.update_lang(None);
        assert_eq!(settings.lang, "en-GB");

        // Test empty input.
        let mut settings = Settings::default();
        unsafe {
            env::remove_var(ENV_VAR_TPNOTE_LANG);
            env::set_var(ENV_VAR_LANG, "");
        }
        settings.update_lang(None);
        assert_eq!(settings.lang, "");

        // Test precedence of `TPNOTE_LANG`.
        let mut settings = Settings::default();
        unsafe {
            env::set_var(ENV_VAR_TPNOTE_LANG, "it-IT");
            env::set_var(ENV_VAR_LANG, "en_GB.UTF-8");
        }
        settings.update_lang(None);
        assert_eq!(settings.lang, "it-IT");
    }

    #[test]
    #[cfg(feature = "lang-detection")]
    fn test_update_get_lang_filter_setting() {
        // Test 1.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        settings.update_get_lang_filter();

        let output_get_lang_filter = settings
            .get_lang_filter
            .language_candidates
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push(' ');
                l
            })
            .collect::<String>();
        assert_eq!(output_get_lang_filter, "en fr de ");

        //
        // Test 2.
        let mut settings = Settings {
            lang: "it-IT".to_string(),
            ..Default::default()
        };
        settings.update_get_lang_filter();

        let output_get_lang_filter = settings
            .get_lang_filter
            .language_candidates
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push(' ');
                l
            })
            .collect::<String>();
        assert_eq!(output_get_lang_filter, "en fr de it ");
    }

    #[test]
    fn test_update_map_lang_filter_hmap_setting() {
        // Test 1.
        let mut settings = Settings {
            lang: "it-IT".to_string(),
            ..Default::default()
        };
        settings.update_map_lang_filter_btmap();

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();

        assert_eq!(output_map_lang_filter.get("de").unwrap(), "de-DE");
        assert_eq!(output_map_lang_filter.get("et").unwrap(), "et-ET");
        assert_eq!(output_map_lang_filter.get("it").unwrap(), "it-IT");

        //
        // Test short `settings.lang`.
        let mut settings = Settings {
            lang: "it".to_string(),
            ..Default::default()
        };
        settings.update_map_lang_filter_btmap();

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();

        assert_eq!(output_map_lang_filter.get("de").unwrap(), "de-DE");
        assert_eq!(output_map_lang_filter.get("et").unwrap(), "et-ET");
        assert_eq!(output_map_lang_filter.get("it"), None);
    }

    #[test]
    #[cfg(feature = "lang-detection")]
    fn test_update_env_lang_detection() {
        // Test 1.
        // Test short `settings.lang`.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        unsafe { env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "fr-FR, de-DE, hu") };
        settings.update_env_lang_detection();

        let output_get_lang_filter = settings
            .get_lang_filter
            .language_candidates
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push(' ');
                l
            })
            .collect::<String>();
        assert_eq!(output_get_lang_filter, "fr de hu en ");

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();
        assert_eq!(output_map_lang_filter.get("de").unwrap(), "de-DE");
        assert_eq!(output_map_lang_filter.get("fr").unwrap(), "fr-FR");
        assert_eq!(output_map_lang_filter.get("en").unwrap(), "en-GB");

        //
        // Test 2.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        unsafe { env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, de-AT, en-US") };
        settings.update_env_lang_detection();

        let output_get_lang_filter = settings
            .get_lang_filter
            .language_candidates
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push(' ');
                l
            })
            .collect::<String>();
        assert_eq!(output_get_lang_filter, "de de en ");

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();
        assert_eq!(output_map_lang_filter.get("de").unwrap(), "de-DE");
        assert_eq!(output_map_lang_filter.get("en").unwrap(), "en-US");

        //
        // Test 3.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        unsafe { env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, +all, en-US") };
        settings.update_env_lang_detection();

        assert!(settings.get_lang_filter.language_candidates.is_empty());

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();
        assert_eq!(output_map_lang_filter.get("de").unwrap(), "de-DE");
        assert_eq!(output_map_lang_filter.get("en").unwrap(), "en-US");

        //
        // Test 4.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        unsafe { env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, de-AT, en") };
        settings.update_env_lang_detection();

        let output_get_lang_filter = settings
            .get_lang_filter
            .language_candidates
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push(' ');
                l
            })
            .collect::<String>();
        assert_eq!(output_get_lang_filter, "de de en ");

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();
        assert_eq!(output_map_lang_filter.get("de").unwrap(), "de-DE");
        assert_eq!(output_map_lang_filter.get("en").unwrap(), "en-GB");

        //
        // Test 5.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        unsafe { env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, +all, de-AT, en") };
        settings.update_env_lang_detection();

        assert!(settings.get_lang_filter.language_candidates.is_empty());

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();
        assert_eq!(output_map_lang_filter.get("de").unwrap(), "de-DE");
        assert_eq!(output_map_lang_filter.get("en").unwrap(), "en-GB");

        // Test `force_lang`.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        unsafe { env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "fr-FR, de-DE, hu") };
        settings.update_env_lang_detection();

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();
        assert_eq!(output_map_lang_filter.get("de").unwrap(), "de-DE");
        assert_eq!(output_map_lang_filter.get("fr").unwrap(), "fr-FR");
        assert_eq!(output_map_lang_filter.get("en").unwrap(), "en-GB");

        //
        // Test empty env. var.
        let mut settings = Settings {
            lang: "".to_string(),
            ..Default::default()
        };
        unsafe { env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "") };
        settings.update_env_lang_detection();

        assert_eq!(settings.get_lang_filter.mode, Mode::Disabled);
        assert!(settings.map_lang_filter_btmap.is_none());

        //
        // Test faulty `settings.lang`.
        let mut settings = Settings {
            lang: "xy-XY".to_string(),
            ..Default::default()
        };
        unsafe { env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "en-GB, fr") };
        settings.update_env_lang_detection();

        let output_get_lang_filter = settings
            .get_lang_filter
            .language_candidates
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push(' ');
                l
            })
            .collect::<String>();
        assert_eq!(output_get_lang_filter, "en fr ");

        let output_map_lang_filter = settings.map_lang_filter_btmap.unwrap();
        assert_eq!(output_map_lang_filter.get("en").unwrap(), "en-GB");

        //
        // Test faulty entry in list.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        unsafe {
            env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, xy-XY");
        }
        settings.update_env_lang_detection();

        assert!(matches!(settings.get_lang_filter.mode, Mode::Error(..)));
        assert!(settings.map_lang_filter_btmap.is_none());
        //
        // Test empty list.
        let mut settings = Settings {
            lang: "en-GB".to_string(),
            ..Default::default()
        };
        unsafe {
            env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "");
        }
        settings.update_env_lang_detection();

        assert!(matches!(settings.get_lang_filter.mode, Mode::Disabled));
        assert!(settings.map_lang_filter_btmap.is_none());
    }
}
