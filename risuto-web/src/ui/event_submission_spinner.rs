use risuto_api::Event;
use std::collections::VecDeque;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct EventSubmissionSpinnerProps {
    pub events_pending_submission: VecDeque<Event>,
}

#[function_component(EventSubmissionSpinner)]
pub fn event_submission_spinner(p: &EventSubmissionSpinnerProps) -> Html {
    html! {
        <div class="dropdown">
            <button
                class={ classes!(
                    "events-pending-spinner",
                    p.events_pending_submission.is_empty().then(|| "no-events"),
                    "btn", "btn-secondary", "btn-circle", "mt-3"
                ) }
                type="button"
                data-bs-toggle="dropdown"
            >
                <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
                <span class="visually-hidden">{ "Submitting events..." }</span>
            </button>
            <ul class={ classes!(
                "events-pending-list",
                p.events_pending_submission.is_empty().then(|| "no-events"),
                "dropdown-menu", "dropdown-menu-dark"
            ) }>
                { for p.events_pending_submission.iter().map(|e| html! {
                    <li>{ format!("{:?}", e) }</li> // TODO: make events prettier
                }) }
            </ul>
        </div>
    }
}
