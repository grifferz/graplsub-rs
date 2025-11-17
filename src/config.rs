use rand::RngCore;
use serde::Deserialize;

// Config from environment.
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_base_url")]
    pub base_url: String,

    #[serde(skip)]
    pub md5_pass_salt: String,

    #[serde(default = "default_num_albums")]
    pub num_albums: u16,

    pub pass: String,

    pub user: String,

    #[serde(default = "default_playlist_name")]
    pub playlist_name: String,

    #[serde(skip)]
    pub salt: String,
}

fn default_base_url() -> String {
    "http://localhost:4533".to_string()
}

fn default_playlist_name() -> String {
    "graplsub_random_albums".to_string()
}

fn default_num_albums() -> u16 {
    100
}

/// Subsonic takes:
/// - a password and a 3 byte random salt
/// - encodes the salt as 6 hexadecimal digits
/// - appends that to the end of the password
/// - MD5 that string: md5({pass}{salt})
pub fn build_secrets(conf: &mut Config) {
    let mut bytes = [0; 3];
    rand::rng().fill_bytes(&mut bytes);
    conf.salt = hex::encode(bytes).to_string();
    conf.md5_pass_salt = format!("{:x}", md5::compute(format!("{}{}", conf.pass, conf.salt)));
}
