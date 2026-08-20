use std::net::UdpSocket;

pub fn get_local_ip() -> String {
    // In Rust 1.97, UdpSocket::connect is a method, not a static function.
    // Bind to any address, then connect to determine local interface.
    UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|s| s.connect("8.8.8.8:80").ok().map(|_| s))
        .and_then(|s| s.local_addr().ok())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

pub fn get_public_ip() -> String {
    for url in [
        "https://api.ipify.org",
        "https://api64.ipify.org",
        "https://checkip.amazonaws.com",
    ] {
        if let Ok(resp) = ureq::get(url)
            .set("User-Agent", "KidsTime/1.0")
            .timeout(std::time::Duration::from_secs(5))
            .call()
        {
            if let Ok(body) = resp.into_string() {
                let ip = body.trim().to_string();
                if !ip.is_empty() && ip.len() <= 45 {
                    return ip;
                }
            }
        }
    }
    String::new()
}

pub fn update_duckdns(domain: &str, token: &str, ip: &str) -> bool {
    if domain.is_empty() || token.is_empty() || ip.is_empty() {
        return false;
    }
    let full_domain = if domain.contains('.') {
        domain.to_string()
    } else {
        format!("{}.duckdns.org", domain)
    };
    let url = format!(
        "https://www.duckdns.org/update?domains={}&token={}&ip={}",
        full_domain, token, ip
    );
    ureq::get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .ok()
        .and_then(|r| r.into_string().ok())
        .map(|s| s.trim() == "OK")
        .unwrap_or(false)
}
