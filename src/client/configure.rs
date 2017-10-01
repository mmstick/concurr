use app_dirs::*;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{AddrParseError, SocketAddr};
use std::path::Path;
use std::process::exit;
use std::str::FromStr;
use toml;

#[derive(Deserialize)]
struct RawConfig {
    nodes:   Vec<String>,
    outputs: bool,
}

impl RawConfig {
    fn get_config(self) -> Result<Config, AddrParseError> {
        let mut nodes = Vec::new();
        for node in self.nodes {
            nodes.push(SocketAddr::from_str(&node)?);
        }
        Ok(Config {
            nodes,
            outputs: self.outputs,
        })
    }
}

pub struct Config {
    pub nodes:   Vec<SocketAddr>,
    pub outputs: bool,
}

const APP_INFO: AppInfo = AppInfo {
    name:   "concurr",
    author: "Michael Aaron Murphy",
};

const DEFAULT_CONFIG: &str = r#"
# A list of nodes that the client will connect to
nodes = [
    "127.0.0.1:31514"
]

# Whether the client should request the standard out / error of tasks.
outputs = true
"#;

fn read_file(path: &Path, buffer: &mut String) -> io::Result<()> {
    if path.exists() {
        File::open(path)?.read_to_string(buffer).map(|_| ())
    } else {
        eprintln!("concurr [INFO]: creating {:?}", path);
        buffer.push_str(DEFAULT_CONFIG);
        let mut file = File::create(path)?;
        file.write_all(DEFAULT_CONFIG.as_bytes()).map(|_| ())
    }
}

pub fn get() -> Config {
    let mut raw = String::new();
    match get_app_dir(AppDataType::UserConfig, &APP_INFO, "config") {
        Ok(path) => if let Err(why) = read_file(&path, &mut raw) {
            eprintln!("concurr [CRITICAL]: could not create/read config file: {}", why);
            exit(1);
        },
        Err(why) => {
            eprintln!("concurr [CRITICAL]: invalid configuration path: {}", why);
            exit(1);
        }
    }

    match toml::from_str::<RawConfig>(&raw) {
        Ok(config) => match config.get_config() {
            Ok(config) => config,
            Err(why) => {
                eprintln!("concurr [CRITICAL]: {}", why);
                exit(1);
            }
        },
        Err(why) => {
            eprintln!("concurr [CRITICAL]: could not parse config: {}", why);
            exit(1);
        }
    }
}
