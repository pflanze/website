use anyhow::{Result, bail};
use website::hash_util::{verify_password, create_password_hash};


fn main() -> Result<()> {
    let prog_args: Vec<String> =
        std::env::args_os().map(|s| s.into_string().expect("non-UTF8 argument given"))
        .collect();
    let _prog = &prog_args[0];
    let _args: Vec<&str> = prog_args[1..].as_ref().iter().map(|s| s.as_ref()).collect();
    let args: &[&str] = _args.as_ref();

    match args {
        ["createpassword", password] => {
            let r = create_password_hash(password)?;
            println!("{r}");
            Ok(())
        }
        ["verifypassword", password, existing_hash] => {
            let r = verify_password(password, existing_hash)?;
            println!("{r}");
            Ok(())
        }
        ["blake3", string] => {
            let mut h = blake3::Hasher::new();
            h.update(string.as_bytes());
            let hash = h.finalize();
            println!("{hash}");
            Ok(())
        }
        _ => bail!("invalid arguments, read the source code please!")
    }
    
}
