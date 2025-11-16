use format_serde_error::SerdeError;
use rand::RngCore;
use reqwest::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use std::process::ExitCode;
use std::time::Duration;
use thiserror::Error;

// Config from environment.
#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default = "default_base_url")]
    base_url: String,

    #[serde(skip)]
    md5_pass_salt: String,

    #[serde(default = "default_num_albums")]
    num_albums: u16,

    pass: String,

    user: String,

    #[serde(default = "default_playlist_name")]
    playlist_name: String,

    #[serde(skip)]
    salt: String,
}

fn default_base_url() -> String {
    "http://localhost:4533".to_string()
}

fn default_playlist_name() -> String {
    "graplsub_random_albums".to_string()
}

fn default_num_albums() -> u16 {
    500
}

// Structures populated from JSON data.
#[derive(Debug, Deserialize)]
pub struct Playlist {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct Playlists {
    // There'll be an empty "playlists {}" block if there's no playlists.
    playlist: Option<Vec<Playlist>>,
}

#[derive(Debug, Deserialize)]
pub struct Song {
    id: String,
}

#[derive(Debug, Deserialize)]
pub struct Album {
    id: String,
    // This one will only be present when the individual album ios requested.
    song: Option<Vec<Song>>,
}

#[derive(Debug, Deserialize)]
pub struct AlbumList {
    // There'll be an empty "album {}" block if there's no albums.
    album: Option<Vec<Album>>,
}

#[derive(Debug, Deserialize)]
pub struct SubsonicResponse {
    // This one can only come back after requesting an album.
    album: Option<Album>,
    // This won't be here if it wasn't a getAlbumList query.
    #[serde(rename(deserialize = "albumList"))]
    album_list: Option<AlbumList>,
    // Again, this one can only come back after creating a playlist.
    playlist: Option<Playlist>,
    // This won't be here if it wasn't a getPlaylists query.
    playlists: Option<Playlists>,
    status: String,
}

#[derive(Debug, Deserialize)]
pub struct TopLevel {
    #[serde(rename(deserialize = "subsonic-response"))]
    subsonic_response: SubsonicResponse,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Response parsing error: {0}")]
    RespParse(#[from] RespParseError),

    // #[error("serde_json error: {0}")]
    #[error(transparent)]
    SerdeError(#[from] format_serde_error::SerdeError),
}

#[derive(Debug, Error)]
pub enum RespParseError {
    #[error("Subsonic response was missing an album: {response}")]
    MissingAlbum { response: String },

    #[error("Subsonic response was missing an albumList: {response}")]
    MissingAlbumList { response: String },

    #[error("Subsonic response was missing a playlist: {response}")]
    MissingPlaylist { response: String },

    #[error("Subsonic response was missing a playlists: {response}")]
    MissingPlaylists { response: String },

    #[error("Subsonic response did not have 'ok' status: {response}")]
    ResponseNotOk { response: String },
}

#[tokio::main]
async fn main() -> ExitCode {
    let mut conf = envy::prefixed("GRAPLSUB_").from_env::<Config>().expect(
        "Please provide all required env vars, minimum GRAPLSUB_PASS \
            and GRAPLSUB_USER, but see also GRAPLSUB_BASE)URL, GRAPLSUB_NUM_ALBUMS \
            and GRAPLSUB_PLAYLIST_NAME",
    );

    build_secrets(&mut conf);

    if conf.num_albums > 500 {
        eprintln!(
            "GRAPLSUB_NUM_ALBUMS too big ({}). Setting to 500.",
            conf.num_albums
        );
        conf.num_albums = 500;
    }

    let api_ver: &'static str = "1.14.0";

    let client = create_client().expect("Failed to create HTTP client");

    let playlist_id = match recreate_playlist(&client, &conf, api_ver).await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
    };

    let (subsonic_response, json) = match random_album_list(&client, &conf, api_ver).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
    };

    match check_subsonic_albumlist_response(&subsonic_response, &json) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
    }

    // Safe to unwrap() album_list because we already checked it was Some(), but album can still be
    // None.
    if let Some(albums) = &subsonic_response
        .subsonic_response
        .album_list
        .unwrap()
        .album
    {
        for album in albums {
            let (subsonic_response, json) =
                match get_album(&client, &conf, api_ver, &album.id).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("{}", e);
                        return ExitCode::from(1);
                    }
                };

            match check_subsonic_get_album_response(&subsonic_response, &json) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{}", e);
                    return ExitCode::from(1);
                }
            }

            // Safe to unwrap() song because we already checked it was Some().
            if let Some(songs) = &subsonic_response.subsonic_response.album.unwrap().song {
                for song in songs {
                    // eprintln!("Song: {}", song.id);
                    let (subsonic_response, json) =
                        match add_song(&client, &conf, api_ver, &playlist_id, &song.id).await {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("{}", e);
                                return ExitCode::from(1);
                            }
                        };

                    match check_subsonic_add_song_response(&subsonic_response, &json) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("{}", e);
                            return ExitCode::from(1);
                        }
                    }
                }
            }
        }
    }

    ExitCode::from(0)
}

/// Subsonic takes:
/// - a password and a 3 byte random salt
/// - encodes the salt as 3 hexadecimal digits
/// - appends that to the end of the password
/// - MD5 that string: md5({pass}{sat})
fn build_secrets(conf: &mut Config) {
    let mut bytes = [0; 3];
    rand::rng().fill_bytes(&mut bytes);
    conf.salt = hex::encode(bytes).to_string();
    conf.md5_pass_salt = format!("{:x}", md5::compute(format!("{}{}", conf.pass, conf.salt)));
}

async fn recreate_playlist(
    client: &Client,
    conf: &Config,
    api_ver: &str,
) -> Result<String, ApiError> {
    let (subsonic_response, json) = playlists(client, conf, api_ver).await?;

    check_subsonic_playlist_response(&subsonic_response, &json)?;

    let mut my_list_id: Option<String> = None;

    // It's safe to unwrap() playlists as this was already chcked, but if there are no playlists
    // then the "playlist" within it will be None.
    if let Some(lists) = &subsonic_response
        .subsonic_response
        .playlists
        .unwrap()
        .playlist
    {
        for playlist in lists {
            if playlist.name == conf.playlist_name {
                my_list_id = Some(playlist.id.clone());
                break;
            }
        }
    }

    if let Some(id) = my_list_id {
        // Our playlist did already exist, so delete it.
        let (subsonic_response, json) = delete_playlist(client, conf, api_ver, &id).await?;

        check_subsonic_delete_playlist_response(&subsonic_response, &json)?;
    } else {
        // Our playlist did NOT already exist, so we can just go ahead and create it as new.
    }

    let (subsonic_response, json) = create_playlist(client, conf, api_ver).await?;

    check_subsonic_create_playlist_response(&subsonic_response, &json)?;

    // Safe to unwrap() because we already checked that it wasn't None.
    Ok(subsonic_response.subsonic_response.playlist.unwrap().id)
}

async fn delete_playlist(
    client: &Client,
    conf: &Config,
    api_ver: &str,
    id: &str,
) -> Result<(TopLevel, String), ApiError> {
    let url = format!(
        "{}/rest/deletePlaylist?u={}&t={}&s={}&f=json&v={}&c=graplsub&id={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, id
    );

    let response = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {
            let text = response.text().await?;
            let obj: TopLevel = serde_json::from_str(&text)
                .map_err(|err| SerdeError::new(text.to_string(), err))?;
            Ok((obj, text))
        }
        StatusCode::NOT_FOUND => {
            // Take a copy of the URL and remove thw query string as that contains auth info (user,
            // md5_pass_salt and salt) and isn't the problem here anyway.
            let mut report_url = response.url().clone();
            report_url.set_query(None);
            Err(ApiError::NotFound {
                resource: report_url.to_string(),
            })
        }
        _ => Err(ApiError::Network(response.error_for_status().unwrap_err())),
    }
}

fn check_subsonic_delete_playlist_response(
    resp: &TopLevel,
    json: &str,
) -> Result<(), RespParseError> {
    // An empty response is expected here so just do the basic checks.
    check_generic_subsonic_response(resp, json)?;

    Ok(())
}

async fn create_playlist(
    client: &Client,
    conf: &Config,
    api_ver: &str,
) -> Result<(TopLevel, String), ApiError> {
    let url = format!(
        "{}/rest/createPlaylist?u={}&t={}&s={}&f=json&v={}&c=graplsub&name={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, conf.playlist_name
    );

    let response = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {
            let text = response.text().await?;
            let obj: TopLevel = serde_json::from_str(&text)
                .map_err(|err| SerdeError::new(text.to_string(), err))?;
            Ok((obj, text))
        }
        StatusCode::NOT_FOUND => {
            // Take a copy of the URL and remove thw query string as that contains auth info (user,
            // md5_pass_salt and salt) and isn't the problem here anyway.
            let mut report_url = response.url().clone();
            report_url.set_query(None);
            Err(ApiError::NotFound {
                resource: report_url.to_string(),
            })
        }
        _ => Err(ApiError::Network(response.error_for_status().unwrap_err())),
    }
}

fn check_subsonic_create_playlist_response(
    resp: &TopLevel,
    json: &str,
) -> Result<(), RespParseError> {
    check_generic_subsonic_response(resp, json)?;

    // I think we only need to check that resp.subsonic_response.playlist is not None as
    // everything else is enforced by the JSON structure.
    if resp.subsonic_response.playlist.is_none() {
        return Err(RespParseError::MissingPlaylist {
            response: json.to_string(),
        });
    }

    Ok(())
}

async fn add_song(
    client: &Client,
    conf: &Config,
    api_ver: &str,
    playlist_id: &str,
    song_id: &str,
) -> Result<(TopLevel, String), ApiError> {
    let url = format!(
        "{}/rest/updatePlaylist?u={}&t={}&s={}&f=json&v={}&c=graplsub&playlistId={}&songIdToAdd={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, playlist_id, song_id
    );

    let response = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {
            let text = response.text().await?;
            let obj: TopLevel = serde_json::from_str(&text)
                .map_err(|err| SerdeError::new(text.to_string(), err))?;
            Ok((obj, text))
        }
        StatusCode::NOT_FOUND => {
            // Take a copy of the URL and remove thw query string as that contains auth info (user,
            // md5_pass_salt and salt) and isn't the problem here anyway.
            let mut report_url = response.url().clone();
            report_url.set_query(None);
            Err(ApiError::NotFound {
                resource: report_url.to_string(),
            })
        }
        _ => Err(ApiError::Network(response.error_for_status().unwrap_err())),
    }
}

fn check_subsonic_add_song_response(resp: &TopLevel, json: &str) -> Result<(), RespParseError> {
    // An empty response is expected here so just do the basic checks.
    check_generic_subsonic_response(resp, json)?;

    Ok(())
}

async fn playlists(
    client: &Client,
    conf: &Config,
    api_ver: &str,
) -> Result<(TopLevel, String), ApiError> {
    let url = format!(
        "{}/rest/getPlaylists?u={}&t={}&s={}&f=json&v={}&c=graplsub",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver
    );

    let response = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {
            let text = response.text().await?;
            // eprintln!("{}", text);
            let obj: TopLevel = serde_json::from_str(&text)
                .map_err(|err| SerdeError::new(text.to_string(), err))?;
            Ok((obj, text))
        }
        StatusCode::NOT_FOUND => {
            // Take a copy of the URL and remove thw query string as that contains auth info (user,
            // md5_pass_salt and salt) and isn't the problem here anyway.
            let mut report_url = response.url().clone();
            report_url.set_query(None);
            Err(ApiError::NotFound {
                resource: report_url.to_string(),
            })
        }
        _ => Err(ApiError::Network(response.error_for_status().unwrap_err())),
    }
}

fn check_generic_subsonic_response(resp: &TopLevel, json: &str) -> Result<(), RespParseError> {
    if resp.subsonic_response.status != "ok" {
        return Err(RespParseError::ResponseNotOk {
            response: json.to_string(),
        });
    }

    Ok(())
}

fn check_subsonic_albumlist_response(resp: &TopLevel, json: &str) -> Result<(), RespParseError> {
    check_generic_subsonic_response(resp, json)?;

    // I think we only need to check that resp.subsonic_response.album_list is not None as
    // everything else is enforced by the JSON structure.
    if resp.subsonic_response.album_list.is_none() {
        return Err(RespParseError::MissingAlbumList {
            response: json.to_string(),
        });
    }

    Ok(())
}

fn check_subsonic_playlist_response(resp: &TopLevel, json: &str) -> Result<(), RespParseError> {
    check_generic_subsonic_response(resp, json)?;

    // I think we only need to check that resp.subsonic_response.playlists is not None as
    // everything else is enforced by the JSON structure.
    if resp.subsonic_response.playlists.is_none() {
        return Err(RespParseError::MissingPlaylists {
            response: json.to_string(),
        });
    }

    Ok(())
}

fn create_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        // Total request timeout
        .timeout(Duration::from_secs(30))
        // Connection timeout
        .connect_timeout(Duration::from_secs(5))
        // Connection pool timeout
        .pool_idle_timeout(Duration::from_secs(90))
        // Max idle connections
        .pool_max_idle_per_host(10)
        .user_agent("graplsub/0.1.0")
        .build()
}

async fn random_album_list(
    client: &Client,
    conf: &Config,
    api_ver: &str,
) -> Result<(TopLevel, String), ApiError> {
    let url = format!(
        "{}/rest/getAlbumList?u={}&t={}&s={}&f=json&v={}&c=graplsub&type=random&size={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, conf.num_albums
    );

    let response = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {
            let text = response.text().await?;
            let obj: TopLevel = serde_json::from_str(&text)
                .map_err(|err| SerdeError::new(text.to_string(), err))?;
            Ok((obj, text))
        }
        StatusCode::NOT_FOUND => {
            // Take a copy of the URL and remove thw query string as that contains auth info (user,
            // md5_pass_salt and salt) and isn't the problem here anyway.
            let mut report_url = response.url().clone();
            report_url.set_query(None);
            Err(ApiError::NotFound {
                resource: report_url.to_string(),
            })
        }
        _ => Err(ApiError::Network(response.error_for_status().unwrap_err())),
    }
}

async fn get_album(
    client: &Client,
    conf: &Config,
    api_ver: &str,
    id: &str,
) -> Result<(TopLevel, String), ApiError> {
    let url = format!(
        "{}/rest/getAlbum?u={}&t={}&s={}&f=json&v={}&c=graplsub&id={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, id
    );

    let response = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {
            let text = response.text().await?;
            let obj: TopLevel = serde_json::from_str(&text)
                .map_err(|err| SerdeError::new(text.to_string(), err))?;
            Ok((obj, text))
        }
        StatusCode::NOT_FOUND => {
            // Take a copy of the URL and remove thw query string as that contains auth info (user,
            // md5_pass_salt and salt) and isn't the problem here anyway.
            let mut report_url = response.url().clone();
            report_url.set_query(None);
            Err(ApiError::NotFound {
                resource: report_url.to_string(),
            })
        }
        _ => Err(ApiError::Network(response.error_for_status().unwrap_err())),
    }
}

fn check_subsonic_get_album_response(resp: &TopLevel, json: &str) -> Result<(), RespParseError> {
    check_generic_subsonic_response(resp, json)?;

    // I think we only need to check that resp.subsonic_response.album is not None as
    // everything else is enforced by the JSON structure.
    if resp.subsonic_response.album.is_none() {
        return Err(RespParseError::MissingAlbum {
            response: json.to_string(),
        });
    }

    Ok(())
}
