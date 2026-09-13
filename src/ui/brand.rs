use leptos::prelude::*;

use super::classes::{AUTH_BRAND, AUTH_BRAND_META, AUTH_BRAND_NAME, AUTH_LOGO};

/// Auth / error card brand mark.
#[component]
pub fn AuthBrand() -> impl IntoView {
    view! {
        <div class=AUTH_BRAND>
            <span class=AUTH_LOGO aria-hidden="true">"B"</span>
            <div>
                <p class=AUTH_BRAND_NAME>"Buwiz"</p>
                <p class=AUTH_BRAND_META>"Philippine tax workspace"</p>
            </div>
        </div>
    }
}
