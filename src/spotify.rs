use base64::{engine::general_purpose, Engine};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i32,
    refresh_token: String,
    scope: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RefreshTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i32,
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
    token_data: TokenResponse,
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
            token_data: TokenResponse::default(),
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

    pub async fn init_token(&mut self) -> Result<String, anyhow::Error> {
        // check for token on disk
        self.token_from_disk().await?;
        println!("accress token on obj is: {}", self.token_data.access_token);
        // println!("refresh token on obj is: {}", self.token_data.refresh_token);
        let token_from_refresh = self.refresh_token().await?;
        Ok(token_from_refresh)
    }

    pub async fn token_from_disk(&mut self) -> Result<String, anyhow::Error> {
        println!("token from disk");
        let mut buf = String::new();
        match tokio::fs::File::open("token").await {
            Ok(mut f) => {
                println!("reading file");
                f.read_to_string(&mut buf).await?;
                println!("buf: ");
                println!("{buf}");
                self.token_data = serde_json::from_str(buf.as_str())?;
                Ok(buf)
            }
            Err(_) => {
                println!("error reading file creating 'token'");
                tokio::fs::File::create("token").await?;
                anyhow::Result::Err(anyhow::anyhow!("no token saved"))
            }
        }
    }

    pub async fn new_token(&mut self, auth_code: &str) -> Result<String, anyhow::Error> {
        let url = String::from("https://accounts.spotify.com/api/token");
        let redirect_uri = self.redirect_uri.clone();
        let client = Client::new();

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
            "grant_type=authorization_code&code={auth_code}&redirect_uri={redirect_uri}"
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
                write_token_data_to_disk(&data).await?;
                return Ok(data.access_token);
            }
            Err(e) => {
                println!("json parsing error: {:?}", e);
                return anyhow::Result::Err(anyhow::anyhow!("json parsing error: {:?}", e));
            }
        }
    }

    pub async fn refresh_token(&mut self) -> Result<String, anyhow::Error> {
        println!("refresh_token");
        let url = String::from("https://accounts.spotify.com/api/token");
        let client = Client::new();

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Content-Type",
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        headers.insert(
            "Authorization",
            format!(
                "Basic {}",
                general_purpose::STANDARD.encode(format!("{}:{}", CLIENT_ID, CLIENT_SECRET))
            )
            .parse()
            .unwrap(),
        );
        let refresh_token = &self.token_data.refresh_token;
        let client_id = &self.client_id;
        let body = reqwest::Body::from(format!(
            "grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}"
        ));
        let spotify_server_res = client.post(url).headers(headers).body(body).send().await;

        let j: Result<RefreshTokenResponse, reqwest::Error> = match spotify_server_res {
            Ok(res) => {
                // println!("refresh_token response: {:?}", res);
                // let t = &res.text().await;
                // println!("text response: {:?}", t);
                let r = res.json::<RefreshTokenResponse>().await;
                r
                // Ok(TokenResponse::default())
            }
            Err(e) => {
                println!("token refresh network error: {:?}", e);
                return anyhow::Result::Err(anyhow::anyhow!(
                    "token refresh network error: {:?}",
                    e
                ));
            }
        };

        match j {
            Ok(data) => {
                println!("got refresh token");
                self.token_data.access_token = data.clone().access_token;
                write_token_data_to_disk(&self.token_data).await?;
                return Ok(self.token_data.access_token.clone());
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

        // let raw_auth_str: Vec<u8> = format!("{}:{}", CLIENT_ID, CLIENT_SECRET).into_bytes();
        // let encoded_auth_str = general_purpose::STANDARD.encode(&raw_auth_str);
        let mut headers = reqwest::header::HeaderMap::new();
        // headers.insert("Content-Type",
        //     "application/x-www-form-urlencoded".parse().unwrap(),);
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.token_data.access_token.clone())
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

    //         let j: Result<CurrentlyPlayingResponse, anyhow::Error> = match currently_playing_res {
    //             Ok(res) => {
    //              res.json().await
    //             },
    //             Err(e) => {
    //                 println!("Server Error: {:?}", e);
    //                 return anyhow::Result::Err(anyhow::anyhow!("Server Error: {:?}", e));
    //             }
    //         };
    //             match j {
    //                 Ok(data) {
    //                     return j;
    //                 },
    //                 Err(e) => {
    //                     return anyhow::Result::Err(anyhow::anyhow!("json parsing error: {:?}", e));
    //                 }
    //             }
}

async fn write_token_data_to_disk(token: &TokenResponse) -> anyhow::Result<()> {
    println!("creating file");
    let f = tokio::fs::File::create("token").await.unwrap();
    println!("json'ing token_data");
    let data = serde_json::to_vec(&token).unwrap();
    let mut writer = BufWriter::new(f);
    println!("writing");
    writer.write_all(&data).await?;
    println!("flushing");
    writer.flush().await?;
    Ok(())
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

impl CurrentlyPlayingResponse {
    pub fn to_string(&self) -> String {
        format!("{}", self.is_playing)
    }
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
