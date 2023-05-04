//! Configuration data that origins from environment variables.
//! Unlike the configuration data in `LIB_CFG` which is source only once at the
//! of Tp-Note, the `SETTINGS` object may be sourced more often to follow
//! changes in the related environment variables.

use crate::config::LIB_CFG;
use crate::error::ConfigError;
use lingua;
#[cfg(feature = "lang-detection")]
use lingua::IsoCode639_1;
use std::collections::HashMap;
use std::{env, mem, str::FromStr, sync::RwLock};
#[cfg(target_family = "windows")]
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
#[cfg(target_family = "windows")]
use windows_sys::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

/// Name of the environment variable, that can be optionally
/// used to overwrite the user's default language setting, which is
/// accessible as `{{ lang }}` template variable and used in various
/// templates.
pub const ENV_VAR_TPNOTE_LANG: &str = "TPNOTE_LANG";

/// Name of the environment variable, that can be optionally
/// used to overwrite the user's `filter_get_lang` and `filter_map_lang`
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
const ENV_VAR_LANG: &str = "LANG";

#[cfg(feature = "lang-detection")]
#[derive(Debug)]
/// Struct containing additional user configuration mostly read from
/// environment variables.
pub(crate) struct Settings {
    pub author: String,
    pub lang: String,
    pub filter_get_lang: Result<Vec<IsoCode639_1>, ConfigError>,
    pub filter_map_lang_hmap: Option<HashMap<String, String>>,
}

#[cfg(not(feature = "lang-detection"))]
#[derive(Debug)]
/// Structure holding various settings from environment varialbes.
/// Some member variables also insert data from `LIB_CFG`.
pub(crate) struct Settings {
    /// Cf. documentation for `update_author_setting()`.
    pub author: String,
    /// Cf. documentation for `update_lang_setting()`.
    pub lang: String,
    /// Cf. documentation for `update_filter_get_lang_setting()`.
    pub filter_get_lang: Result<Vec<String>, ConfigError>,
    /// Cf. documentation for `update_filter_map_lang_hmap_setting()`.
    pub filter_map_lang_hmap: Option<HashMap<String, String>>,
}

/// Default to empty lists and values.
impl Default for Settings {
    fn default() -> Self {
        Settings {
            author: String::new(),
            lang: String::new(),
            filter_get_lang: Ok(vec![]),
            filter_map_lang_hmap: None,
        }
    }
}

/// Global mutable varible of type `Settings`.
/// Todo: use `default()` when `impl const` is available.
pub(crate) static SETTINGS: RwLock<Settings> = RwLock::new(Settings {
    author: String::new(),
    lang: String::new(),
    filter_get_lang: Ok(vec![]),
    filter_map_lang_hmap: None,
});

/// When `lang` is not `-`, overwrite `SETTINGS.lang` with `lang`.
/// In any case, disable the `get_lang` filter by deleting all languages
/// in `SETTINGS.filter_get_lang`.
pub(crate) fn force_lang_setting(lang: &str) {
    let lang = lang.trim();
    let mut settings = SETTINGS.write().unwrap();
    // Overwrite environment setting.
    if lang != "-" {
        let _ = mem::replace(&mut settings.lang, lang.to_string());
    }
    // Disable the `get_lang` Tera filter.
    let _ = mem::replace(&mut settings.filter_get_lang, Ok(vec![]));
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
    let _ = mem::replace(&mut settings.author, author);
}

/// (Re)read environment variables and store them in the global `SETTINGS`
/// object. Some data originates from `LIB_CFG`.
pub fn update_settings() -> Result<(), ConfigError> {
    let mut settings = SETTINGS.write().unwrap();
    update_author_setting(&mut settings);
    update_lang_setting(&mut settings);
    update_filter_get_lang_setting(&mut settings);
    update_filter_map_lang_hmap_setting(&mut settings);
    update_env_lang_detection(&mut settings);

    log::trace!("`SETTINGS` updated:\n{:#?}", settings);

    if let Err(e) = &settings.filter_get_lang {
        Err(e.clone())
    } else {
        Ok(())
    }
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
        lang = String::new();
        let mut buf = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
        let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        if len > 0 {
            lang = String::from_utf16_lossy(&buf[..((len - 1) as usize)]);
        }
    };

    // Store result.
    let _ = mem::replace(&mut settings.lang, lang);
}

/// Read language list from `LIB_CFG.tmpl.filter_get_lang`, add the user's
/// default language subtag and store them in `SETTINGS.filter_get_lang`.
#[cfg(feature = "lang-detection")]
/// Convert the `get_lang_filter()` configuration from the config file.
fn update_filter_get_lang_setting(settings: &mut Settings) {
    let lib_cfg = LIB_CFG.read().unwrap();
    // Read and convert ISO codes from config object.
    match lib_cfg
        .tmpl
        .filter_get_lang
        .iter()
        .map(|l| {
            IsoCode639_1::from_str(l).map_err(|_| {
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
                ConfigError::ParseLanguageCode {
                    language_code: l.into(),
                    all_langs,
                }
            })
        })
        .collect::<Result<Vec<IsoCode639_1>, ConfigError>>()
    {
        Ok(mut iso_codes) => {
            // Add the user's language subtag as reported from the OS.
            if !settings.lang.is_empty() {
                if let Some((lang_subtag, _)) = settings.lang.split_once('-') {
                    if let Ok(iso_code) = IsoCode639_1::from_str(lang_subtag) {
                        if !iso_codes.contains(&iso_code) {
                            iso_codes.push(iso_code);
                        }
                    }
                }
            }
            // Store result.
            let _ = mem::replace(&mut settings.filter_get_lang, Ok(iso_codes));
        }
        Err(e) =>
        // Store error.
        {
            let _ = mem::replace(&mut settings.filter_get_lang, Err(e));
        }
    }
}

#[cfg(not(feature = "lang-detection"))]
/// Reset to empty default.
fn update_filter_get_lang_setting(settings: &mut Settings) {
    let _ = mem::replace(&mut settings.filter_get_lang, Ok(vec![]));
}

/// Read keys and values from `LIB_CFG.tmpl.filter_map_lang` into HashMap.
/// Add the user's default language and region.
fn update_filter_map_lang_hmap_setting(settings: &mut Settings) {
    let mut hm = HashMap::new();
    let lib_cfg = LIB_CFG.read().unwrap();
    for l in &lib_cfg.tmpl.filter_map_lang {
        if l.len() >= 2 {
            hm.insert(l[0].to_string(), l[1].to_string());
        };
    }
    // Insert the user's default language and region in the HashMap.
    if !settings.lang.is_empty() {
        if let Some((lang_subtag, _)) = settings.lang.split_once('-') {
            // Do not overwrite existing languages.
            if !lang_subtag.is_empty() && !hm.contains_key(lang_subtag) {
                hm.insert(lang_subtag.to_string(), settings.lang.to_string());
            }
        };
    }

    // Store result.
    let _ = mem::replace(&mut settings.filter_map_lang_hmap, Some(hm));
}

/// Reads the environment variable `LANG_DETECTION`. If not empty,
/// parse the  content and overwrite `settings.filter_get_lang` and
/// `settings.filter_map_lang`.
fn update_env_lang_detection(settings: &mut Settings) {
    if let Ok(env_var) = env::var(ENV_VAR_TPNOTE_LANG_DETECTION) {
        if env_var == "" {
            let _ = mem::replace(&mut settings.filter_get_lang, Ok(vec![]));
            let _ = mem::replace(&mut settings.filter_map_lang_hmap, None);
            log::info!(
                "Empty env. var. `{}` disables `lang-detection` feature.",
                ENV_VAR_TPNOTE_LANG_DETECTION
            );
            return;
        }

        // Read and convert ISO codes from config object.
        let mut hm: HashMap<String, String> = HashMap::new();
        match env_var
            // The happy path.
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
            // The error path.
            .map(|l| {
                IsoCode639_1::from_str(l.trim()).map_err(|_| {
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
                    ConfigError::ParseLanguageCode {
                        language_code: l.into(),
                        all_langs,
                    }
                })
            })
            .collect::<Result<Vec<IsoCode639_1>, ConfigError>>()
        {
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
                let _ = mem::replace(&mut settings.filter_get_lang, Ok(iso_codes));
                let _ = mem::replace(&mut settings.filter_map_lang_hmap, Some(hm));
            }
            Err(e) =>
            // Store error.
            {
                let _ = mem::replace(&mut settings.filter_get_lang, Err(e));
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
    fn test_update_update_author_setting() {
        let mut settings = Settings::default();
        env::set_var(ENV_VAR_LOGNAME, "testauthor");
        update_author_setting(&mut settings);
        assert_eq!(settings.author, "testauthor");
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
        let _ = mem::replace(&mut settings.lang, "en-GB".to_string());
        update_filter_get_lang_setting(&mut settings);

        let output_filter_get_lang = settings
            .filter_get_lang
            .unwrap()
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push_str(" ");
                l
            })
            .collect::<String>();
        assert_eq!(output_filter_get_lang, "en fr de ");

        //
        // Test 2.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "it-IT".to_string());
        update_filter_get_lang_setting(&mut settings);

        let output_filter_get_lang = settings
            .filter_get_lang
            .unwrap()
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push_str(" ");
                l
            })
            .collect::<String>();
        assert_eq!(output_filter_get_lang, "en fr de it ");
    }

    #[test]
    fn test_update_filter_map_lang_hmap_setting() {
        // Test 1.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "it-IT".to_string());
        update_filter_map_lang_hmap_setting(&mut settings);

        let output_filter_map_lang = settings.filter_map_lang_hmap.unwrap();

        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("et").unwrap(), "et-ET");
        assert_eq!(output_filter_map_lang.get("it").unwrap(), "it-IT");

        //
        // Test short `settings.lang`.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "it".to_string());
        update_filter_map_lang_hmap_setting(&mut settings);

        let output_filter_map_lang = settings.filter_map_lang_hmap.unwrap();

        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("et").unwrap(), "et-ET");
        assert_eq!(output_filter_map_lang.get("it"), None);
    }

    #[test]
    fn test_update_env_lang_detection() {
        // Test 1.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "en-GB".to_string());
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "fr-FR, de-DE, hu");
        update_env_lang_detection(&mut settings);

        let output_filter_get_lang = settings
            .filter_get_lang
            .unwrap()
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push_str(" ");
                l
            })
            .collect::<String>();
        assert_eq!(output_filter_get_lang, "fr de hu en ");
        let output_filter_map_lang = settings.filter_map_lang_hmap.unwrap();
        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("fr").unwrap(), "fr-FR");
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-GB");

        //
        // Test 2.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "en-GB".to_string());
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, de-AT, en-US");
        update_env_lang_detection(&mut settings);

        let output_filter_get_lang = settings
            .filter_get_lang
            .unwrap()
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push_str(" ");
                l
            })
            .collect::<String>();
        assert_eq!(output_filter_get_lang, "de de en ");
        let output_filter_map_lang = settings.filter_map_lang_hmap.unwrap();
        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-US");
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "".to_string());
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "");
        update_env_lang_detection(&mut settings);

        assert!(settings.filter_get_lang.unwrap().is_empty());
        assert!(settings.filter_map_lang_hmap.is_none());

        //
        // Test 3.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "en-GB".to_string());
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, de-AT, en");
        update_env_lang_detection(&mut settings);

        let output_filter_get_lang = settings
            .filter_get_lang
            .unwrap()
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push_str(" ");
                l
            })
            .collect::<String>();
        assert_eq!(output_filter_get_lang, "de de en ");
        let output_filter_map_lang = settings.filter_map_lang_hmap.unwrap();
        assert_eq!(output_filter_map_lang.get("de").unwrap(), "de-DE");
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-GB");
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "".to_string());
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "");
        update_env_lang_detection(&mut settings);

        assert!(settings.filter_get_lang.unwrap().is_empty());
        assert!(settings.filter_map_lang_hmap.is_none());

        //
        // Test faulty `settings.lang`.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "xy-XY".to_string());
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "en-GB");
        update_env_lang_detection(&mut settings);

        let output_filter_get_lang = settings
            .filter_get_lang
            .unwrap()
            .iter()
            .map(|l| {
                let mut l = l.to_string();
                l.push_str(" ");
                l
            })
            .collect::<String>();
        assert_eq!(output_filter_get_lang, "en ");
        let output_filter_map_lang = settings.filter_map_lang_hmap.unwrap();
        assert_eq!(output_filter_map_lang.get("en").unwrap(), "en-GB");

        //
        // Test faulty entry in list.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "en-GB".to_string());
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "de-DE, xy-XY");
        update_env_lang_detection(&mut settings);

        assert!(settings.filter_get_lang.is_err());
        assert!(settings.filter_map_lang_hmap.is_none());
        //
        // Test empty list.
        let mut settings = Settings::default();
        let _ = mem::replace(&mut settings.lang, "en-GB".to_string());
        env::set_var(ENV_VAR_TPNOTE_LANG_DETECTION, "");
        update_env_lang_detection(&mut settings);

        assert_eq!(settings.filter_get_lang.unwrap(), vec![]);
        assert!(settings.filter_map_lang_hmap.is_none());
    }
}
