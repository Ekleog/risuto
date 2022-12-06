use yew::prelude::*;
use crate::ui;

#[derive(Clone, PartialEq, Properties)]
pub struct OfflineBannerProps {
    pub connection_state: ui::ConnState,
}

#[function_component(OfflineBanner)]
pub fn offline_banner(p: &OfflineBannerProps) -> Html {
    let offline = !matches!(p.connection_state, ui::ConnState::Connected);
    let offline_banner_message = match p.connection_state {
        ui::ConnState::Disconnected => "Currently offline. Trying to reconnect...",
        ui::ConnState::WebsocketConnected(_) | ui::ConnState::Connected => {
            "Currently reconnecting..."
        }
    };

    html! {
        <div
            class={ classes!(
                "offline-banner", (!offline).then(|| "is-online"),
                "d-flex", "align-items-center"
            ) }
            aria-hidden={ if offline { "false" } else { "true" } }
        >
            <div class="spinner-border spinner-border-sm m-2" role="status"></div>
            <div>{ offline_banner_message }</div>
        </div>
    }
}
