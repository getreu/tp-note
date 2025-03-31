//! This module abstracts the Lingua library API.
use crate::settings::SETTINGS;
use crate::{config::Mode, error::LibCfgError};
pub(crate) use lingua::IsoCode639_1;
use lingua::{LanguageDetector, LanguageDetectorBuilder};
use std::collections::HashMap; // Reexport this type.

/// A filter telling in which natural language(s) the input text is written.
/// It returns an array of ISO 639-1 code representations listing the detected
/// languages. If no language can be reliably identified, the output is the
/// empty array.
#[cfg(feature = "lang-detection")]
pub(crate) fn get_lang(input: &str) -> Result<Vec<String>, LibCfgError> {
    use itertools::Itertools;

    let input = input.trim();
    // Return early if there is no input text.
    if input.is_empty() {
        return Ok(vec![]);
    }

    let settings = SETTINGS.read_recursive();

    // Check if we can return early.
    match &settings.get_lang_filter.mode {
        Mode::Disabled => return Ok(vec![]),

        Mode::Error(e) => return Err(e.clone()),
        _ => {}
    }

    // Build `LanguageDetector`.
    let detector: LanguageDetector = if !&settings.get_lang_filter.language_candidates.is_empty() {
        log::trace!(
            "Execute template filter `get_lang` \
                        with languages candidates: {:?}",
            &settings.get_lang_filter.language_candidates,
        );

        LanguageDetectorBuilder::from_iso_codes_639_1(&settings.get_lang_filter.language_candidates)
            .with_minimum_relative_distance(settings.get_lang_filter.relative_distance_min)
            .build()
    } else {
        log::trace!(
            "Execute template filter `get_lang` \
                        with all available languages",
        );
        LanguageDetectorBuilder::from_all_languages()
            .with_minimum_relative_distance(settings.get_lang_filter.relative_distance_min)
            .build()
    };

    // Detect languages.
    let detected_languages: Vec<String> = match &settings.get_lang_filter.mode {
        Mode::Multilingual => {
            // TODO
            let consecutive_words_min = settings.get_lang_filter.consecutive_words_min;
            let words_total_percentage_min = settings.get_lang_filter.words_total_percentage_min;

            let words_total = input.split_whitespace().count();
            let words_min = [consecutive_words_min, words_total / 3]; // TODO
            let words_min = words_min.iter().min().unwrap();
            log::trace!(
                "Language snippets with less than {} words will be ignored.",
                words_min
            );

            let words_distribution: HashMap<String, usize> = detector
                .detect_multiple_languages_of(input)
                .into_iter()
                // Filter too short word sequences.
                .filter(|l| {
                    let allow_through = l.word_count() >= *words_min;
                    log::trace!(
                        "Language(s) detected: {}, {}, {}: {:?}",
                        l.language().iso_code_639_1().to_string(),
                        l.word_count(),
                        allow_through,
                        input[l.start_index()..l.end_index()]
                            .chars()
                            .take(50)
                            .collect::<String>()
                    );
                    allow_through
                })
                .map(|l| (l.language().iso_code_639_1().to_string(), l.word_count()))
                .into_grouping_map_by(|n| n.0.clone())
                .aggregate(|acc, _key, val| Some(acc.unwrap_or(0) + val.1));

            // Descending order sort.
            let words_distribution: Vec<(String, usize)> = words_distribution
                .into_iter()
                .sorted_by_key(|l| usize::MAX - l.1)
                .collect();
            log::debug!(
                "Languages distribution per word count:\n {:?}",
                words_distribution
            );

            // Filter languages, whose words do not occur sufficiently in total.
            let words_distribution_total: usize = words_distribution.iter().map(|l| l.1).sum();
            let words_total_min: usize =
                words_distribution_total * words_total_percentage_min / 100;

            // Filter languages with too few words and return language list.
            words_distribution
                .into_iter()
                .filter(|(l, wc)| {
                    if *wc >= words_total_min {
                        true
                    } else {
                        let words_percentage = wc * 100 / words_distribution_total;
                        log::info!(
                            "Language `{}` rejected: not enough words in total ({}%<{}%)",
                            l,
                            words_percentage,
                            words_total_percentage_min
                        );
                        false
                    }
                })
                .map(|(l, _)| l)
                .collect::<Vec<String>>()
        }

        Mode::Monolingual => detector
            .detect_language_of(input)
            .into_iter()
            .map(|l| l.iso_code_639_1().to_string())
            .inspect(|l| log::debug!("Language: '{}' in input detected.", l))
            .collect(),

        Mode::Disabled => unreachable!(), // See early return above.

        Mode::Error(_) => unreachable!(), // See early return above.
    };

    Ok(detected_languages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::RwLockWriteGuard;

    #[test]
    fn test_get_lang() {
        use crate::{
            config::{GetLang, Mode},
            settings::Settings,
        };
        use lingua::IsoCode639_1;

        // The `get_lang` filter requires an initialized `SETTINGS` object.
        // Lock the config object for this test.
        let get_lang_filter = GetLang {
            mode: Mode::Multilingual,
            language_candidates: vec![IsoCode639_1::DE, IsoCode639_1::EN, IsoCode639_1::FR],
            relative_distance_min: 0.2,
            consecutive_words_min: 5,
            words_total_percentage_min: 10,
        };

        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        settings.get_lang_filter = get_lang_filter;
        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);

        let input = "Das große Haus";
        let output = get_lang(input).unwrap();
        assert_eq!("de", output[0]);

        let input = "Il est venu trop tard";
        let output = get_lang(input).unwrap();
        assert_eq!("fr", output[0]);

        let input = "How to set up a roof rack";
        let output = get_lang(input).unwrap();
        assert_eq!("en", output[0]);

        let input = "1917039480 50198%-328470";
        let output = get_lang(input).unwrap();
        assert!(output.is_empty());

        let input = " \t\n ";
        let output = get_lang(input).unwrap();
        assert!(output.is_empty());

        let input = "Parlez-vous français? \
        Ich spreche Französisch nur ein bisschen. \
        A little bit is better than nothing. \
        Noch mehr Deutsch. \
        Bien-sûr, je parle un peu. Qu'est-ce que tu veux?";
        let output = get_lang(input).unwrap();

        // Execute template filter `get_lang` with languages candidates: [EN, FR, DE, ET]
        // Language(s) detected: fr, 2, false: "Parlez-vous français?"
        // Language(s) detected: de, 7, true: "Ich spreche Französisch nur ein bisschen."
        // Language(s) detected: en, 6, true: "little bit is better than nothing."
        // Language(s) detected: de, 3, false: "Noch mehr Deutsch."
        // Language(s) detected: fr, 9, true: "Bien-sûr, je parle un peu. Qu'est-ce que tu veux?"
        // Languages distribution per word count: [("fr", 9), ("de", 7), ("en", 6)]
        assert_eq!(output, ["fr", "de", "en"]);

        let input = "Parlez-vous français? \
        Ich spreche Französisch nur ein bisschen. \
        A little bit is better than nothing.";
        let output = get_lang(input).unwrap();

        // Scheme index: 0, applying the content template: `tmpl.from_clipboard_content`
        // Execute template filter `get_lang` with languages candidates: [EN, FR, DE, ET]
        // Language(s) detected: fr, 2, false: "Parlez-vous français?"
        // Language(s) detected: de, 7, true: "Ich spreche Französisch nur ein bisschen."
        // Language(s) detected: en, 6, true: "little bit is better than nothing."
        // Languages distribution per word count: [("de", 7), ("en", 6)]
        assert_eq!(output, ["de", "en"]);

        // Release the lock.
        drop(_settings);
    }

    #[test]
    fn test_get_lang2() {
        use crate::{
            config::{GetLang, Mode},
            settings::Settings,
        };
        use lingua::IsoCode639_1;

        // The `get_lang` filter requires an initialized `SETTINGS` object.
        // Lock the config object for this test.
        let get_lang_filter = GetLang {
            mode: Mode::Monolingual,
            language_candidates: vec![IsoCode639_1::DE, IsoCode639_1::EN, IsoCode639_1::FR],
            relative_distance_min: 0.2,
            consecutive_words_min: 5,
            words_total_percentage_min: 10,
        };

        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        settings.get_lang_filter = get_lang_filter;
        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);

        let input = "Das große Haus";
        let output = get_lang(input).unwrap();
        assert_eq!("de", output[0]);

        let input = "Il est venu trop tard";
        let output = get_lang(input).unwrap();
        assert_eq!("fr", output[0]);

        let input = "How to set up a roof rack";
        let output = get_lang(input).unwrap();
        assert_eq!("en", output[0]);

        let input = "1917039480 50198%-328470";
        let output = get_lang(input).unwrap();
        assert!(output.is_empty());

        let input = " \t\n ";
        let output = get_lang(input).unwrap();
        assert!(output.is_empty());

        let input = "Parlez-vous français? \
        Ich spreche Französisch nur ein bisschen. \
        A little bit is better than nothing.";
        let output = get_lang(input).unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!("de", output[0]);

        // Release the lock.
        drop(_settings);
    }
}
