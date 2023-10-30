//! Configuration data that origins from environment variables.
//! Unlike the configuration data in `LIB_CFG` which is sourced only once when
//! Tpublaunched, the `SETTINGS` object may be sourced more often in
//! order to follow changes in the related environment variables.

use crate::config::LIB_CFG;
#[cfg(feature = "lang-detection")]
use crate::config::TMPL_FILTER_GET_LANG_ALL;
use crate::error::LibCfgError;
#[cfg(feature = "lang-detection")]
use lingua;
#[cfg(feature = "lang-detection")]
use lingua::IsoCode639_1;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::env;
#[cfg(feature = "lang-detection")]
use std::str::FromStr;
#[cfg(target_family = "windows")]
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
#[cfg(target_family = "windows")]
use windows_sys::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

/// The name of the environment variable which can be optionally set to
/// overwrite the `flename.extension_default` configuration file setting.
pub const ENV_VAR_TPNOTE_EXTENSION_DEFAULT: &str = "TPNOTE_EXTENSION_DEFAULT";
/// Name of the environment variable, that can be optionally
/// used to overwrite the user's default language setting, which is
/// accessible as `{{ lang }}` template variable and used in various
/// templates.
pub const ENV_VAR_TPNOTE_LANG: &str = "TPNOTE_LANG";

/// Name of the environment variable, that can be optionally
/// used to overwrite the user's `tmpl.filter.get_lang` and `tmpl.filter.map_lang`
/// configuration file setting.
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

#[derive(Debug)]
#[allow(dead_code)]
/// Indicates how the `get_lang` filter operates.
pub(crate) enum FilterGetLang {
    /// The filter is disabled and returns the empty string.
    Disabled,
    /// All available (about 76) languages are selected as search candidates.
    /// This causes the filter execution to take some time (up to 5 seconds).
    AllLanguages,
    /// A list of language tags the algorithm considers as potential candidates
    /// to determinate the natural language.
    #[cfg(feature = "lang-detection")]
    SomeLanguages(Vec<IsoCode639_1>),
    /// A list of language tags the algorithm considers as potential candidates
    /// to determinate the natural language.
    #[cfg(not(feature = "lang-detection"))]
    SomeLanguages(Vec<String>),
    /// The filter configuration could not be read and converted properly.
    Error(LibCfgError),
}

/// Struct containing additional user configuration read from or depending
/// on environment variables.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct Settings {
    pub author: String,
    pub lang: String,
    pub extension_default: String,
    pub filter_get_lang: FilterGetLang,
    pub filter_map_lang_btmap: Option<BTreeMap<String, String>>,
}

const DEFAULT_SETTINGS: Settings = Settings {
    author: String::new(),
    lang: String::new(),
    extension_default: String::new(),
    filter_get_lang: FilterGetLang::Disabled,
    filter_map_lang_btmap: None,
};

impl Default for Settings {
    #[cfg(not(test))]
    /// Defaults to empty lists and values.
    fn default() -> Self {
        DEFAULT_SETTINGS
    }

    #[cfg(test)]
    /// Defaults to test values.
    fn default() -> Self {
        let mut settings = DEFAULT_SETTINGS;
        settings.author = String::from("testuser");
        settings.lang = String::from("ab_AB");
        settings.extension_default = String::from("md");
        settings
    }
}

/// Global mutable varible of type `Settings`.
#[cfg(not(test))]
pub(crate) static SETTINGS: RwLock<Settings> = RwLock::new(DEFAULT_SETTINGS);

#[cfg(test)]
lazy_static::lazy_static! {
/// Global default for `SETTINGS` in test environments.
pub(crate) static ref SETTINGS: RwLock<Settings> = RwLock::new(DEFAULT_SETTINGS);
}

/// When `lang` is `Some(l)`, overwrite `SETTINGS.lang` with `l`.
/// In any case, disable the `get_lang` filter by setting `filter_get_lang`
/// to `FilterGetLang::Disabled`.
pub(crate) fn force_lang_setting(lang: Option<String>) {
    let mut settings = SETTINGS.write();
    // Overwrite environment setting.
    if let Some(l) = lang {
        settings.lang = l;
    }
    // Disable the `get_lang` Tera filter.
    settings.filter_get_lang = FilterGetLang::Disabled;

    log::trace!(
        "`SETTINGS` updated after `force_lang_setting()`:\n{:#?}",
        settings
    );
}

/// (Re)read environment variables and store them in the global `SETTINGS`
/// object. Some data originates from `LIB_CFG`.
pub fn update_settings() -> Result<(), LibCfgError> {
    let mut settings = SETTINGS.write();
    update_author_setting(&mut settings);
    update_extension_default_setting(&mut settings);
    update_lang_setting(&mut settings);
    update_filter_get_lang_setting(&mut settings);
    update_filter_map_lang_btmap_setting(&mut settings);
    update_env_lang_detection(&mut settings);

    log::trace!(
        "`SETTINGS` updated (reading config + env. vars.):\n{:#?}",
        settings
    );

    if let FilterGetLang::Error(e) = &settings.filter_get_lang {
        Err(e.clone())
    } else {
        Ok(())
    }
}

/// Set `SETTINGS.author` to content of the first not empty environment
/// variable: `TPNOTE_USER`, `LOGNAME` or `USER`.
fn update_author_setting(settings: &mut Settings) {
    let author = env::var(ENV_VAR_TPNOTE_USER).unwrap_or_else(|_| {
        env::var(ENV_VAR_LOGNAME).unwrap_or_else(|_| {
            env::var(ENV_VAR_USERNAME)
                .unwrap_or_else(|_| env::var(ENV_VAR_USER).unwrap_or_default())
        })
    });

    // Store result.
    settings.author = author;
}

/// Read the environment variable `TPNOTE_EXTENSION_DEFAULT` or -if empty-
/// the configuration file variable `filename.extension_default` into
/// `SETTINGS.extension_default`.
fn update_extension_default_setting(settings: &mut Settings) {
    // Get the environment variable if it exists.
    let ext = match env::var(ENV_VAR_TPNOTE_EXTENSION_DEFAULT) {
        Ok(ed_env) if !ed_env.is_empty() => ed_env,
        Err(_) | Ok(_) => {
            let lib_cfg = LIB_CFG.read_recursive();
            lib_cfg.filename.extension_default.to_string()
        }
    };
    settings.extension_default = ext;
}

/// Read the environment variable `TPNOTE_LANG` or -if empty- `LANG` into
/// `SETTINGS.lang`.
fn update_lang_setting(settings: &mut Settings) {
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
    settings.lang = lang;
}

/// Read language list from `LIB_CFG.tmpl.filter.get_lang`, add the user's
/// default language subtag and store them in `SETTINGS.filter_get_lang`
/// as `FilterGetLang::SomeLanguages(l)` `enum` variant.
/// If `SETTINGS.filter_get_lang` contains a tag `TMPL_FILTER_GET_LANG_ALL`,
/// all languages are selected by setting `FilterGetLang::AllLanguages`.
/// Errors are stored in the `FilterGetLang::Error(e)` variant.
#[cfg(feature = "lang-detection")]
fn update_filter_get_lang_setting(settings: &mut Settings) {
    let lib_cfg = LIB_CFG.read_recursive();

    let mut all_languages_selected = false;
    // Read and convert ISO codes from config object.
    match lib_cfg
        .tmpl
        .filter.get_lang
        .iter()
        // Skip if this is the pseudo tag for all languages.
        .filter(|&l| {
            if l == TMPL_FILTER_GET_LANG_ALL {
                all_languages_selected = true;
                // Skip this string.
                false
            } else {
                // Continue.
                true
            }
        })
        .map(|l| {
            IsoCode639_1::from_str(l).map_err(|_| {
                // The error path.
                // Produce list of all available langugages.
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
        .collect::<Result<Vec<IsoCode639_1>, LibCfgError>>()
    {
        // The happy path.
        Ok(mut iso_codes) => {
            if all_languages_selected {
                // Store result.
                settings.filter_get_lang = FilterGetLang::AllLanguages;
            } else {
                // Add the user's language subtag as reported from the OS.
                // Silently ignore if anything goes wrong here.
                if !settings.lang.is_empty() {
                    if let Some((lang_subtag, _)) = settings.lang.split_once('-') {
                        if let Ok(iso_code) = IsoCode639_1::from_str(lang_subtag) {
                            if !iso_codes.contains(&iso_code) {
                                iso_codes.push(iso_code);
                            }
                        }
                    }
                }

                // Check if there are at least 2 languages in the list.
                settings.filter_get_lang = match iso_codes.len() {
                    0 => FilterGetLang::Disabled,
                    1 => FilterGetLang::Error(LibCfgError::NotEnoughLanguageCodes {
                        language_code: iso_codes[0].to_string(),
                    }),
                    _ => FilterGetLang::SomeLanguages(iso_codes),
                }
            }
        }
        // The error path.
        Err(e) =>
        // Store error.
        {
            settings.filter_get_lang = FilterGetLang::Error(e);
        }
    }
}

#[cfg(not(feature = "lang-detection"))]
/// Disable filter.
fn update_filter_get_lang_setting(settings: &mut Settings) {
    settings.filter_get_lang = FilterGetLang::Disabled;
}

/// Read keys and values from `LIB_CFG.tmpl.filter_btmap_lang` in the `BTreeMap`.
/// Add the user's default language and region.
fn update_filter_map_lang_btmap_setting(settings: &mut Settings) {
    let mut btm = BTreeMap::new();
    let lib_cfg = LIB_CFG.read_recursive();
    for l in &lib_cfg.tmpl.filter.map_lang {
        if l.len() >= 2 {
            btm.insert(l[0].to_string(), l[1].to_string());
        };
    }
    // Insert the user's default language and region in the Map.
    if !settings.lang.is_empty() {
        if let Some((lang_subtag, _)) = settings.lang.split_once('-') {
            // Do not overwrite existing languages.
            if !lang_subtag.is_empty() && !btm.contains_key(lang_subtag) {
                btm.insert(lang_subtag.to_string(), settings.lang.to_string());
            }
        };
    }

    // Store result.
    settings.filter_map_lang_btmap = Some(btm);
}

/// Reads the environment variable `LANG_DETECTION`. If not empty,
/// parse the content and overwrite the `settings.filter_get_lang` and
/// the `settings.filter_map_lang` variables.
#[cfg(feature = "lang-detection")]
fn update_env_lang_detection(settings: &mut Settings) {
    if let Ok(env_var) = env::var(ENV_VAR_TPNOTE_LANG_DETECTION) {
        if env_var.is_empty() {
            // Early return.
            settings.filter_get_lang = FilterGetLang::Disabled;
            settings.filter_map_lang_btmap = None;
            log::debug!(
                "Empty env. var. `{}` disables the `lang-detection` feature.",
                ENV_VAR_TPNOTE_LANG_DETECTION
            );
            return;
        }

        // Read and convert ISO codes from config object.
        let mut hm: BTreeMap<String, String> = BTreeMap::new();
        let mut all_languages_selected = false;
        match env_var
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
            // Check if this is the pseudo tag `TMPL_FILTER_GET_LANG_ALL `.
            .filter(|&l| {
                if l == TMPL_FILTER_GET_LANG_ALL {
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
                    // Produce list of all available langugages.
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
            .collect::<Result<Vec<IsoCode639_1>, LibCfgError>>()
        {
            // The happy path.
            Ok(mut iso_codes) => {
                // Add the user's language subtag as reported from the OS.
                // Continue the happy path.
                if !settings.lang.is_empty() {
                    if let Some(lang_subtag) = settings.lang.split('-').next() {
                        if let Ok(iso_code) = IsoCode639_1::from_str(lang_subtag) {
                            if !iso_codes.contains(&iso_code) {
                                iso_codes.push(iso_code);
                            }
                            // Check if there is a remainder (region code).
                            if lang_subtag != settings.lang && !hm.contains_key(lang_subtag) {
                                hm.insert(lang_subtag.to_string(), settings.lang.to_string());
                            }
                        }
                    }
                }
                // Store result.
                if all_languages_selected {
                    settings.filter_get_lang = FilterGetLang::AllLanguages;
                } else {
                    settings.filter_get_lang = match iso_codes.len() {
                        0 => FilterGetLang::Disabled,
                        1 => FilterGetLang::Error(LibCfgError::NotEnoughLanguageCodes {
                            language_code: iso_codes[0].to_string(),
                        }),
                        _ => FilterGetLang::SomeLanguages(iso_codes),
                    }
                }
                settings.filter_map_lang_btmap = Some(hm);
            }
            // The error path.
            Err(e) =>
            // Store error.
            {
                settings.filter_get_lang = FilterGetLang::Error(e);
            }
        }
    }
}

/// Ignore the environment variable `LANG_DETECTION`.
#[cfg(not(feature = "lang-detection"))]
fn update_env_lang_detection(settings: &mut Settings) {
    if let Ok(env_var) = env::var(ENV_VAR_TPNOTE_LANG_DETECTION) {
        if !env_var.is_empty() {
            settings.filter_get_lang = FilterGetLang::Disabled;
            settings.filter_map_lang_btmap = None;
            log::debug!(
                "Ignoring the env. var. `{}`. The `lang-detection` feature \
                 is not included in this build.",
                ENV_VAR_TPNOTE_LANG_DETECTION
            );
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
        env::set_var(ENV_VAR_LOGNAME, "testauthor");
        update_author_setting(&mut settings);
        assert_eq!(settings.author, "testauthor");
    }

    #[test]
    fn test_update_extension_default_setting() {
        let mut settings = Settings::default();
        env::set_var(ENV_VAR_TPNOTE_EXTENSION_DEFAULT, "markdown");
        update_extension_default_setting(&mut settings);
        assert_eq!(settings.extension_default, "markdown");

        let mut settings = Settings::default();
        std::env::remove_var(ENV_VAR_TPNOTE_EXTENSION_DEFAULT);
        update_extension_default_setting(&mut settings);
        assert_eq!(settings.extension_default, "md");
    }

    #[test]
    #[cfg(not(target_family = "windows"))]
    fn test_update_lang_setting() {
        // Test 1
        let mut settings = Settings::default();
        env::remove_var(ENV_VAR_TPNOTE_LANG);
        env::set_var(ENV_VAR_LANG, "en_GB.UTF-8");
        update_lang_setting(&mut settings);
        assert_eq!(settings.lang, "en-GB");

        // Test empty input.
        let mut settings = Settings::default();
        env::remove_var(ENV_VAR_TPNOTE_LANG);
        env::set_var(ENV_VAR_LANG, "");
        update_lang_setting(&mut settings);
        assert_eq!(settings.lang, "");

        // Test precedence of `TPNOTE_LANG`.
        let mut settings = Settings::default();
        env::set_var(ENV_VAR_TPNOTE_LANG, "it-IT");
        env::set_var(ENV_VAR_LANG, "en_GB.UTF-8");
        update_lang_setting(&mut settings);
        assert_eq!(settings.lang, "it-IT");
    }

    #[test]
    fn test_update_filter_get_lang_setting() {
        // Test 1.
        let mut settings = Settings::default();
        settings.lang = "en-GB".to_string();
        update_filter_get_lang_setting(&mut settings);

        if let FilterGetLang::SomeLanguages(ofgl) = settings.filter_get_lang {
            let output_filter_get_lang = ofgl
                .iter()
                .map(|l| {
                    let mut l = l.to_string();
                    l.push_str(" ");
                    l
                })
                .collect::<String>();
            assert_eq!(output_filter_get_lang, "en fr de ");
        } else {
            panic!("Wrong variant: {:?}", settings.filter_get_lang);
        }

        //
        // Test 2.
        let mut settings = Settings::default();
        settings.lang = "it-IT".to_string();
        update_filter_get_lang_setting(&mut settings);

        if let FilterGetLang::SomeLanguages(ofgl) = settings.filter_get_lang {
            let output_filter_get_lang = ofgl
                .iter()
                .map(|l| {
                    let mut l = l.to_string();
                    l.push_str(" ");
                    l
                })
                .collect::<String>();
            assert_eq!(output_filter_get_lang, "en fr de it ");
        } else {
            panic!("Wrong variant: {:?}", settings.filter_get_lang);
        }
    }

    #[test]
    fn test_update_filter_map_lang_hmap_setting() {
        // Test 1.
        let mut settings = Settings::default();
        settings.lang = "it-IT".to_string();
        update_filter_map_lang_btmap_setting(&mut settings);

        let output_filter_map_lang = settings.filter_map_lang_btmap.unwrap();

        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("et").unwrap(), "et-ET");
        assert_eq!(output_filter_map_lang.get("it").unwrap(), "it-IT");

        //
        // Test short `settings.lang`.
        let mut settings = Settings::default();
        settings.lang = "it".to_string();
        update_filter_map_lang_btmap_setting(&mut settings);

        let output_filter_map_lang = settings.filter_map_lang_btmap.unwrap();

        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("et").unwrap(), "et-ET");
        assert_eq!(output_filter_map_lang.get("it"), None);
    }

    #[test]
    fn test_update_env_lang_detection() {
        // Test 1.
        let mut settings = Settings::default();
        settings.lang = "en-GB".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "fr-FR, de-DE, hu");
        update_env_lang_detection(&mut settings);

        if let FilterGetLang::SomeLanguages(ofgl) = settings.filter_get_lang {
            let output_filter_get_lang = ofgl
                .iter()
                .map(|l| {
                    let mut l = l.to_string();
                    l.push_str(" ");
                    l
                })
                .collect::<String>();
            assert_eq!(output_filter_get_lang, "fr de hu en ");
        } else {
            panic!("Wrong variant: {:?}", settings.filter_get_lang);
        }

        let output_filter_map_lang = settings.filter_map_lang_btmap.unwrap();
        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("fr").unwrap(), "fr-FR");
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-GB");

        //
        // Test 2.
        let mut settings = Settings::default();
        settings.lang = "en-GB".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, de-AT, en-US");
        update_env_lang_detection(&mut settings);

        if let FilterGetLang::SomeLanguages(ofgl) = settings.filter_get_lang {
            let output_filter_get_lang = ofgl
                .iter()
                .map(|l| {
                    let mut l = l.to_string();
                    l.push_str(" ");
                    l
                })
                .collect::<String>();
            assert_eq!(output_filter_get_lang, "de de en ");
        } else {
            panic!("Wrong variant: {:?}", settings.filter_get_lang);
        }
        let output_filter_map_lang = settings.filter_map_lang_btmap.unwrap();
        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-US");

        //
        // Test 3.
        let mut settings = Settings::default();
        settings.lang = "en-GB".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, +all, en-US");
        update_env_lang_detection(&mut settings);

        assert!(matches!(
            settings.filter_get_lang,
            FilterGetLang::AllLanguages
        ));
        let output_filter_map_lang = settings.filter_map_lang_btmap.unwrap();
        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-US");

        //
        // Test 4.
        let mut settings = Settings::default();
        settings.lang = "en-GB".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, de-AT, en");
        update_env_lang_detection(&mut settings);

        if let FilterGetLang::SomeLanguages(ofgl) = settings.filter_get_lang {
            let output_filter_get_lang = ofgl
                .iter()
                .map(|l| {
                    let mut l = l.to_string();
                    l.push_str(" ");
                    l
                })
                .collect::<String>();
            assert_eq!(output_filter_get_lang, "de de en ");
        } else {
            panic!("Wrong variant: {:?}", settings.filter_get_lang);
        }
        let output_filter_map_lang = settings.filter_map_lang_btmap.unwrap();
        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-GB");

        //
        // Test 5.
        let mut settings = Settings::default();
        settings.lang = "en-GB".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, +all, de-AT, en");
        update_env_lang_detection(&mut settings);

        assert!(matches!(
            settings.filter_get_lang,
            FilterGetLang::AllLanguages
        ));
        let output_filter_map_lang = settings.filter_map_lang_btmap.unwrap();
        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-GB");

        //
        // Test empty env. var.
        let mut settings = Settings::default();
        settings.lang = "".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "");
        update_env_lang_detection(&mut settings);

        assert!(matches!(settings.filter_get_lang, FilterGetLang::Disabled));
        assert!(settings.filter_map_lang_btmap.is_none());

        //
        // Test faulty `settings.lang`.
        let mut settings = Settings::default();
        settings.lang = "xy-XY".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "en-GB, fr");
        update_env_lang_detection(&mut settings);

        if let FilterGetLang::SomeLanguages(ofgl) = settings.filter_get_lang {
            let output_filter_get_lang = ofgl
                .iter()
                .map(|l| {
                    let mut l = l.to_string();
                    l.push_str(" ");
                    l
                })
                .collect::<String>();
            assert_eq!(output_filter_get_lang, "en fr ");
        } else {
            panic!("Wrong variant: {:?}", settings.filter_get_lang);
        }
        let output_filter_map_lang = settings.filter_map_lang_btmap.unwrap();
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-GB");

        //
        // Test faulty entry in list.
        let mut settings = Settings::default();
        settings.lang = "en-GB".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, xy-XY");
        update_env_lang_detection(&mut settings);

        assert!(matches!(settings.filter_get_lang, FilterGetLang::Error(..)));
        assert!(settings.filter_map_lang_btmap.is_none());
        //
        // Test empty list.
        let mut settings = Settings::default();
        settings.lang = "en-GB".to_string();
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "");
        update_env_lang_detection(&mut settings);

        assert!(matches!(settings.filter_get_lang, FilterGetLang::Disabled));
        assert!(settings.filter_map_lang_btmap.is_none());
    }
}
