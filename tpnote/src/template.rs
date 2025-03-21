//! Helper functions dealing with `TemplateKind` variants.

use crate::config::CFG;
use crate::settings::ARGS;
use tpnote_lib::template::TemplateKind;

/// Helper function to inhibit template application according to
/// command line parameters.
pub(crate) fn template_kind_filter(template_kind: TemplateKind) -> TemplateKind {
    // Treat inhibitors:
    match template_kind {
        TemplateKind::FromTextFile => {
            if (ARGS.add_header || CFG.arg_default.add_header)
                && !CFG.arg_default.no_filename_sync
                && !ARGS.no_filename_sync
            {
                // No change, we do it.
                template_kind
            } else {
                log::info!(
                    "Not adding header to text file: \
                     `add_header` is not enabled or `no_filename_sync`",
                );
                log::debug!("Changing the template to \"TemplateKind::None\"");
                // We change to `None`.
                TemplateKind::None
            }
        }
        TemplateKind::SyncFilename => {
            if ARGS.no_filename_sync {
                log::info!("Filename synchronisation disabled with the flag: `--no-filename-sync`",);
                log::debug!("Changing the template to \"TemplateKind::None\"");
                TemplateKind::None
            } else if CFG.arg_default.no_filename_sync {
                log::info!(
                    "Filename synchronisation disabled with the configuration file \
             variable: `arg_default.no_filename_sync = true`",
                );
                log::debug!("Changing the template to \"TemplateKind::None\"");
                TemplateKind::None
            } else {
                // We do it, no change
                template_kind
            }
        }
        // Otherwise, there are no more inhibitors so far.
        _ => template_kind,
    }
}
