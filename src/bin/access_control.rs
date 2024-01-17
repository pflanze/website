use std::io::Write;

use anyhow::{Result, bail, anyhow};
use clap::Parser as ClapParser;
use website::{access_control::{statements_and_methods::DO_WARN_THREAD,
                               db::access_control_transaction,
                               types::User,
                               trimcheck::{trimcheck_password, trimcheck_username,
                                           trimcheck_groupname, trimcheck_email}},
              hash_util::create_password_hash};


// use https://lib.rs/crates/inquire or https://crates.io/crates/rustyline?
fn try_ask_input(ask: &str) -> Result<Option<String>> {
    let mut outp = std::io::stdout().lock();
    write!(&mut outp, "{ask}: ")?;
    outp.flush()?;
    let inp = std::io::stdin();
    let mut line = String::new();
    if inp.read_line(&mut line)? == 0 {
        Ok(None)
    } else {
        Ok(Some(line.trim_end().into()))
    }
}

fn ask_input(ask: &str) -> Result<String> {
    try_ask_input(ask)?.ok_or_else(|| anyhow!("cancelled by user"))
}

#[derive(clap::Parser, Debug)]
/// Change the access control database.
struct Args {
    /// Action, one of "create-user", "create-group", "add" (user to
    /// group), "remove" (user from group), "passwd" (change passwd of
    /// a user) or one of the queries "user-in-group", "list" (user or
    /// group, with associations).
    #[clap(required(true))]
    action: String,
    
    /// The user with this username
    #[clap(long)]
    user: Option<String>,
    
    /// The group with this groupname
    #[clap(long)]
    group: Option<String>,
}

fn main() -> Result<()> {
    DO_WARN_THREAD.store(false, std::sync::atomic::Ordering::SeqCst);

    let args = Args::parse();
    // dbg!(&args);
    match &*args.action {
        "create-user" => {
            let username: &str =
                trimcheck_username(
                    args.user.as_ref().ok_or_else(
                        || anyhow!("need --user option"))?)?;
            if args.group.is_some() {
                bail!("can't create-user for group (and won't add it to a \
                       group at the same time and there's no primary group)")
            }
            // Run the check in a separate transaction to avoid
            // blocking other processes while reading from stdin! The
            // UNIQUE constraint will catch any race condition anyway.
            access_control_transaction(|trans| {
                if let Some(user) = trans.get_user_by_username(username)? {
                    bail!("already got user with given username, name {:?}, surname {:?}",
                          user.name, user.surname);
                }
                Ok(())
            })?;
                
            let name = ask_input("first name")?;
            let surname = ask_input("surname")?;
            let _email = ask_input("email (optional)")?;
            let email = trimcheck_email(&_email)?;
            let _password = ask_input("new password")?;
            let password = trimcheck_password(&_password)?;
            // ^ XX allow repeat ask
            let hashed_pass = create_password_hash(&password)?;
            access_control_transaction(|trans| -> Result<_> {
                let user = User {
                    id: None,
                    username: username.to_string(),
                    email: email.map(|v| v.to_string()),
                    name,
                    surname,
                    hashed_pass,
                };
                trans.insert_user(&user)?;
                Ok(())
            })
        }
        "create-group" => {
            if args.user.is_some() {
                bail!("can't create-group for user")
            }
            let groupname: &str =
                trimcheck_groupname(
                    args.group.as_ref().ok_or_else(
                        || anyhow!("need --group option"))?)?;
            access_control_transaction(|trans| {
                trans.insert_group(groupname)
            })
        }
        "add" => {
            let username: &str =
                trimcheck_username(
                    args.user.as_ref().ok_or_else(
                        || anyhow!("need --user option"))?)?;
            let groupname: &str =
                trimcheck_groupname(
                    args.group.as_ref().ok_or_else(
                        || anyhow!("need --group option"))?)?;
            access_control_transaction(|trans| {
                let user = trans.get_user_by_username(username)?.ok_or_else(
                    || anyhow!("There's no user with username {username:?}"))?;
                let group = trans.get_group_by_groupname(groupname)?.ok_or_else(
                    || anyhow!("There's no group with groupname {groupname:?}"))?;
                let user_id = user.id.expect("is from db hence has id");
                if trans.userid_has_groupname(user_id, groupname)? {
                    bail!("User {username:?} and group {groupname:?} are already connected")
                }
                trans.add_user_in_group(&user, &group)
            })
        }
        "remove" => {
            let username: &str =
                trimcheck_username(
                    args.user.as_ref().ok_or_else(
                        || anyhow!("need --user option"))?)?;
            let groupname: &str =
                trimcheck_groupname(
                    args.group.as_ref().ok_or_else(
                        || anyhow!("need --group option"))?)?;
            access_control_transaction(|trans| {
                let user = trans.get_user_by_username(username)?.ok_or_else(
                    || anyhow!("There's no user with username {username:?}"))?;
                let group = trans.get_group_by_groupname(groupname)?.ok_or_else(
                    || anyhow!("There's no group with groupname {groupname:?}"))?;
                trans.remove_user_in_group(&user, &group)
            })
        }
        "user-in-group" => {
            let username: &str =
                trimcheck_username(
                    args.user.as_ref().ok_or_else(
                        || anyhow!("need --user option"))?)?;
            let groupname: &str =
                trimcheck_groupname(
                    args.group.as_ref().ok_or_else(
                        || anyhow!("need --group option"))?)?;
            let r = access_control_transaction(|trans| -> Result<_> {
                let user = trans.get_user_by_username(username)?.ok_or_else(
                    || anyhow!("There's no user with username {username:?}"))?;
                let group = trans.get_group_by_groupname(groupname)?.ok_or_else(
                    || anyhow!("There's no group with groupname {groupname:?}"))?;
                Ok(
                    if trans.user_in_group(user.id.unwrap(),
                                           group.id.unwrap())? {
                        "yes"
                    } else {
                        "no"
                    })
            })?;
            println!("{r}");
            Ok(())
        }
        "passwd" => {
            let username: &str =
                trimcheck_username(
                    args.user.as_ref().ok_or_else(
                        || anyhow!("need --user option"))?)?;
            if args.group.is_some() {
                bail!("can't (currently) set passwd for group")
            }
            access_control_transaction(|trans| -> Result<_> {
                let _user = trans.get_user_by_username(username)?.ok_or_else(
                    || anyhow!("There's no user with username {username:?}"))?;
                Ok(())
            })?;
            let _password = ask_input("new password")?;
            let password = trimcheck_password(&_password)?;
            let hashed_pass = create_password_hash(password)?;
            access_control_transaction(|trans| -> Result<_> {
                let mut user = trans.get_user_by_username(username)?.ok_or_else(
                    || anyhow!("There's no user with username {username:?}"))?;
                user.hashed_pass = hashed_pass;
                trans.update_user(&user)?;
                Ok(())
            })
        }
        "list" => {
            todo!()
        }
        _ => bail!("invalid action name")
    }
}
