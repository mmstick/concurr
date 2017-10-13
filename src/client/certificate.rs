use app_dirs::{get_app_dir, AppDataType};
use concurr::APP_INFO;
use native_tls::Certificate;
use std::fs::File;
use std::io::Read;
use std::process::exit;

pub fn get(domain: &str) -> Certificate {
    let cert = [domain, ".der"].concat();
    let result =
        get_app_dir(AppDataType::UserConfig, &APP_INFO, &cert).map(|p| {
            File::open(p).and_then(|mut file| {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).map(|_| Certificate::from_der(&buf))
            })
        });

    match result {
        Ok(Ok(Ok(cert))) => cert,
        Ok(Ok(Err(why))) => {
            eprintln!("concurr [CRITICAL]: error parsing '{}' in cert path: {}", cert, why);
            exit(1);
        }
        Ok(Err(why)) => {
            eprintln!("concurr [CRITICAL]: error reading '{}' in cert path: {}", cert, why);
            exit(1);
        }
        Err(why) => {
            eprintln!("concurr [CRITICAL]: invalid app dir path: {}", why);
            exit(1);
        }
    }
}
