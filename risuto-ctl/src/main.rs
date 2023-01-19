use anyhow::Context;
use risuto_api::{AuthToken, UserId, Uuid};

#[derive(structopt::StructOpt)]
struct Opt {
    #[structopt(short, long)]
    host: String,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(structopt::StructOpt)]
enum Command {
    /// Create a user
    CreateUser {
        /// Username
        name: String,

        /// Initial password
        initial_password: String,
    },
}

fn admin_token() -> anyhow::Result<AuthToken> {
    let tok =
        std::env::var("ADMIN_TOKEN").context("retrieving ADMIN_TOKEN environment variable")?;
    let tok = Uuid::try_parse(&tok).context("parsing ADMIN_TOKEN as an auth token")?;
    Ok(AuthToken(tok))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = <Opt as structopt::StructOpt>::from_args();

    let client = reqwest::Client::new();

    match opt.cmd {
        Command::CreateUser {
            name,
            initial_password,
        } => {
            client
                .post(format!("{}/api/admin/create-user", opt.host))
                .json(&risuto_api::NewUser::new(
                    UserId(Uuid::new_v4()),
                    name,
                    initial_password,
                ))
                .bearer_auth(admin_token()?.0)
                .send()
                .await?
                .error_for_status()?;
        }
    }

    Ok(())
}
