use futures::FutureExt;
use risuto_client::api::{AuthToken, NewSession};
use yew::prelude::*;

use crate::{
    api::{self, ApiError},
    LoginInfo,
};

#[derive(Clone, PartialEq, Properties)]
pub struct LoginProps {
    pub info: Option<LoginInfo>,
    pub on_authed: Callback<LoginInfo>,
}

pub struct Login {
    host: String,
    user: String,
    pass: String,
    error: Option<&'static str>,
}

pub enum LoginMsg {
    HostChanged(String),
    UserChanged(String),
    PassChanged(String),
    SubmitClicked,
    Authed(String, String, Result<AuthToken, ApiError>),
}

fn get_device() -> anyhow::Result<String> {
    Ok(format!("{}", whoami::platform()))
    // TODO: add more details, see https://github.com/ardaku/whoami/issues/52
}

impl Component for Login {
    type Message = LoginMsg;
    type Properties = LoginProps;

    fn create(ctx: &Context<Self>) -> Self {
        let (host, user) = match &ctx.props().info {
            Some(i) => (i.host.clone(), i.user.clone()),
            None => (String::new(), String::new()),
        };
        Self {
            host,
            user,
            pass: String::new(),
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LoginMsg::HostChanged(h) => self.host = h,
            LoginMsg::UserChanged(u) => self.user = u,
            LoginMsg::PassChanged(p) => self.pass = p,
            LoginMsg::SubmitClicked => {
                let device = get_device().unwrap_or_else(|_| String::from("Unknown device"));
                let session = NewSession {
                    user: self.user.clone(),
                    password: self.pass.clone(),
                    device,
                };
                let host = self.host.clone();
                let user = self.user.clone();
                ctx.link().send_future(
                    api::auth(self.host.clone(), session)
                        .map(move |token| LoginMsg::Authed(host, user, token)),
                );
                // TODO: show some kind of indicator that auth is in progress?
                // making host/user disabled would also avoid the need of passing them through Authed
                // TODO: reuse the Client built in App
                return false;
            }
            LoginMsg::Authed(host, user, Ok(token)) => {
                ctx.props().on_authed.emit(LoginInfo { host, user, token });
                return false;
            }
            LoginMsg::Authed(_, _, Err(ApiError::SendingRequest(err))) => {
                tracing::error!(?err, "login failed sending request");
                self.error = Some("Failed connecting to server. Maybe the URL is mistyped?");
            }
            LoginMsg::Authed(_, _, Err(ApiError::ParsingResponse(err))) => {
                tracing::error!(?err, "login failed parsing response");
                self.error = Some(
                    "The server seems to not be a valid risuto server. Maybe the URL is mistyped?",
                );
            }
            LoginMsg::Authed(_, _, Err(ApiError::PermissionDenied)) => {
                tracing::error!("login failed due to permission denied");
                self.error =
                    Some("Failed to authenticate. Please check your username and password.");
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        macro_rules! callback_for {
            ($msg:ident) => {
                ctx.link().callback(|e: web_sys::Event| {
                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                    LoginMsg::$msg(input.value())
                })
            };
        }
        html! {<>
            <div class="text-center my-4">
                <h1>{ "Login" }</h1>
            </div>
            {for self.error.map(|err| html! {
                <div class="alert alert-danger">
                    { err }
                </div>
            })}
            <form class="login-form">
                <div class="input-group mb-3">
                    <label class="input-group-text col-xl-1" for="host">{ "Host" }</label>
                    <input
                        type="url"
                        class="form-control form-control-lg"
                        id="host"
                        placeholder="https://example.org"
                        value={self.host.clone()}
                        onchange={callback_for!(HostChanged)}
                    />
                </div>
                <div class="input-group mb-3">
                    <label class="input-group-text col-xl-1" for="user">{ "Username" }</label>
                    <input
                        type="text"
                        class="form-control form-control-lg"
                        id="user"
                        placeholder="user"
                        value={self.user.clone()}
                        onchange={callback_for!(UserChanged)}
                    />
                </div>
                <div class="input-group mb-3">
                    <label class="input-group-text col-xl-1" for="pass">{ "Password" }</label>
                    <input
                        type="password"
                        class="form-control form-control-lg"
                        id="pass"
                        placeholder="pass"
                        value={self.pass.clone()}
                        onchange={callback_for!(PassChanged)}
                    />
                </div>
                <input
                    type="button"
                    class="btn btn-primary"
                    onclick={ctx.link().callback(|_| LoginMsg::SubmitClicked)}
                    value="Connect"
                />
            </form>
        </>}
    }
}
