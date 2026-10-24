//! The self-playing OpenGrok demo on the home page. Browser-only: it measures layout and runs
//! on animation frames, so it is compiled for the hydrate build alone.

mod film;
mod player;
mod window;

pub use film::OpenGrokFilm;
