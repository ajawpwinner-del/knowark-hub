// 网关访问 Key：只进 Windows 凭据管理器，不落设置文件、不写日志
use rand::{distributions::Alphanumeric, Rng};

const SERVICE: &str = "KnowArkHub";
const ACCOUNT: &str = "gateway-key";

fn entry() -> anyhow::Result<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, ACCOUNT)?)
}

fn generate() -> String {
    let body: String = rand::thread_rng().sample_iter(&Alphanumeric).take(40).map(char::from).collect();
    format!("ka-{body}")
}

pub fn gateway_key() -> anyhow::Result<String> {
    let e = entry()?;
    match e.get_password() {
        Ok(k) => Ok(k),
        Err(keyring::Error::NoEntry) => {
            let k = generate();
            e.set_password(&k)?;
            Ok(k)
        }
        Err(err) => Err(err.into()),
    }
}

pub fn rotate_key() -> anyhow::Result<String> {
    let k = generate();
    entry()?.set_password(&k)?;
    Ok(k)
}
