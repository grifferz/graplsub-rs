# graplsub-rs

Generate A Random Album Playlist for Subsonic-compatible servers.

## Introduction

I was [lamenting] the general state of music player support for proper random
album playlists, i.e. a large playlist featuring one complete album at a time.

I was previously using [MPD] and had resorted to writing a shell script to
maintain a playlist with `mpc` for any of my clients to pick up. I've now
switched to using [Navidrome], the Subsonic-compatible server, and threatened
to write a similar program to do the same using its [API].

Well, here it is.

[lamenting]: https://social.bitfolk.com/@grifferz/115543287855865431
[MPD]: https://www.musicpd.org/
[Navidrome]: https://www.navidrome.org/
[API]: https://www.subsonic.org/pages/api.jsp

## Installation

This is a Rust application, so after cloning this repo you can build it from
source with `cargo` like:

```bash
$ cargo build --release
```

The binary should then be found in the `target/release/` directory. Put it on
your path or run it from anywhere.

## Prerequisites

- A Subsonic-compatible server with an open API endpoint that supports at
  least API version 1.14.0. I have only tested against Navidrome which is
  currently on API version 1.16.1.
- More than one album 😀

## Basic theory of operation

This thing quite simply uses the Subsonic API to poke songs into a playlist.

`graplsub` requests a large number of albums at random and then pushes the
songs in, in order from each album. It does 100 albums by default (if you have
that many), but can do up to 500 (API limitation). For me, 500 albums is
usually around 5 days of music if left constantly playing.

Every time you run `graplsub`, it will delete the playlist and create it again
with a new random order. This should not be a problem as Subsonic clients
usually load a playlist into a play _queue_, and play that. If you have
modified your library or just want to shuffle things again you can clear your
play queue and load the playlist in again.

Personally I have `graplsub` run regularly so new music I add gets found.

## Usage

All configuration is currently by environment variables.

### Required environment variables

#### `GRAPLSUB_USER`

Your user name in the Subsonic server. The playlist will be owned by this
user.

#### `GRAPLSUB_PASS`

Your password for accessing your Subsonic server. The user name and password
are the same as what you would have put in any Subsonic client you use and
have the same security implications.

### Optional environment variables

#### `GRAPLSUB_BASE_URL`

Default: `http://localhost:4533`

This should be the same as the host and port that you have put into any
Subsonic client that you use.

Don't put a trailing `/` — that will result in `graplsub` getting a HTML page
from the Subsonic server instead of the JSON API, and it will complain about
that.

I have not tested TLS (https) connections but I think they should work. Let me
know!

#### `GRAPLSUB_NUM_ALBUMS`

Default: `100`

How many albums full of songs to put in to the playlist each time.

Each time `graplsub` runs it will delete the playlist and create it again with
this many albums worth of songs. Obviously albums have different numbers of
tracks so it's hard to predict how big the playlist will be.

All tracks from each album will be added, in disc and track order.

The Subsonic API has a limit of 500 albums per request. It wouldn't be hard to
paginate through the entire library to get all albums, but I didn't feel like
there was much need — for me, 500 albums is usually more than 5 days of
continuous music.

#### `GRAPLSUB_PLAYLIST_NAME`

Default: `graplsub_random_albums`

The name of the playlist to generate. This is going to be deleted each time
you run `graplsub` so you wouldn't want to use one that is curated in any
other way.

## Limitations

`graplsub` works well enough for my needs now but there are a few things I can
think of to improve either as a matter of pride or if nerd-sniped into it.
These include:

- Batching of song submissions. At the moment each song is pushed into the
  playlist with an individual HTTP `GET` request. Since an album will often
  have tens of tracks on it this obviously means that for 500 albums that's
  thousands of HTTP requests each time to make a playlist. The Subsonic API
  does allow multiple songs to be added each time, though this is done by
  adding query parameters onto a `GET` request (don't blame me, I didn't
  design this…), so there is a limit to how far you could go with that. Even
  so, on my network it takes about 12 seconds to add approximately 2,000
  tracks.
- It might be nice to accept command-line arguments or a config file instead
  of just environment variables.
- Public playlist option? By default new Subsonic playlists are private,
  meaning only visible to clients logged in using your own credentials. You
  can make them public and then all other users on your server can see them
  (only useful if they can also see your library, of course). You can set the
  playlist public after it's created, but at the moment that setting will be
  lost when `graplsub` deletes and re-creates the playlist. It wouldn't be
  hard to add a "public playlist" option, but no one in my house wants to
  listen to my library by random albums except me! 😀
- Maybe there would be some use in limiting the playlist length by track count
  or total playtime instead of just album count.
- It could be good to have an option to exclude "various artists" albums.
- I can see how one might want to only take albums from a particular library
  or set of libraries. Personally my default user account only sees my own
  library and I have to log in as a different user to see the shared libraries
  of others. In a more permissive setup though you'd get random albums from
  every library. Not all Subsonic servers even support multiple libraries
  (Navidrome does).
- Navidrome's [Smart Playlists] feature has some good ideas. It's a pity it
  couldn't solve my random album needs, but I'm told they're working on it.
  Anyway, it could be interesting to add some other playlist-building rules in
  addition to "random albums", like say "bias towards albums that haven't been
  listened to recently", or "choose albums from this set of genres". The full
  set of album and song metadata is available through the API so that's
  interesting.
- Running `graplsub` repeatedly is a waste of resources if you aren't actually
  going to look at the playlist again, but there is no way to change the order
  or pick up new music without doing so. It looks like there may be a way to
  extract from Navidrome some sort of "last update" time for the entire
  library so it could be possible to have `graplsub` optionally run in a mode
  that doesn't make a new playlist unless the library has changed.
- I could probably provide some binaries if anyone actually cares.

[Smart Playlists]: https://www.navidrome.org/docs/usage/smartplaylists/

## Security considerations

Subsonic security posture is fairly weak and I just have to work with that.
All clients are using the same API as `graplsub` uses, so these same issues
face them as well.

The API is all `GET` requests each of which carry the credentials as query
parameters, so there's concerns about exposure in transit and in log files
etc.

The password is salted and MD5 hashed but you probably don't need telling how
weak MD5 is, and there is no protection against replaying credentials, i.e.,
if an attacker sees the MD5 token and the salt that are in each `GET` request
they can just use them again at any time.

Hopefully then it's clear that you would never want to reuse credentials here.
