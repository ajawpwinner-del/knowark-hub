// 局域网地址与防火墙：手机上的知舟要经局域网 IP 访问网关
use crate::proc;
use serde::Serialize;
use std::net::IpAddr;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanAddress {
    pub ip: String,
    pub adapter: String,
    pub primary: bool,
}

fn is_virtual(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["vethernet", "wsl", "docker", "hyper-v", "virtualbox", "vmware", "loopback", "tailscale", "zerotier"]
        .iter()
        .any(|k| n.contains(k))
}

pub fn lan_addresses() -> Vec<LanAddress> {
    let primary = local_ip_address::local_ip().ok();
    let mut list: Vec<LanAddress> = local_ip_address::list_afinet_netifas()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(name, ip)| match ip {
            IpAddr::V4(v4) if v4.is_private() && !is_virtual(&name) => Some(LanAddress {
                ip: v4.to_string(),
                adapter: name,
                primary: Some(ip) == primary,
            }),
            _ => None,
        })
        .collect();
    list.sort_by_key(|a| !a.primary);
    list
}

const RULE: &str = "KnowArk Hub";

pub async fn firewall_rule_exists() -> bool {
    let o = proc::run("netsh", &["advfirewall", "firewall", "show", "rule", &format!("name={RULE}")]).await;
    o.ok
}

/// 弹出系统 UAC，由用户确认后以管理员身份添加入站规则（仅专用网络）
pub async fn add_firewall_rule(port: u16) -> anyhow::Result<()> {
    let _ = proc::run("netsh", &["advfirewall", "firewall", "delete", "rule", &format!("name={RULE}")]).await;
    let args = format!(
        "advfirewall firewall add rule name=\"{RULE}\" dir=in action=allow protocol=TCP localport={port} profile=private"
    );
    let ps = format!("Start-Process netsh -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '{}'", args.replace('\'', "''"));
    let o = proc::run("powershell", &["-NoProfile", "-Command", &ps]).await;
    if !o.ok {
        anyhow::bail!("未能添加防火墙规则（可能取消了管理员授权）");
    }
    Ok(())
}
