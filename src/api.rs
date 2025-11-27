use format_serde_error::SerdeError;
use reqwest::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

// Infrastructure needed to be a Subsonic API client.

// Structures that will be deserialsied from JSON.

/// A playlist. We only need its name and ID as we never check what's actually in it.
#[derive(Debug, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
}

/// For calls that return a list of playlists.
#[derive(Debug, Deserialize)]
pub struct Playlists {
    // There'll be an empty "playlists {}" block if there's no playlists.
    pub playlist: Option<Vec<Playlist>>,
}

/// A song. Right now we only keep track of song IDs.
#[derive(Debug, Deserialize)]
pub struct Song {
    pub id: String,
}

/// An individual album's details as returned by getAlbum. We only care about the album ID and the
/// list of songs on it.
#[derive(Debug, Deserialize)]
pub struct Album {
    pub id: String,
    // This one will only be present when the individual album is requested.
    pub song: Option<Vec<Song>>,
}

/// This is a list of albums as returned by albumList.
#[derive(Debug, Deserialize)]
pub struct AlbumList {
    // There'll be an empty "album {}" block if there's no albums.
    pub album: Option<Vec<Album>>,
}

/// The main response structure. Usually there'll only be one of these members present, depending
/// on which API call was used.
#[derive(Debug, Deserialize)]
pub struct SubsonicResponse {
    // This one can only come back after requesting an album.
    pub album: Option<Album>,
    // This won't be here if it wasn't a getAlbumList query.
    #[serde(rename(deserialize = "albumList"))]
    pub album_list: Option<AlbumList>,
    // Again, this one can only come back after creating a playlist.
    pub playlist: Option<Playlist>,
    // This won't be here if it wasn't a getPlaylists query.
    pub playlists: Option<Playlists>,
    status: String,
}

/// Outer wrapper returned in every API response.
#[derive(Debug, Deserialize)]
pub struct TopLevel {
    #[serde(rename(deserialize = "subsonic-response"))]
    pub subsonic_response: SubsonicResponse,
}

// Error handling.

/// Wrapper for all kinds of errors that can be bubbled up to main().
#[derive(Debug, Error)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Response parsing error: {0}")]
    RespParse(#[from] RespParseError),

    #[error(transparent)]
    SerdeError(#[from] format_serde_error::SerdeError),
}

/// Errors related to parsing API responses. <ost of these never get triggered because the response
/// won't deserialise if it's incorrect.
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

pub fn create_client() -> Result<Client, reqwest::Error> {
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

/// An HTTP GET request to the API.
pub async fn get(client: &Client, url: &str) -> Result<(TopLevel, String), Error> {
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
            // Take a copy of the URL and remove the query string as that contains auth info (user,
            // md5_pass_salt and salt) and isn't the problem here anyway.
            let mut report_url = response.url().clone();
            report_url.set_query(None);
            Err(Error::NotFound {
                resource: report_url.to_string(),
            })
        }
        _ => Err(Error::Network(response.error_for_status().unwrap_err())),
    }
}

/// Basic checks that are common to every API response.
pub fn check_generic_response(resp: &TopLevel, json: &str) -> Result<(), RespParseError> {
    if resp.subsonic_response.status != "ok" {
        return Err(RespParseError::ResponseNotOk {
            response: json.to_string(),
        });
    }

    Ok(())
}
