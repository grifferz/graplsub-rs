use reqwest::Client;

use crate::api;
use crate::config;

async fn list_all(
    client: &Client,
    conf: &config::Config,
    api_ver: &str,
) -> Result<(api::TopLevel, String), api::Error> {
    let url = format!(
        "{}/rest/getPlaylists?u={}&t={}&s={}&f=json&v={}&c=graplsub",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver
    );

    api::get(client, &url).await
}

fn check_playlist_response(resp: &api::TopLevel, json: &str) -> Result<(), api::RespParseError> {
    api::check_generic_response(resp, json)?;

    // I think we only need to check that resp.subsonic_response.playlists is not None as
    // everything else is enforced by the JSON structure.
    if resp.subsonic_response.playlists.is_none() {
        return Err(api::RespParseError::MissingPlaylists {
            response: json.to_string(),
        });
    }

    Ok(())
}

async fn delete(
    client: &Client,
    conf: &config::Config,
    api_ver: &str,
    id: &str,
) -> Result<(api::TopLevel, String), api::Error> {
    let url = format!(
        "{}/rest/deletePlaylist?u={}&t={}&s={}&f=json&v={}&c=graplsub&id={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, id
    );

    api::get(client, &url).await
}

fn check_delete_response(resp: &api::TopLevel, json: &str) -> Result<(), api::RespParseError> {
    // An empty response is expected here so just do the basic checks.
    api::check_generic_response(resp, json)?;

    Ok(())
}

async fn create(
    client: &Client,
    conf: &config::Config,
    api_ver: &str,
) -> Result<(api::TopLevel, String), api::Error> {
    let url = format!(
        "{}/rest/createPlaylist?u={}&t={}&s={}&f=json&v={}&c=graplsub&name={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, conf.playlist_name
    );

    api::get(client, &url).await
}

fn check_create_response(resp: &api::TopLevel, json: &str) -> Result<(), api::RespParseError> {
    api::check_generic_response(resp, json)?;

    // I think we only need to check that resp.subsonic_response.playlist is not None as
    // everything else is enforced by the JSON structure.
    if resp.subsonic_response.playlist.is_none() {
        return Err(api::RespParseError::MissingPlaylist {
            response: json.to_string(),
        });
    }

    Ok(())
}

pub async fn recreate(
    client: &Client,
    conf: &config::Config,
    api_ver: &str,
) -> Result<String, api::Error> {
    let (subsonic_response, json) = list_all(client, conf, api_ver).await?;

    check_playlist_response(&subsonic_response, &json)?;

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
        let (subsonic_response, json) = delete(client, conf, api_ver, &id).await?;

        check_delete_response(&subsonic_response, &json)?;
    } else {
        // Our playlist did NOT already exist, so we can just go ahead and create it as new.
    }

    let (subsonic_response, json) = create(client, conf, api_ver).await?;

    check_create_response(&subsonic_response, &json)?;

    // Safe to unwrap() because we already checked that it wasn't None.
    Ok(subsonic_response.subsonic_response.playlist.unwrap().id)
}

pub async fn update(
    client: &Client,
    conf: &config::Config,
    api_ver: &str,
    playlist_id: &str,
    song_id: &str,
) -> Result<(api::TopLevel, String), api::Error> {
    let url = format!(
        "{}/rest/updatePlaylist?u={}&t={}&s={}&f=json&v={}&c=graplsub&playlistId={}&songIdToAdd={}",
        conf.base_url, conf.user, conf.md5_pass_salt, conf.salt, api_ver, playlist_id, song_id
    );

    api::get(client, &url).await
}

pub fn check_update_response(resp: &api::TopLevel, json: &str) -> Result<(), api::RespParseError> {
    // An empty response is expected here so just do the basic checks.
    api::check_generic_response(resp, json)?;

    Ok(())
}
