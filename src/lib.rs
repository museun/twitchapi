use ::serde::de::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use time::{OffsetDateTime, PrimitiveDateTime};

/// A clonable Twitch API client
#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
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

impl Client {
    const BASE_URI: &'static str = "https://api.twitch.tv/helix";

    /// Create a new Twitch API client with the provided Client-ID
    pub fn new(client_id: &str) -> Self {
        let client = reqwest::ClientBuilder::new()
            .user_agent("twitchapi/ccd6048 (github.com/museun/twitchapi)")
            .default_headers({
                let mut map = reqwest::header::HeaderMap::new();
                map.insert("Client-ID", client_id.parse().unwrap());
                map
            })
            .build()
            .unwrap();
        Self { client }
    }

    /// Get a collection of streams for the provided user logins
    pub async fn get_streams<I>(&self, user_logins: I) -> anyhow::Result<Vec<Stream>>
    where
        I: IntoIterator,
        I::Item: serde::Serialize,
    {
        #[derive(Deserialize)]
        struct Data {
            data: Vec<Stream>,
        }

        self.get_response::<Data, _, _>("streams", std::iter::repeat("user_login").zip(user_logins))
            .await
            .map(|data| data.data)
    }

    /// Get a collection of streams for the provided user ids
    pub async fn get_streams_from_id<I>(&self, user_ids: I) -> anyhow::Result<Vec<Stream>>
    where
        I: IntoIterator,
        I::Item: serde::Serialize,
    {
        #[derive(Deserialize)]
        struct Data {
            data: Vec<Stream>,
        }

        self.get_response::<Data, _, _>("streams", std::iter::repeat("user_id").zip(user_ids))
            .await
            .map(|data| data.data)
    }

    /// Get a collection of users for the provided user names
    pub async fn get_users<I>(&self, user_logins: I) -> anyhow::Result<Vec<User>>
    where
        I: IntoIterator,
        I::Item: serde::Serialize,
    {
        #[derive(Deserialize)]
        struct Data {
            data: Vec<User>,
        }

        self.get_response::<Data, _, _>("users", std::iter::repeat("login").zip(user_logins))
            .await
            .map(|data| data.data)
    }

    /// Get a collection of users for the provided user ids
    pub async fn get_users_from_id<I>(&self, user_ids: I) -> anyhow::Result<Vec<User>>
    where
        I: IntoIterator,
        I::Item: serde::Serialize,
    {
        #[derive(Deserialize)]
        struct Data {
            data: Vec<User>,
        }

        self.get_response::<Data, _, _>("users", std::iter::repeat("id").zip(user_ids))
            .await
            .map(|data| data.data)
    }

    /// Get a collection of users for a Twitch channel
    pub async fn get_users_for(&self, room: &str) -> anyhow::Result<Users> {
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

    async fn get_response<'a, T, M, V>(&self, ep: &str, map: M) -> anyhow::Result<T>
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

/// Deserialize a `time::PrimitiveDateTime`
fn prim_date_time<'de, D>(deser: D) -> Result<PrimitiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    // TODO borrow this
    let s = String::deserialize(deser)?;
    time::parse(&s, "%FT%TZ").map_err(Error::custom)
}

/// Deserialize a `time::OffsetDateTime` with an assumed ***UTC*** offset
fn assume_utc_date_time<'de, D>(deser: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    // TODO borrow this
    let s = String::deserialize(deser)? + " +0000";
    time::parse(&s, "%FT%TZ %z").map_err(Error::custom)
}

/// Deserialize using a `FromStr` impl
fn from_str<'de, D, T>(deser: D) -> Result<T, D::Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
    D: Deserializer<'de>,
{
    // TODO borrow this
    let s = String::deserialize(deser)?;
    s.parse().map_err(Error::custom)
}
