use std::process::ExitCode;

mod album;
mod api;
mod config;
mod playlist;

#[tokio::main]
async fn main() -> ExitCode {
    let mut conf = envy::prefixed("GRAPLSUB_")
        .from_env::<config::Config>()
        .expect(
            "Please provide all required env vars, minimum GRAPLSUB_PASS \
            and GRAPLSUB_USER, but see also GRAPLSUB_BASE_URL, GRAPLSUB_NUM_ALBUMS \
            and GRAPLSUB_PLAYLIST_NAME",
        );

    config::build_secrets(&mut conf);

    if conf.num_albums > 500 {
        eprintln!(
            "GRAPLSUB_NUM_ALBUMS too big ({}). Setting to 500.",
            conf.num_albums
        );
        conf.num_albums = 500;
    }

    let api_ver: &'static str = "1.14.0";

    let client = api::create_client().expect("Failed to create HTTP client");

    let playlist_id = match playlist::recreate(&client, &conf, api_ver).await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
    };

    let (subsonic_response, json) = match album::random_list(&client, &conf, api_ver).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
    };

    match album::check_list_response(&subsonic_response, &json) {
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
                match album::get(&client, &conf, api_ver, &album.id).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("{}", e);
                        return ExitCode::from(1);
                    }
                };

            match album::check_get_response(&subsonic_response, &json) {
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
                        match playlist::update(&client, &conf, api_ver, &playlist_id, &song.id)
                            .await
                        {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("{}", e);
                                return ExitCode::from(1);
                            }
                        };

                    match playlist::check_update_response(&subsonic_response, &json) {
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
