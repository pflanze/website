use std::fmt::Debug;

use sqlite::{Statement, Bindable, BindableWithIndex, ReadableWithIndex};

use crate::sqlite_util::bind_option_vec_u8;


pub trait FromStatement {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error>
    where Self: Sized;
}

macro_rules! newtype_impl_sqlite {
    { $t:tt } => {
        impl ReadableWithIndex for $t {
            fn read<T: sqlite::ColumnIndex>(
                st: &Statement, i: T
            ) -> sqlite::Result<Self> {
                Ok($t(i64::read(st, i)?))
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
                f.write_fmt(format_args!("{}({})", stringify!($t), self.0))
            }
        }

        impl Clone for $t {
            fn clone(&self) -> Self {
                Self(self.0)
            }
        }
        impl Copy for $t {}
    }
}

pub struct UserId(pub i64);
newtype_impl_sqlite!(UserId);

#[derive(Debug)]
pub struct User {
    pub id: Option<UserId>,
    pub username: String,
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
        self.username.bind(st, offset(0))?;
        self.email.as_ref().map(|v| v.as_str()).bind(st, offset(1))?;
        self.name.bind(st, offset(2))?;
        self.surname.bind(st, offset(3))?;
        self.hashed_pass.bind(st, offset(4))?;
        if let Some(id) = self.id {
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
newtype_impl_sqlite!(GroupId);

#[derive(Debug)]
pub struct Group {
    pub id: Option<GroupId>,
    pub groupname: String,
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

#[derive(Debug)]
pub struct SessionData {
    pub id: Option<i64>,
    pub sessionid: String,
    pub last_request_time: i64, // unixtime
    pub user_id: Option<UserId>, // in the future, session data can exist even if not logged in
    pub ip: Option<Vec<u8>>, // the IP that logged in
}

impl FromStatement for SessionData {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error> {
        Ok((Self {
            id: Some(sth.read(0)?),
            sessionid: sth.read(1)?,
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
        self.sessionid.bind(st, offset(0))?;
        self.last_request_time.bind(st, offset(1))?;
        self.user_id.bind(st, offset(2))?;
        bind_option_vec_u8(&self.ip, st, offset(3))?;
        if let Some(id) = self.id {
            id.bind(st, offset(4))?;
        }
        Ok(())
    }
}

