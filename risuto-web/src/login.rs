use crate::LoginInfo;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct LoginProps {
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

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            host: String::new(),
            user: String::new(),
            pass: String::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LoginMsg::HostChanged(h) => self.host = h,
            LoginMsg::UserChanged(u) => self.user = u,
            LoginMsg::PassChanged(p) => self.pass = p,
            LoginMsg::SubmitClicked => {
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
            }
        }
        html! {
            <form>
                <div class="form-group">
                    <label for="host">{ "Host" }</label>
                    <input
                        type="url"
                        class="form-control"
                        id="host"
                        placeholder="https://example.org"
                        onchange={callback_for!(HostChanged)}
                    />
                </div>
                <div class="form-group">
                    <label for="user">{ "Username" }</label>
                    <input
                        type="text"
                        class="form-control"
                        id="user"
                        placeholder="user"
                        onchange={callback_for!(UserChanged)}
                    />
                </div>
                <div class="form-group">
                    <label for="pass">{ "Password" }</label>
                    <input
                        type="password"
                        class="form-control"
                        id="pass"
                        placeholder="pass"
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
        }
    }
}
