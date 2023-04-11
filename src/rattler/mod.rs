// code under in this file and under the 'rattler' folder is copied from:
// https://github.com/mamba-org/rattler/tree/main/crates/rattler-bin
//
// License: BSD-3-Clause
// check the source code for full license text and copyright information

// use crate::rattler::writer::IndicatifWriter;
use indicatif::{MultiProgress, ProgressDrawTarget};
use once_cell::sync::Lazy;

pub(crate) mod writer;
pub(crate) mod commands;

/// Returns a global instance of [`indicatif::MultiProgress`].
///
/// Although you can always create an instance yourself any logging will interrupt pending
/// progressbars. To fix this issue, logging has been configured in such a way to it will not
/// interfere if you use the [`indicatif::MultiProgress`] returning by this function.
pub fn global_multi_progress() -> MultiProgress {
    static GLOBAL_MP: Lazy<MultiProgress> = Lazy::new(|| {
        let mp = MultiProgress::new();
        mp.set_draw_target(ProgressDrawTarget::stderr_with_hz(20));
        mp
    });
    GLOBAL_MP.clone()
}

