//! The self-playing AI Co-worker demos on the home page, one per stage of a filing. Browser-only:
//! they measure layout and run on animation frames, so they are compiled for the hydrate build
//! alone.

mod confirm;
mod connect;
mod kit;
mod pay;
mod player;
mod prepare;
mod profiles;
mod shell;
mod submit;

use leptos::prelude::*;

pub use confirm::ConfirmFilm;
pub use connect::ConnectFilm;
pub use pay::PayFilm;
pub use prepare::PrepareFilm;
pub use profiles::ProfilesFilm;
pub use submit::SubmitFilm;

use kit::Rig;
use player::{ReplayButton, SpeedChips, StepChips};

/// A film's steps, speed and replay controls and live caption, over the window.
#[component]
fn FilmBar(rig: Rig, steps: &'static [&'static str]) -> impl IntoView {
    view! {
            <div class="mx-auto mb-4 flex w-full max-w-[1080px] flex-wrap items-center gap-2">
                <StepChips steps current=rig.act />
                <div class="ml-auto flex items-center gap-2">
                    <SpeedChips timeline=rig.tl />
                    <ReplayButton timeline=rig.tl />
                </div>
                <p class="min-h-5 basis-full pt-1 text-left text-[13px] text-gc-text-muted" aria-live="polite">{move || rig.caption.get()}</p>
            </div>
    }
}
