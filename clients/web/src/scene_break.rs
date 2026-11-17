use leptos::prelude::*;

/// Five diamonds tip to tip, the middle one largest, drawn where one scene ends and the next begins.
/// The middle one holds a plus, which shows only where the break is also the button to add something there.
pub fn diamonds() -> impl IntoView {
    view! {
        <svg
            class="diamonds between-scenes"
            width="48.2"
            height="12.6"
            viewBox="0 0 48.2 12.6"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            <polygon class="diamond leftmost" points="0.5,6.3 4.5,2.3 8.5,6.3 4.5,10.3"></polygon>
            <polygon class="diamond left" points="8.5,6.3 13.4,1.4 18.3,6.3 13.4,11.2"></polygon>
            <g class="diamond middle">
                <polygon points="18.3,6.3 24.1,0.5 29.9,6.3 24.1,12.1"></polygon>
                <path class="plus" d="M21.55 6.3h5.1M24.1 3.75v5.1" stroke-linecap="round"></path>
            </g>
            <polygon class="diamond right" points="29.9,6.3 34.8,1.4 39.7,6.3 34.8,11.2"></polygon>
            <polygon class="diamond rightmost" points="39.7,6.3 43.7,2.3 47.7,6.3 43.7,10.3"></polygon>
        </svg>
    }
}
