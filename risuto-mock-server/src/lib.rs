use std::{
    collections::{hash_map, HashMap},
    sync::Arc,
};

use risuto_client::{
    api::{
        self, Action, AuthInfo, AuthToken, Event, NewSession, NewUser, Query, Search, Tag, UserId,
        Uuid,
    },
    DbDump,
};

pub enum Error {
    NameAlreadyUsed,
    PermissionDenied,
    UuidAlreadyUsed,
}

pub struct MockServer(HashMap<UserId, DbUser>);

struct DbUser {
    // uid is in db.owner
    name: String,
    pass_hash: String,
    sessions: HashMap<AuthToken, Device>,
    db: DbDump,
}

struct Device(String);

impl MockServer {
    pub fn admin_create_user(&mut self, u: NewUser) -> Result<(), Error> {
        if self.0.values().any(|db| db.name == u.name) {
            return Err(Error::NameAlreadyUsed);
        }

        match self.0.entry(u.id) {
            hash_map::Entry::Occupied(_) => Err(Error::UuidAlreadyUsed),
            hash_map::Entry::Vacant(entry) => {
                entry.insert(DbUser {
                    name: u.name.clone(),
                    pass_hash: u.initial_password_hash,
                    sessions: HashMap::new(),
                    db: DbDump {
                        owner: u.id,
                        users: Arc::new(HashMap::new()),
                        tags: Arc::new(HashMap::new()),
                        searches: Arc::new(HashMap::new()),
                        perms: Arc::new(HashMap::new()),
                        tasks: Arc::new(HashMap::new()),
                    },
                });
                for db in self.0.values_mut() {
                    db.db.add_users(vec![api::User {
                        id: u.id,
                        name: u.name.clone(),
                    }]);
                }
                Ok(())
            }
        }
    }

    pub fn auth(&mut self, s: NewSession) -> Result<AuthToken, Error> {
        for u in self.0.values_mut() {
            if u.name == s.user {
                if !matches!(bcrypt::verify(&s.password, &u.pass_hash), Ok(true)) {
                    return Err(Error::PermissionDenied);
                } else {
                    let tok = AuthToken(Uuid::new_v4());
                    u.sessions.insert(tok, Device(s.device));
                    return Ok(tok);
                }
            }
        }
        Err(Error::PermissionDenied)
    }

    fn resolve(&self, tok: AuthToken) -> Result<&DbUser, Error> {
        for u in self.0.values() {
            if u.sessions.contains_key(&tok) {
                return Ok(u);
            }
        }
        Err(Error::PermissionDenied)
    }

    fn resolve_mut(&mut self, tok: AuthToken) -> Result<&mut DbUser, Error> {
        for u in self.0.values_mut() {
            if u.sessions.contains_key(&tok) {
                return Ok(u);
            }
        }
        Err(Error::PermissionDenied)
    }

    pub fn unauth(&mut self, tok: AuthToken) -> Result<(), Error> {
        let u = self.resolve_mut(tok)?;
        u.sessions.remove(&tok);
        Ok(())
    }

    pub fn whoami(&self, tok: AuthToken) -> Result<UserId, Error> {
        let u = self.resolve(tok)?;
        Ok(u.db.owner)
    }

    pub fn fetch_users(&self, tok: AuthToken) -> Result<Vec<api::User>, Error> {
        let _u = self.resolve(tok)?;
        Ok(self
            .0
            .values()
            .map(|u| api::User {
                id: u.db.owner,
                name: u.name.clone(),
            })
            .collect())
    }

    pub fn fetch_tags(&self, tok: AuthToken) -> Result<Vec<(Tag, AuthInfo)>, Error> {
        let u = self.resolve(tok)?;
        Ok(u.db
            .tags
            .iter()
            .map(|(id, t)| (t.clone(), *u.db.perms.get(id).unwrap()))
            .collect())
    }

    pub fn fetch_searches(&self, tok: AuthToken) -> Result<Vec<Search>, Error> {
        let u = self.resolve(tok)?;
        Ok(u.db.searches.values().cloned().collect())
    }

    pub fn search_tasks(
        &self,
        tok: AuthToken,
        q: Query,
    ) -> Result<(Vec<api::Task>, Vec<Event>), Error> {
        let u = self.resolve(tok)?;
        let mut tasks = Vec::new();
        let mut evts = Vec::new();
        for t in u.db.search(&Search::stub_for_query(q)) {
            tasks.push(api::Task {
                id: t.id,
                owner_id: t.owner_id,
                date: t.date,
                initial_title: t.initial_title.clone(),
                top_comment_id: t.top_comment.creation_id,
            });
            evts.extend(t.events.values().flat_map(|e| e.iter()).cloned());
        }
        Ok((tasks, evts))
    }

    pub fn submit_action(&mut self, tok: AuthToken, a: Action) -> Result<(), Error> {
        self.resolve(tok)?;
        match a {
            Action::NewTask(t, top_comm) => {
                let u = self.resolve_mut(tok)?;
                u.db.add_tasks(vec![t.clone()]);
                u.db.add_events_and_refresh_all(vec![api::Event {
                    id: t.top_comment_id,
                    owner_id: t.owner_id,
                    date: t.date,
                    task_id: t.id,
                    data: api::EventData::AddComment {
                        text: top_comm,
                        parent_id: None,
                    },
                }]);
            }
            Action::NewEvent(e) => {
                for u in self.0.values_mut() {
                    if u.db.tasks.contains_key(&e.task_id) {
                        u.db.add_events_and_refresh_all(vec![e.clone()]);
                    }
                }
            }
        }
        Ok(())
    }
}
