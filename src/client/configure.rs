use app_dirs::*;
use concurr::APP_INFO;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{AddrParseError, SocketAddr};
use std::path::Path;
use std::str::FromStr;
use toml;
use toml::de::Error as DecodeError;

#[derive(Deserialize)]
struct Node {
    address: String,
    domain:  String,
}

#[derive(Deserialize)]
struct RawConfig {
    nodes:     Vec<Node>,
    localhost: Option<bool>,
    outputs:   Option<bool>,
    verbose:   Option<bool>,
}

impl RawConfig {
    fn get_config(self) -> Result<Config, AddrParseError> {
        let mut nodes = Vec::new();
        for node in self.nodes {
            nodes.push((SocketAddr::from_str(&node.address)?, node.domain));
        }
        let mut flags = if self.outputs.unwrap_or(false) { OUTPUTS } else { 0 };
        flags |= if self.verbose.unwrap_or(false) { VERBOSE } else { 0 };
        flags |= if self.localhost.unwrap_or(true) { LOCHOST } else { 0 };
        Ok(Config { nodes, flags })
    }
}

pub const OUTPUTS: u8 = 1;
pub const VERBOSE: u8 = 2;
pub const LOCHOST: u8 = 4;

pub struct Config {
    pub nodes: Vec<(SocketAddr, String)>,
    pub flags: u8,
}

impl Config {
    pub fn get() -> Result<Config, ConfigError> {
        let mut raw = String::new();
        read_file(&get_app_dir(AppDataType::UserConfig, &APP_INFO, "config")?, &mut raw)?;
        toml::from_str::<RawConfig>(&raw)?.get_config().map_err(Into::into)
    }
}

const DEFAULT_CONFIG: &str = r#"
# A list of nodes that the client will connect to.
#
# Each element is anonymous structure that contains two fields: address, and
# domain. The address defines the location of the node from your client's
# point of view, whereas the domain defines the name written in the server's
# SSL certificate -- for security purposes.
nodes = [
    # { address = "192.168.1.2:31514", domain = "node1" },
    # { address = "192.168.1.3:31514", domain = "node2" },
]

# Wher the client should be used as a node in itself
localhost = true
# Whether the client should request the standard out / error of tasks.
outputs = true
# Whether additional information about jobs should be printed.
verbose = false
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

pub enum ConfigError {
    AppDir(AppDirsError),
    Decode(DecodeError),
    Address(AddrParseError),
    File(io::Error),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            ConfigError::AppDir(ref err) => write!(f, "XDG app dirs error: {}", err),
            ConfigError::Decode(ref err) => write!(f, "TOML config decoding error: {}", err),
            ConfigError::Address(ref err) => write!(f, "invalid address in config: {}", err),
            ConfigError::File(ref err) => write!(f, "config I/O error: {}", err),
        }
    }
}

impl From<DecodeError> for ConfigError {
    fn from(err: DecodeError) -> ConfigError { ConfigError::Decode(err) }
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> ConfigError { ConfigError::File(err) }
}

impl From<AddrParseError> for ConfigError {
    fn from(err: AddrParseError) -> ConfigError { ConfigError::Address(err) }
}

impl From<AppDirsError> for ConfigError {
    fn from(err: AppDirsError) -> ConfigError { ConfigError::AppDir(err) }
}
