use serde::{Deserialize, Serialize};
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Debug)]
pub enum Error {
    Reqwest {
        error: reqwest::Error,
    },
    InvalidClientId {
        error: reqwest::header::InvalidHeaderValue,
    },
    InvalidOAuthToken {
        error: reqwest::header::InvalidHeaderValue,
    },
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Reqwest { error }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Reqwest { error } => write!(f, "reqwest error: {}", error),
            Error::InvalidClientId { error } => write!(f, "invalid client id: {}", error),
            Error::InvalidOAuthToken { error } => write!(f, "invalid oauth token: {}", error),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Reqwest { error } => Some(error),
            Error::InvalidClientId { error } => Some(error),
            Error::InvalidOAuthToken { error } => Some(error),
        }
    }
}

/// Users in a channel
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Users {
    #[serde(skip_deserializing)]
    /// Name of the room
    pub room: String,
    #[serde(skip_deserializing)]
    /// Total number of people in the room
    pub chatter_count: usize,
    /// List of broadcasters in the room
    pub broadcaster: Vec<String>,
    /// List of vips in the room
    pub vips: Vec<String>,
    /// List of moderators in the room
    pub moderators: Vec<String>,
    /// List of staff in the room
    pub staff: Vec<String>,
    /// List of admins in the room
    pub admins: Vec<String>,
    /// List of global_mods in the room
    pub global_mods: Vec<String>,
    /// List of viewers in the room
    pub viewers: Vec<String>,
}

/// A Twitch stream
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Stream {
    #[serde(deserialize_with = "from_str")]
    /// The stream id
    pub id: u64,
    #[serde(deserialize_with = "from_str")]
    /// User id of the broadcaster
    pub user_id: u64,
    /// User name of the broadcaster
    pub user_name: String,
    #[serde(deserialize_with = "from_str")]
    /// Id of the game being broadcasted
    pub game_id: u64,
    #[serde(rename = "type")]
    /// The type of stream (`Some("live")` or None for offline current)
    pub type_: Option<String>, // TODO enum
    /// The title of the stream
    pub title: String,
    /// The viewer count for the stream
    pub viewer_count: u64,
    #[serde(deserialize_with = "assume_utc_date_time")]
    /// When the stream started, from an UTC offset
    pub started_at: time::OffsetDateTime,
}

/// A Twitch user
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    #[serde(deserialize_with = "from_str")]
    /// Their user id
    pub id: u64,
    /// Their login name
    pub login: String,
    /// Their display name
    pub display_name: String,
}

/// Required authentication keys
#[derive(Clone, Debug, PartialEq)]
pub struct Authenication {
    /// Your twitch Client-ID
    pub client_id: String,
    /// A OAuth token that is associated with the Client-ID
    pub oauth_token: String,
}

/// A clonable Twitch API client
#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
}

impl Client {
    const BASE_URI: &'static str = "https://api.twitch.tv/helix";

    /// Create a new Twitch API client with the provided `Authenication`
    pub fn new(auth: impl std::borrow::Borrow<Authenication>) -> Result<Self, Error> {
        reqwest::ClientBuilder::new()
            // TODO get this at build-time
            .user_agent("twitchapi/ccd6048 (github.com/museun/twitchapi)")
            .default_headers({
                let auth = auth.borrow();
                let mut map = reqwest::header::HeaderMap::new();
                map.insert(
                    "Client-ID",
                    auth.client_id
                        .parse()
                        .map_err(|error| Error::InvalidClientId { error })?,
                );
                map.insert(
                    "Authorization",
                    format!("Bearer {}", auth.oauth_token)
                        .parse()
                        .map_err(|error| Error::InvalidOAuthToken { error })?,
                );
                map
            })
            .build()
            .map_err(Into::into)
            .map(|client| Self { client })
    }

    /// Get a collection of streams for the provided user logins
    pub async fn get_streams<I>(&self, user_logins: I) -> Result<Vec<Stream>, Error>
    where
        I: IntoIterator,
        I::Item: serde::Serialize,
    {
        #[derive(Deserialize)]
        struct Data {
            data: Vec<Stream>,
        }

        self.get_response("streams", std::iter::repeat("user_login").zip(user_logins))
            .await
            .map(|data: Data| data.data)
    }

    /// Get a collection of streams for the provided user ids
    pub async fn get_streams_from_id<I>(&self, user_ids: I) -> Result<Vec<Stream>, Error>
    where
        I: IntoIterator,
        I::Item: serde::Serialize,
    {
        #[derive(Deserialize)]
        struct Data {
            data: Vec<Stream>,
        }

        self.get_response("streams", std::iter::repeat("user_id").zip(user_ids))
            .await
            .map(|data: Data| data.data)
    }

    /// Get a collection of users for the provided user names
    pub async fn get_users<I>(&self, user_logins: I) -> Result<Vec<User>, Error>
    where
        I: IntoIterator,
        I::Item: serde::Serialize,
    {
        #[derive(Deserialize)]
        struct Data {
            data: Vec<User>,
        }

        self.get_response("users", std::iter::repeat("login").zip(user_logins))
            .await
            .map(|data: Data| data.data)
    }

    /// Get a collection of users for the provided user ids
    pub async fn get_users_from_id<I>(&self, user_ids: I) -> Result<Vec<User>, Error>
    where
        I: IntoIterator,
        I::Item: serde::Serialize,
    {
        #[derive(Deserialize)]
        struct Data {
            data: Vec<User>,
        }

        self.get_response("users", std::iter::repeat("id").zip(user_ids))
            .await
            .map(|data: Data| data.data)
    }

    /// Get a collection of users for a Twitch channel
    pub async fn get_users_for(&self, room: &str) -> Result<Users, Error> {
        #[derive(Deserialize)]
        struct Data {
            chatter_count: usize,
            chatters: Users,
        }

        let req = self
            .client
            .get(&format!(
                "https://tmi.twitch.tv/group/user/{}/chatters",
                room
            ))
            .build()?;

        self.client
            .execute(req)
            .await?
            .error_for_status()?
            .json()
            .await
            .map(|data: Data| Users {
                room: room.to_string(),
                chatter_count: data.chatter_count,
                ..data.chatters
            })
            .map_err(Into::into)
    }

    async fn get_response<'a, T, M, V>(&self, ep: &str, map: M) -> Result<T, Error>
    where
        for<'de> T: serde::Deserialize<'de>,
        M: IntoIterator<Item = (&'a str, V)>,
        V: serde::Serialize,
    {
        let mut req = self.client.get(&format!("{}/{}", Self::BASE_URI, ep));
        for (key, val) in map {
            req = req.query(&[(key, val)]);
        }
        self.client
            .execute(req.build()?)
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }
}

/// Deserialize a `time::OffsetDateTime` with an assumed ***UTC*** offset
fn assume_utc_date_time<'de, D>(deser: D) -> Result<OffsetDateTime, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    time::parse(&(String::deserialize(deser)? + " +0000"), "%FT%TZ %z")
        .map_err(serde::de::Error::custom)
}

/// Deserialize using a `FromStr` impl
fn from_str<'de, D, T>(deser: D) -> Result<T, D::Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
    D: serde::de::Deserializer<'de>,
{
    String::deserialize(deser)?
        .parse()
        .map_err(serde::de::Error::custom)
}
