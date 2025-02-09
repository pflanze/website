use std::{path::PathBuf, pin::Pin, sync::{atomic::AtomicBool, Mutex, Arc}};

use anyhow::{bail, Result};
use blake3::{Hasher, Hash};
use sqlite::{Statement, Connection, State, Bindable, BindableWithIndex};

use chj_util::{warn_thread, notime};

use crate::{defn_with_statement, get_statement, try_sqlite};
use super::{transaction::Transaction,
            types::{User, Group, Count, SessionData, UserId, GroupId, UserOrGroupName},
            util::{get_unique_by, UniqueError, RequiredUniqueError, required_unique},
            sqliteposerror::SQLitePosError};

pub static DO_WARN_THREAD: AtomicBool = AtomicBool::new(false);

// ------------------------------------------------------------------
pub struct Statements {
    st_select_user_by_id: Option<Result<Statement<'static>, sqlite::Error>>,
    st_select_user_by_username: Option<Result<Statement<'static>, sqlite::Error>>,
    st_select_group_by_groupname: Option<Result<Statement<'static>, sqlite::Error>>,
    st_select_userid_from_username_groupname: Option<Result<Statement<'static>, sqlite::Error>>,
    st_select_groupid_from_userid_groupname: Option<Result<Statement<'static>, sqlite::Error>>,
    st_insert_into_user: Option<Result<Statement<'static>, sqlite::Error>>,
    st_insert_into_group: Option<Result<Statement<'static>, sqlite::Error>>,
    st_insert_userid_groupid: Option<Result<Statement<'static>, sqlite::Error>>,
    st_delete_userid_groupid: Option<Result<Statement<'static>, sqlite::Error>>,
    st_select_count_from_useringroup: Option<Result<Statement<'static>, sqlite::Error>>,
    st_update_user: Option<Result<Statement<'static>, sqlite::Error>>,
    st_select_sessiondata_by_sessionid: Option<Result<Statement<'static>, sqlite::Error>>,
    st_update_sessiondata: Option<Result<Statement<'static>, sqlite::Error>>,
    st_insert_into_sessiondata: Option<Result<Statement<'static>, sqlite::Error>>,
    // WARNING: don't forget to add new fields to Drop for Db !
}

pub struct Db {
    path: Arc<PathBuf>,
    pub(crate) write_transaction_mutex: Mutex<()>,
}

impl Db {
    pub fn new(path: &str) -> Self {
        Db {
            path: Arc::new(path.into()),
            write_transaction_mutex: Default::default()
        }
    }
}

pub struct ConnectionAndStatements {
    path: Arc<PathBuf>, // clone from Db
    pub(crate) connection: Option<Pin<Box<Connection>>>,
    statements: Statements
}

pub struct DbConnection {
    pub(crate) db: Arc<Db>,
    pub(crate) connection_and_statements: ConnectionAndStatements,
}

impl Drop for ConnectionAndStatements {
    fn drop(&mut self) {
        macro_rules! drop {
            { $field:ident } => {
                if let Some(s) = self.statements.$field.take() {
                    drop(s);
                    warn_thread!("dropped {}", stringify!($field));
                }
            }
        }
        drop!(st_select_user_by_id);
        drop!(st_select_user_by_username);
        drop!(st_select_group_by_groupname);
        drop!(st_select_userid_from_username_groupname);
        drop!(st_select_groupid_from_userid_groupname);
        drop!(st_insert_into_user);
        drop!(st_insert_into_group);
        drop!(st_insert_userid_groupid);
        drop!(st_delete_userid_groupid);
        drop!(st_select_count_from_useringroup);
        drop!(st_update_user);
        drop!(st_select_sessiondata_by_sessionid);
        drop!(st_update_sessiondata);
        drop!(st_insert_into_sessiondata);
        // drop(self.connection);
        warn_thread!("dropped Db");
    }
}

impl DbConnection {
    pub(crate) fn mynew(db: Arc<Db>) -> Self {
        let path = db.path.clone();
        DbConnection {
            db,
            connection_and_statements: ConnectionAndStatements {
                connection: None,
                statements: Statements {
                    st_select_user_by_id: None,
                    st_select_user_by_username: None,
                    st_select_group_by_groupname: None,
                    st_select_userid_from_username_groupname: None,
                    st_select_groupid_from_userid_groupname: None,
                    st_insert_into_user: None,
                    st_insert_into_group: None,
                    st_insert_userid_groupid: None,
                    st_delete_userid_groupid: None,
                    st_select_count_from_useringroup: None,
                    st_update_user: None,
                    st_select_sessiondata_by_sessionid: None,
                    st_update_sessiondata: None,
                    st_insert_into_sessiondata: None,
                },
                path
            }
        }
    }
}

impl ConnectionAndStatements {
    #[inline]
    pub fn with_connection<'s, F, R, E>(&'s mut self, f: F) -> Result<R, E>
    where
        F: FnOnce(&'s Connection, &'s mut Statements) -> Result<R, E>,
        E: From<SQLitePosError> // for connection initialization errors
    {
        {
            let oc = &mut self.connection;
            if oc.is_none() {
                let c = try_sqlite!(sqlite::open(&*self.path));
                // Configure the connection
                // ------------------------
                try_sqlite!(c.execute("PRAGMA journal_mode = WAL"));
                // https://www.sqlite.org/wal.html "Writers sync the
                // WAL on every transaction commit if PRAGMA
                // synchronous is set to FULL but omit this sync if
                // PRAGMA synchronous is set to NORMAL". XX dangerous
                // but OK for current usage.
                try_sqlite!(c.execute("PRAGMA synchronous = NORMAL"));
                try_sqlite!(c.execute("PRAGMA foreign_keys = ON"));

                // Store it
                *oc = Some(Box::pin(c));
                warn_thread!("initialized database field");
            }
        }
        let c = self.connection.as_ref().unwrap();
        let s = &mut self.statements; // ?? why can I take a mut ref to statements from a & ?
        f(c, s)
    }
}

// ------------------------------------------------------------------
defn_with_statement!(with_select_user_by_id,
                     st_select_user_by_id,
                     "select id, username, email, name, surname, hashed_pass \
                      from User where id = ?");
impl<'t> Transaction<'t> {
    pub fn get_user_by_id(
        &mut self, id: i64
    ) -> Result<Option<User>, UniqueError>
    {
        self.connection_and_statements.with_select_user_by_id(|sth| {
            get_unique_by("select_user_by_id", sth, [id].as_ref())
        })
    }
}

defn_with_statement!(with_select_user_by_username,
                     st_select_user_by_username,
                     "select id, username, email, name, surname, hashed_pass \
                      from User where username = ?");
impl<'t> Transaction<'t> {
    pub fn get_user_by_username(
        &mut self, username: &str
    ) -> Result<Option<User>, UniqueError>
    {
        self.connection_and_statements.with_select_user_by_username(|sth| {
            get_unique_by("select_user_by_username", sth, [username].as_ref())
        })
    }
}

defn_with_statement!(with_select_group_by_groupname,
                     st_select_group_by_groupname,
                     "select id, groupname \
                      from \"Group\" where groupname = ?");
impl<'t> Transaction<'t> {
    pub fn get_group_by_groupname(
        &mut self, groupname: &str // use UserOrGroupName ?
    ) -> Result<Option<Group>, UniqueError>
    {
        self.connection_and_statements.with_select_group_by_groupname(|sth| {
            get_unique_by("select_group_by_groupname", sth, [groupname].as_ref())
        })
    }
    pub fn xget_group_by_groupname(
        &mut self, groupname: &str // use UserOrGroupName ?
    ) -> Result<Group, RequiredUniqueError>
    {
        required_unique("Group", || format!("{groupname:?}"),
                        self.get_group_by_groupname(groupname))
    }
}

defn_with_statement!(with_select_userid_from_username_groupname,
                     st_select_userid_from_username_groupname,
                     "select User.id \
                      from User \
                      inner join UserInGroup on User.id = user_id \
                      inner join Group on group_id = Group.id \
                      where User.username = ? and Group.groupname = ?");
impl<'t> Transaction<'t> {
    pub fn username_has_groupname(
        // use UserOrGroupName for both arguments?
        &mut self, username: &str, groupname: &str
    ) -> Result<bool>
    {
        self.connection_and_statements.with_select_userid_from_username_groupname(|sth| {
            sth.reset()?;
            let arguments = [username, groupname];
            sth.bind(arguments.as_ref())?;
            match sth.next()? {
                State::Row => {
                    match sth.next()? {
                        State::Row => bail!(
                            "username_has_groupname: more than one result \
                             for arguments {arguments:?}"),
                        State::Done => Ok(true),
                    }
                 }
                State::Done => Ok(false),
            }
        })
    }
}

// XX remove this and use user_in_group instead?
defn_with_statement!(with_select_groupid_from_userid_groupname,
                     st_select_groupid_from_userid_groupname,
                     "select UserInGroup.group_id \
                      from UserInGroup \
                      inner join \"Group\" on group_id = \"Group\".id \
                      where UserInGroup.user_id = ? and \"Group\".groupname = ?");
impl<'t> Transaction<'t> {
    pub fn userid_has_groupname(
        &mut self, user_id: UserId, groupname: &str // use UserOrGroupName ?
    ) -> Result<bool, SQLitePosError>
    {
        self.connection_and_statements.with_select_groupid_from_userid_groupname(|sth| {
            try_sqlite!(sth.reset());
            try_sqlite!(user_id.bind(sth, 1));
            try_sqlite!(groupname.bind(sth, 2));
            match try_sqlite!(sth.next()) {
                State::Row => {
                    match try_sqlite!(sth.next()) {
                        State::Row => panic!( // XX better Err 
                            "userid_has_groupname: more than one result \
                             for arguments {user_id:?}, {groupname:?}"),
                        State::Done => Ok(true),
                    }
                 }
                State::Done => Ok(false),
            }
        })
    }
}

defn_with_statement!(with_insert_into_user,
                     st_insert_into_user,
                     "insert into \"User\" (username, email, name, surname, hashed_pass) \
                      values (?, ?, ?, ?, ?)");
impl<'t> Transaction<'t> {
    pub fn insert_user(
        &mut self, user: &User
    ) -> Result<(), SQLitePosError> {
        assert!(! user.id.is_some()); // relax if wanting to bypass auto-increment?
        self.connection_and_statements.with_insert_into_user(|sth| {
            try_sqlite!(sth.reset());
            try_sqlite!(user.bind(sth));
            match try_sqlite!(sth.next()) {
                State::Done => Ok(()),
                _ => panic!("what happened?") // XX return as Err
            }
        })
    }
}

defn_with_statement!(with_update_user,
                     st_update_user,
                     "update \"User\" set (username, email, name, surname, hashed_pass) \
                      = (?, ?, ?, ?, ?) \
                      where id = ?");
impl<'t> Transaction<'t> {
    pub fn update_user(
        &mut self, user: User
    ) -> Result<(), SQLitePosError> {
        let _id = user.id.expect("has id because it was read from DB, or caller provided it");
        self.connection_and_statements.with_update_user(|sth| {
            try_sqlite!(sth.reset());
            try_sqlite!(user.bind(sth));
            match try_sqlite!(sth.next()) {
                State::Done => Ok(()),
                _ => panic!("what happened?") // XX Err
            }
        })
    }
}

defn_with_statement!(with_insert_into_group,
                     st_insert_into_group,
                     "insert into \"Group\" (groupname) values (?)");
impl<'t> Transaction<'t> {
    pub fn insert_group(
        &mut self, groupname: UserOrGroupName
    ) -> Result<()> {
        self.connection_and_statements.with_insert_into_group(|sth| {
            sth.reset()?;
            let arguments = [ groupname ];
            sth.bind(arguments.as_ref())?;
            match sth.next()? {
                State::Done => Ok(()),
                _ => bail!("what happened?")
            }
        })
    }
}

defn_with_statement!(with_insert_userid_groupid,
                     st_insert_userid_groupid,
                     "insert into \"UserInGroup\" (user_id, group_id) values (?, ?)");
impl<'t> Transaction<'t> {
    /// panics unless user and group carry ids. Only those are
    /// currently being used (only using objects for type safety;
    /// should we have UserId and GroupId instead?).
    pub fn add_user_in_group(
        &mut self, user: &User, group: &Group
    ) -> Result<()> {
        let user_id = user.id.expect("user has id");
        let group_id = group.id.expect("group has id");
        self.connection_and_statements.with_insert_userid_groupid(|sth| {
            sth.reset()?;
            let arguments = [ user_id.0, group_id.0 ];
            sth.bind(arguments.as_ref())?;
            match sth.next()? {
                State::Done => Ok(()),
                _ => bail!("what happened?")
            }
        })
    }
}

defn_with_statement!(with_delete_userid_groupid,
                     st_delete_userid_groupid,
                     "delete from \"UserInGroup\" where user_id = ? and group_id = ?");
impl<'t> Transaction<'t> {
    /// panics unless user and group carry ids. Only those are
    /// currently being used (only using objects for type safety;
    /// should we have UserId and GroupId instead?).
    pub fn remove_user_in_group(
        &mut self, user: &User, group: &Group
    ) -> Result<()> {
        let user_id = user.id.expect("user has id");
        let group_id = group.id.expect("group has id");
        self.connection_and_statements.with_delete_userid_groupid(|sth| {
            sth.reset()?;
            let arguments = [ user_id.0, group_id.0 ];
            sth.bind(arguments.as_ref())?;
            match sth.next()? {
                State::Done => Ok(()),
                _ => bail!("what happened?")
            }
        })
    }
}

defn_with_statement!(with_select_count_from_useringroup,
                     st_select_count_from_useringroup,
                     "select count(*) from UserInGroup \
                      where user_id = ? and group_id = ?");
impl<'t> Transaction<'t> {
    pub fn user_in_group(&mut self, user_id: UserId, group_id: GroupId) -> Result<bool>
    {
        self.connection_and_statements.with_select_count_from_useringroup(|sth| {
            let count : Count =
                get_unique_by("select_count_from_useringroup",
                              sth,
                              [ user_id.0, group_id.0 ].as_ref())?
                .expect("always get the count");
            match count.0 {
                0 => Ok(false),
                1 => Ok(true),
                _ => bail!("buggy db, has more than 1 entry for {user_id:?}/{group_id:?}")
            }
        })
    }
}

pub fn sessionid_hash(hasher: Hasher, session_id: &str) -> Hash {
    notime!{
        "hashing";
        let mut hasher = hasher;
        hasher.update(session_id.as_bytes());
        hasher.finalize()
    }
}

defn_with_statement!(with_select_sessiondata_by_sessionid,
                     st_select_sessiondata_by_sessionid,
                     "select id, sessionid_hash, last_request_time, user_id, ip \
                      from SessionData \
                      where sessionid_hash = ?");
impl<'t> Transaction<'t> {
    pub fn get_sessiondata_by_sessionid_hash(
        &mut self, sessionid_hash: &Hash
    ) -> Result<Option<SessionData>, UniqueError>
    {
        self.connection_and_statements.with_select_sessiondata_by_sessionid(|sth| {
            get_unique_by("select_sessiondata_by_sessionid",
                          sth,
                          [sessionid_hash.as_bytes().as_ref()].as_ref())
        })
    }

    // NOTE: to avoid having to re-hash while re-trying transactions,
    // better use `sessionid_hash` and
    // `get_sessiondata_by_sessionid_hash` separately, thus do not
    // provide a `get_sessiondata_by_sessionid`.
}

defn_with_statement!(with_update_sessiondata,
                     st_update_sessiondata,
                     "update \"SessionData\" \
                      set (sessionid_hash, last_request_time, user_id, ip) \
                      = (?, ?, ?, ?) \
                      where id = ?");
impl<'t> Transaction<'t> {
    pub fn update_sessiondata(
        &mut self, sessiondata: &SessionData
    ) -> Result<(), SQLitePosError> {
        let _id = sessiondata.id.expect(
            "has id because it was read from DB, or caller provided it");
        self.connection_and_statements.with_update_sessiondata(|sth| {
            try_sqlite!(sth.reset());
            try_sqlite!(sessiondata.bind(sth));
            match try_sqlite!(sth.next()) {
                State::Done => Ok(()),
                _ => panic!("what happened?") // XX Err
            }
        })
    }
}

defn_with_statement!(with_insert_into_sessiondata,
                     st_insert_into_sessiondata,
                     "insert into \"SessionData\" \
                      (sessionid_hash, last_request_time, user_id, ip) \
                      values (?, ?, ?, ?)");
impl<'t> Transaction<'t> {
    pub fn insert_sessiondata(
        &mut self, sessiondata: &SessionData
    ) -> Result<(), SQLitePosError> {
        assert!(! sessiondata.id.is_some()); // relax if wanting to bypass auto-increment?
        self.connection_and_statements.with_insert_into_sessiondata(|sth| {
            try_sqlite!(sth.reset());
            try_sqlite!(sessiondata.bind(sth));
            match try_sqlite!(sth.next()) {
                State::Done => Ok(()),
                _ => panic!("what happened?") // XX Err
            }
        })
    }
}

