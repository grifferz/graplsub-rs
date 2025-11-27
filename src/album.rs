use reqwest::Client;

use crate::api;
use crate::config;

pub async fn get(
    client: &Client,
    conf: &config::Config,
    api_ver: &str,
    id: &str,
) -> Result<(api::TopLevel, String), api::Error> {
    let url = format!(
        "{}/rest/getAlbum?u={}&t={}&s={}&f=json&v={}&c=graplsub&id={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, id
    );

    api::get(client, &url).await
}

pub fn check_get_response(resp: &api::TopLevel, json: &str) -> Result<(), api::RespParseError> {
    api::check_generic_response(resp, json)?;

    // I think we only need to check that resp.subsonic_response.album is not None as
    // everything else is enforced by the JSON structure.
    if resp.subsonic_response.album.is_none() {
        return Err(api::RespParseError::MissingAlbum {
            response: json.to_string(),
        });
    }

    Ok(())
}

pub async fn random_list(
    client: &Client,
    conf: &config::Config,
    api_ver: &str,
) -> Result<(api::TopLevel, String), api::Error> {
    let url = format!(
        "{}/rest/getAlbumList?u={}&t={}&s={}&f=json&v={}&c=graplsub&type=random&size={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, conf.num_albums
    );

    api::get(client, &url).await
}

pub fn check_list_response(resp: &api::TopLevel, json: &str) -> Result<(), api::RespParseError> {
    api::check_generic_response(resp, json)?;

    // I think we only need to check that resp.subsonic_response.album_list is not None as
    // everything else is enforced by the JSON structure.
    if resp.subsonic_response.album_list.is_none() {
        return Err(api::RespParseError::MissingAlbumList {
            response: json.to_string(),
        });
    }

    Ok(())
}
