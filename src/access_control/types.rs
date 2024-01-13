use sqlite::{Statement, Bindable, BindableWithIndex};

use crate::sqlite_util::bind_option_vec_u8;


pub trait FromStatement {
    fn from_statement<'s, 'slf>(
        sth: &'s mut Statement<'slf>
    ) -> Result<(Self, &'s mut Statement<'slf>), sqlite::Error>
    where Self: Sized;
}

#[derive(Debug)]
pub struct User {
    pub id: Option<i64>,
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


#[derive(Debug)]
pub struct UserInGroup {
    pub user_id: i64,
    pub group_id: i64
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
pub struct Group {
    pub id: Option<i64>,
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
    pub user_id: Option<i64>, // in the future, session data can exist even if not logged in
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

