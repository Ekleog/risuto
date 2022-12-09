#![feature(panic_info_message)]

use gloo_storage::{LocalStorage, Storage};
use risuto_api::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

mod api;
mod ui;

const KEY_LOGIN: &str = "login";

lazy_static::lazy_static! {
    static ref CLIENT: reqwest_middleware::ClientWithMiddleware = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
        .with(reqwest_retry::RetryTransientMiddleware::new_with_policy(
            reqwest_retry::policies::ExponentialBackoff::builder()
                .retry_bounds(std::time::Duration::from_secs(1), std::time::Duration::from_secs(30))
                .build_with_max_retries(u32::MAX)
                // we can stay a long time off-network, and concurrent requests are limited to 1 anyway with the submission queue
                // (except for the unauth requests but we do want to try our best actually delivering them too anyway)
        ))
        .build();
}

fn main() {
    tracing_wasm::set_as_global_default();
    yew::set_custom_panic_hook(Box::new(|info| {
        let mut message = match info.location() {
            None => format!("Panic occurred at unknown place:\n"),
            Some(l) => format!(
                "Panic occurred at file '{}' line '{}':\n",
                l.file(),
                l.line()
            ),
        };
        // TODO: when replacing this with console_error_panic_hook::stringify,
        // we can stop depending on nightly
        if let Some(m) = info.message() {
            let _ = std::fmt::write(&mut message, *m);
        } else {
            message += "failed recovering a message from the panic";
        }
        let document = web_sys::window()
            .expect("no web_sys window")
            .document()
            .expect("no web_sys document");
        document
            .get_element_by_id("body")
            .expect("no #body element")
            .set_inner_html(include_str!("../panic-page.html"));
        document
            .get_element_by_id("panic-message")
            .expect("no #panic-message element")
            .set_inner_html(&message);
    }));
    yew::Renderer::<Main>::new().render();
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LoginInfo {
    host: String,
    user: String,
    token: AuthToken,
}

pub enum MainMsg {
    Login(LoginInfo),
    Logout,
}

pub struct Main {
    login: Option<LoginInfo>,
    logout: Option<LoginInfo>, // info saved from login info, without the token
}

impl Component for Main {
    type Message = MainMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Main {
            login: LocalStorage::get(KEY_LOGIN).ok(),
            logout: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            MainMsg::Login(info) => {
                LocalStorage::set(KEY_LOGIN, &info)
                    .expect("failed saving login info to LocalStorage");
                self.login = Some(info);
            }
            MainMsg::Logout => {
                // TODO: warn the user upon logout that unsynced changes may be lost
                let login = self.login.take().expect("got logout while not logged in");
                spawn_local(api::unauth(login.host.clone(), login.token));
                LocalStorage::delete(KEY_LOGIN);
                self.logout = Some(LoginInfo {
                    host: login.host,
                    user: login.user,
                    token: AuthToken::stub(),
                });
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        match &self.login {
            None => html! {
                <div class="container">
                    <ui::Login
                        info={self.logout.clone()}
                        on_authed={ctx.link().callback(MainMsg::Login)}
                    />
                </div>
            },
            Some(login) => html! {
                <ui::App
                    login={login.clone()}
                    on_logout={ctx.link().callback(|_| MainMsg::Logout)}
                />
            },
        }
    }
}
