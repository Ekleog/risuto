use risuto_client::api::Action;
use std::collections::VecDeque;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct ActionSubmissionSpinnerProps {
    pub actions_pending_submission: VecDeque<Action>,
}

#[function_component(ActionSubmissionSpinner)]
pub fn action_submission_spinner(p: &ActionSubmissionSpinnerProps) -> Html {
    html! {
        <div class="float-above dropdown">
            <button
                class={ classes!(
                    "events-pending-spinner",
                    p.actions_pending_submission.is_empty().then(|| "no-events"),
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
                p.actions_pending_submission.is_empty().then(|| "no-events"),
                "dropdown-menu", "dropdown-menu-dark"
            ) }>
                { for p.actions_pending_submission.iter().map(|e| html! {
                    <li>{ format!("{:?}", e) }</li> // TODO: make events prettier
                }) }
            </ul>
        </div>
    }
}
