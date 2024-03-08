use std::fmt::Debug;

use blake3::Hasher;
use sqlite::{Statement, Bindable, BindableWithIndex, ReadableWithIndex};

use crate::sqlite_util::bind_option_vec_u8;


pub trait FromStatement {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error>
    where Self: Sized;
}

macro_rules! newtype_sqlite {
    { $t:tt, $type:ty } => {
        impl ReadableWithIndex for $t {
            fn read<T: sqlite::ColumnIndex>(
                st: &Statement, i: T
            ) -> sqlite::Result<Self> {
                Ok($t(<$type>::read(st, i)?))
            }
        }

        impl BindableWithIndex for $t {
            fn bind<T: sqlite::ParameterIndex>(
                self, st: &mut Statement, i: T
            ) -> sqlite::Result<()> {
                self.0.bind(st, i)
            }
        }

        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                self.0.eq(&other.0)
            }
        }

        impl Eq for $t {}

        impl Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_fmt(format_args!("{}({:?})", stringify!($t), self.0))
            }
        }

        impl Clone for $t {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }
    }
}

macro_rules! newtype_sqlite_copy {
    { $t:tt, $type:ty } => {
        newtype_sqlite!{ $t, $type }
        impl Copy for $t {}
    }
}

// Can't use KString or would need to impl From<KString> for `Cow<'_,
// _>`, and also associated `read` function for KString.
pub struct UserOrGroupName(String);
newtype_sqlite!(UserOrGroupName, String);

// (Thus also can't use KString conversion traits.)

#[derive(Debug, thiserror::Error)]
pub enum UserOrGroupNameError {
    #[error("invalid character {1:?} in user or group name at index {0}")]
    InvalidCharacter(usize, char)
}

impl UserOrGroupName {
    pub fn new(s: String) -> Result<Self, UserOrGroupNameError> {
        for (i, c) in s.chars().enumerate() {
            if ! c.is_ascii_alphanumeric() {
                return Err(UserOrGroupNameError::InvalidCharacter(i, c))
            }
        }
        Ok(Self(s))
    }
    pub fn to_string(self) -> String {
        self.0
    }
}
impl TryFrom<String> for UserOrGroupName {
    type Error = UserOrGroupNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl TryFrom<&str> for UserOrGroupName {
    type Error = UserOrGroupNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.into())
    }
}

pub struct UserId(pub i64);
newtype_sqlite_copy!(UserId, i64);

#[derive(Debug)]
pub struct User {
    pub id: Option<UserId>,
    pub username: UserOrGroupName,
    pub email: Option<String>,
    pub name: String,
    pub surname: String,
    pub hashed_pass: String,
}

impl FromStatement for User {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error> {
        Ok((User {
            id: Some(sth.read(0)?),
            username: sth.read(1)?,
            email: sth.read(2)?,
            name: sth.read(3)?,
            surname: sth.read(4)?,
            hashed_pass: sth.read(5)?,
        }, sth))
    }
}
impl Bindable for &User {
    fn bind(self, st: &mut Statement) -> sqlite::Result<()> {
        // HACK: assume we want to bind it from index 0, and id should
        // come last. Make our own trait?
        let offset = |n: usize| -> usize { n + 1 };
        let User { id, username, email, name, surname, hashed_pass } = self;
        username.clone().bind(st, offset(0))?;
        email.as_ref().map(|v| v.as_str()).bind(st, offset(1))?;
        name.bind(st, offset(2))?;
        surname.bind(st, offset(3))?;
        hashed_pass.bind(st, offset(4))?;
        if let Some(id) = id {
            id.bind(st, offset(5))?;
        }
        Ok(())
    }
}

// fake DB object for count(*) queries
#[derive(Debug)]
pub struct Count(pub i64);

impl FromStatement for Count {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error>
    {
        Ok((Count(sth.read(0)?), sth))
    }
}


pub struct GroupId(pub i64);
newtype_sqlite_copy!(GroupId, i64);

#[derive(Debug)]
pub struct Group {
    pub id: Option<GroupId>,
    pub groupname: UserOrGroupName,
}

impl FromStatement for Group {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error>
    where Self: Sized {
        Ok((Group {
            id: Some(sth.read(0)?),
            groupname: sth.read(1)?,
        }, sth))
    }
}


#[derive(Debug)]
pub struct UserInGroup {
    pub user_id: UserId,
    pub group_id: GroupId
}

impl FromStatement for UserInGroup {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error> {
        Ok((UserInGroup {
            user_id: sth.read(0)?,
            group_id: sth.read(1)?,
        }, sth))
    }
}


#[derive(Debug)]
pub struct FailedLoginAttempt {
    pub id: Option<i64>,
    pub ip: Vec<u8>,
    pub username: String,
    pub unixtime_next_allowed: i64,
    pub seconds_next_wait: i64,
}

impl FromStatement for FailedLoginAttempt {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error> {
        Ok((Self {
            id: Some(sth.read(0)?),
            ip: sth.read(1)?,
            username: sth.read(2)?,
            unixtime_next_allowed: sth.read(3)?,
            seconds_next_wait: sth.read(4)?,
        }, sth))
    }
}
impl Bindable for &FailedLoginAttempt {
    fn bind(self, st: &mut Statement) -> sqlite::Result<()> {
        // HACK: assume we want to bind it from index 0, and id should
        // come last. Make our own trait?
        let offset = |n: usize| -> usize { n + 1 };
        self.ip.bind(st, offset(0))?;
        self.username.bind(st, offset(1))?;
        self.unixtime_next_allowed.bind(st, offset(2))?;
        self.seconds_next_wait.bind(st, offset(3))?;
        if let Some(id) = self.id {
            id.bind(st, offset(4))?;
        }
        Ok(())
    }
}

/// The sessionid is hashed to avoid a timing side channel on database
/// lookups of the user-provided sessionids.
#[derive(Debug)]
pub struct SessionData {
    pub id: Option<i64>,
    sessionid_hash: Vec<u8>,
    pub last_request_time: i64, // unixtime
    pub user_id: Option<UserId>, // in the future, session data can exist even if not logged in
    pub ip: Option<Vec<u8>>, // the IP that logged in
}

impl SessionData {
    /// For `hasher`, pass the clone of a `blake3::Hasher` that was
    /// already `update`d with a secret part.
    pub fn new(
        id: Option<i64>,
        sessionid: &str,
        last_request_time: i64,
        user_id: Option<UserId>,
        ip: Option<Vec<u8>>,
        hasher: Hasher
    ) -> Self {
        let mut hasher = hasher;
        hasher.update(sessionid.as_bytes());
        let h = hasher.finalize();
        SessionData {
            id,
            sessionid_hash: h.as_bytes().to_vec(),
            last_request_time,
            user_id,
            ip
        }
    }
}

impl FromStatement for SessionData {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error> {
        Ok((Self {
            id: Some(sth.read(0)?),
            sessionid_hash: sth.read(1)?,
            last_request_time: sth.read(2)?,
            user_id: sth.read(3)?,
            ip: sth.read(4)?,
        }, sth))
    }
}
impl Bindable for &SessionData {
    fn bind(self, st: &mut Statement) -> sqlite::Result<()> {
        // HACK: assume we want to bind it from index 0, and id should
        // come last. Make our own trait?
        let offset = |n: usize| -> usize { n + 1 };
        self.sessionid_hash.bind(st, offset(0))?;
        self.last_request_time.bind(st, offset(1))?;
        self.user_id.bind(st, offset(2))?;
        bind_option_vec_u8(&self.ip, st, offset(3))?;
        if let Some(id) = self.id {
            id.bind(st, offset(4))?;
        }
        Ok(())
    }
}

