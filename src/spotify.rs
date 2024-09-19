use base64::{engine::general_purpose, Engine};
use reqwest::{Client, Response};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i32,
    refresh_token: String,
    scope: String,
}

#[derive(Debug, Default)]
enum ResponseType {
    #[default]
    Code,
    Token,
}

#[derive(Debug, Default)]
pub struct Spotify {
    client_id: String, // 	Required	The Client ID generated after registering your application.
    response_type: ResponseType, //Required	Set to code.
    redirect_uri: String, // Required	The URI to redirect to after the user grants or denies permission.
    // This URI needs to have been entered in the Redirect URI allowlist that you specified when you registered your application (See the app guide).
    // The value of redirect_uri here must exactly match one of the values you entered when you registered your application, including upper or lowercase, terminating slashes, and such.
    state: Option<String>, // Optional, but strongly recommended	This provides protection against attacks such as cross-site request forgery. See RFC-6749.
    scope: Option<String>, //	Optional	A space-separated list of scopes.If no scopes are specified, authorization will be granted only to access publicly available information:
    //	that is, only information normally visible in the Spotify desktop, web, and mobile players.
    pub show_dialog: bool, // Optional	Whether or not to force the user to approve the app again if they’ve already done so. If false (default), a user who has already approved the application may be automatically redirected to the URI specified by redirect_uri. If true, the user will not be automatically redirected and will have to approve the app again.

    token: Option<String>,
}

impl Spotify {
    fn new() -> Self {
        Spotify {
            client_id: String::from(""),
            response_type: ResponseType::Code,
            redirect_uri: String::from(""),
            state: None,
            scope: None,
            show_dialog: false,
            token: None,
        }
    }

    pub fn from_client_id(client_id: &str) -> Self {
        Spotify {
            client_id: String::from(client_id),
            ..Default::default()
        }
    }

    pub fn with_state(mut self, state: &str) -> Self {
        self.state = Some(String::from(state));
        self
    }

    pub fn with_scope(mut self, scope: &str) -> Self {
        self.scope = Some(String::from(scope));
        self
    }

    pub fn with_redirect_uri(mut self, redirect_uri: &str) -> Self {
        self.redirect_uri = String::from(redirect_uri);
        self
    }

    pub fn auth_url(&self) -> String {
        let base = "https://accounts.spotify.com/authorize".to_owned();
        let params = format!(
            "?client_id={}&response_type=code&redirect_uri={}&state={}&scope={}&show_dialog={}",
            urlencoding::encode(self.client_id.as_str()),
            urlencoding::encode(self.redirect_uri.as_str()),
            urlencoding::encode(self.state.clone().unwrap_or_default().as_str()),
            urlencoding::encode(self.scope.clone().unwrap_or_default().as_str()),
            urlencoding::encode(self.show_dialog.to_string().as_str())
        );
        base + params.as_str()
    }

    async fn token_from_disk(&mut self) -> Result<String, anyhow::Error> {
        let mut buf = String::new();
        match tokio::fs::File::open("token").await {
            Ok(mut f) => {
                f.read_to_string(&mut buf).await.unwrap();
                self.token = Some(buf.clone());
                Ok(buf)
            }
            Err(_) => {
                tokio::fs::File::create("token").await.unwrap();
                anyhow::Result::Err(anyhow::anyhow!("no token saved"))
            }
        }
    }

    pub async fn token(&mut self) -> Result<String, anyhow::Error> {
        let disk_token = self.token_from_disk().await;
        if disk_token.is_ok() && disk_token.as_ref().unwrap().len() > 0 {
            self.token = Some(disk_token.as_ref().unwrap().clone());
            return Ok(disk_token.unwrap());
        }
        let url = String::from("https://accounts.spotify.com/api/token");
        let redirect_uri = self.redirect_uri.clone();
        let client = Client::new();

        // encode client_id and client_secret

        let raw_auth_str: Vec<u8> = format!("{}:{}", CLIENT_ID, CLIENT_SECRET).into_bytes();
        let encoded_auth_str = general_purpose::STANDARD.encode(&raw_auth_str);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Content-Type",
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        headers.insert(
            "Authorization",
            format!("Basic {}", encoded_auth_str).parse().unwrap(),
        );
        let body = reqwest::Body::from(format!(
            "grant_type=authorization_code&code={AUTH_CODE}&redirect_uri={redirect_uri}"
        ));

        let spotify_server_res = client.post(url).headers(headers).body(body).send().await;

        let j: Result<TokenResponse, reqwest::Error> = match spotify_server_res {
            Ok(res) => res.json().await,
            Err(e) => {
                println!("Server Error: {:?}", e);
                return anyhow::Result::Err(anyhow::anyhow!("Server Error: {:?}", e));
            }
        };

        match j {
            Ok(data) => {
                println!("got token for: {:?}", data.scope);
                self.token = Some(data.access_token.clone());
                write_token_to_disk(data.access_token.clone()).await;
                return Ok(data.access_token);
            }
            Err(e) => {
                println!("json parsing error: {:?}", e);
                return anyhow::Result::Err(anyhow::anyhow!("json parsing error: {:?}", e));
            }
        }
    }

    pub async fn get_currently_playing(&self) -> Result<CurrentlyPlayingResponse, anyhow::Error> {
        let url = "https://api.spotify.com/v1/me/player/currently-playing";
        let client = Client::new();

        let raw_auth_str: Vec<u8> = format!("{}:{}", CLIENT_ID, CLIENT_SECRET).into_bytes();
        let encoded_auth_str = general_purpose::STANDARD.encode(&raw_auth_str);
        let mut headers = reqwest::header::HeaderMap::new();
        // headers.insert("Content-Type",
        //     "application/x-www-form-urlencoded".parse().unwrap(),);
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.token.clone().unwrap())
                .parse()
                .unwrap(),
        );

        let currently_playing_res = client
            .get(url)
            .headers(headers)
            .send()
            .await?
            .json::<CurrentlyPlayingResponse>()
            .await?;

        Ok(currently_playing_res)
    }
async fn write_token_to_disk(token: String) {
    let mut f = tokio::fs::File::create("token").await.unwrap();
    f.write_all(token.as_bytes()).await.unwrap();
}

#[derive(Deserialize)]
enum CurrentlyPlayingType {
    #[serde(rename = "track")]
    Track,
    #[serde(rename = "episode")]
    Episode,
    #[serde(rename = "ad")]
    Ad,
    #[serde(rename = "unknown")]
    Unknown,
}
// #[derive(Deserialize)]
// pub enum PlayableItem {
//     TrackObject(TrackObject),
//     EpisodeObject(EpisodeObject),
// }
#[derive(Deserialize)]
pub struct TrackObject {
    album: AlbumObject,
    pub artists: Vec<SimplifiedArtistObject>,
    duration_ms: i32,
    id: String,
    pub name: String,
    popularity: i32,
    is_local: bool,
}
#[derive(Deserialize)]
pub struct EpisodeObject {}

#[derive(Deserialize)]
pub struct CurrentlyPlayingResponse {
    timestamp: u64,
    progress_ms: i32,
    is_playing: bool,
    // could ALSO be an EpisodeObject maybe?
    pub item: Option<Item>,
    currently_playing_type: CurrentlyPlayingType,
}

#[derive(Deserialize)]
pub struct Item {
    pub album: AlbumObject,
}

#[derive(Deserialize)]
pub struct AlbumObject {
    id: String,
    name: String,
    release_date: String,
    release_date_precision: String,
    pub artists: Vec<SimplifiedArtistObject>,
}
#[derive(Deserialize)]
pub struct SimplifiedArtistObject {
    id: String,
    pub name: String,
    href: String,
}
