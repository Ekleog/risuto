use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct SettingsMenuProps {
    pub on_logout: Callback<()>,
}

#[function_component(SettingsMenu)]
pub fn settings_menu(p: &SettingsMenuProps) -> Html {
    html! {
        <div class="dropdown">
            <button
                type="button"
                class="btn btn-light btn-circle m-3 bi-btn bi-gear-fill fs-6"
                title="Settings"
                data-bs-toggle="dropdown"
            >
            </button>
            <ul class="dropdown-menu dropdown-menu-dark mt-3">
                <li><a class="dropdown-item" href="#" onclick={p.on_logout.reform(|_| ())}>
                    <span class="bi-power me-2" aria-hidden="true"></span>
                    {"Logout"}
                </a></li>
            </ul>
        </div>
    }
}
