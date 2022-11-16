use yew::prelude::*;

use crate::LoginInfo;

#[derive(Clone, PartialEq, Properties)]
pub struct LoginProps {
    pub info: Option<LoginInfo>,
    pub on_submit: Callback<LoginInfo>,
}

pub struct Login {
    host: String,
    user: String,
    pass: String,
}

pub enum LoginMsg {
    HostChanged(String),
    UserChanged(String),
    PassChanged(String),
    SubmitClicked,
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
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LoginMsg::HostChanged(h) => self.host = h,
            LoginMsg::UserChanged(u) => self.user = u,
            LoginMsg::PassChanged(p) => self.pass = p,
            LoginMsg::SubmitClicked => {
                // TODO: hash password (username + "risuto" + password), validate login
                ctx.props().on_submit.emit(LoginInfo {
                    host: self.host.clone(),
                    user: self.user.clone(),
                    pass: self.pass.clone(),
                });
                return false;
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
                <button
                    type="submit"
                    class="btn btn-primary"
                    onclick={ctx.link().callback(|_| LoginMsg::SubmitClicked)}
                >
                    { "Connect" }
                </button>
            </form>
        </>}
    }
}
