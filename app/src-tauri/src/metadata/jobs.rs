//! What the metadata worker does (PLAN.md Phase 4.5), apart from its thread
//! and Tauri (`worker`), so tests run it a step at a time with the fake
//! transport and clock.
//!
//! A job is an album to match on MusicBrainz (`Job::Match`, followed by its
//! cover and then its description when they're wanted), whose cover to
//! fetch from the Cover Art Archive (`Job::Cover`, followed by its
//! description), or whose description to fetch from Wikipedia
//! (`Job::Description`); or an artist to match on MusicBrainz
//! (`Job::Artist`, followed by its biography) or whose biography to fetch
//! from Wikipedia (`Job::Biography`). Jobs wait in `Jobs` by priority: what
//! the user asked for, then the artist page being looked at, then the album
//! playing, then background enrichment, in the order they came within each.
//! A job is queued once; asking for it again at a higher priority moves it
//! forward.
//!
//! Whether a job is still needed, and whether its source may be used, is
//! decided when it runs, from the settings and the database as they are
//! then. Automatic jobs (all but the user's requests) run only while "match
//! automatically" is on, and follow the skip rules in
//! `needs_match`, `needs_cover`, `needs_description`, `needs_artist_match`
//! and `needs_biography`; a user's request skips the waits in those rules.
//!
//! Offline: `http::Client` backs off from a host it couldn't reach.
//! Automatic jobs for that host stay queued meanwhile, and `Worker::step`
//! says how long to wait before the host is tried again, while jobs for
//! other hosts go on. A user's request is tried at once, and fails with
//! `Error::Offline` while its host is backing off.
//!
//! Events go through `Host`: art changes at once (to drop cached art), and
//! the albums and artists changed in batches at most a second apart, so a long run
//! doesn't flood the UI, with progress at most four times a second.
//!
//! Besides jobs, the worker runs calls (`Shared::call`): closures that use
//! its client and connection for something the user waits on, such as the
//! candidates in a "Find details" dialog. They run ahead of every job, and
//! their changes are sent at once.

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use super::albums::{self, LinkStatus};
use super::artists;
use super::cache::unix_now;
use super::coverartarchive::{self, Fetched};
use super::http::{Client, Clock};
use super::images::ImageCache;
use super::musicbrainz;
use super::settings::{self, Kind, ServiceSettings, SourceId};
use super::wikipedia;
use super::Error;

/// Seconds after which a search that found nothing, or only a doubtful
/// candidate, is tried again (PLAN.md Phase 4); likewise a cover the archive
/// didn't have, and a biography that wasn't found.
pub const RETRY_AFTER: i64 = 30 * 86400;

/// Albums and artists changed are reported at most this often.
const BATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Progress is reported at most this often, besides pausing and going idle.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Job {
    /// Match the album on MusicBrainz if it needs it, then fetch its cover
    /// and description if they're wanted.
    Match(i64),
    /// Fetch the album's cover from the Cover Art Archive if it needs it,
    /// then its description if that's wanted.
    Cover(i64),
    /// Fetch the album's description from Wikipedia if it needs it.
    Description(i64),
    /// Match the artist on MusicBrainz if it needs it, then fetch its
    /// biography if that's wanted.
    Artist(i64),
    /// Fetch the artist's biography from Wikipedia if it needs it.
    Biography(i64),
}

/// What a job is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subject {
    Album(i64),
    Artist(i64),
}

impl Job {
    pub fn subject(self) -> Subject {
        match self {
            Job::Match(id) | Job::Cover(id) | Job::Description(id) => Subject::Album(id),
            Job::Artist(id) | Job::Biography(id) => Subject::Artist(id),
        }
    }

    /// The hosts the job talks to, for pausing while one is unreachable.
    fn hosts(self) -> &'static [&'static str] {
        match self {
            Job::Match(_) | Job::Artist(_) => &[musicbrainz::HOST],
            Job::Cover(_) => &[coverartarchive::HOST],
            // The release group's links come from MusicBrainz.
            Job::Description(_) => &[musicbrainz::HOST, wikipedia::WIKIDATA_HOST, wikipedia::HOST],
            Job::Biography(_) => &[wikipedia::WIKIDATA_HOST, wikipedia::HOST],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Enrichment of the whole library.
    Background,
    /// The album playing; automatic, like background work.
    Playing,
    /// The artist page the user is looking at; automatic too, but the
    /// page is waiting for it.
    Viewing,
    /// Asked for by the user, who waits for the answer.
    User,
}

/// Where a user's request gets its answer once the job, and the jobs that
/// follow it (an album's cover and description after its match), are done.
pub type Reply = mpsc::Sender<Result<(), Error>>;

/// Work the user waits on, run on the worker with its client and
/// connection (`Shared::call`). It returns what sends its answer, which the
/// worker runs once the changes it made are reported. Dropped unrun when the
/// worker stops, which drops whatever it would have answered through.
pub type Call = Box<dyn FnOnce(&mut CallContext) -> Answer + Send>;

/// Sends a call's answer.
pub type Answer = Box<dyn FnOnce() + Send>;

/// What a call works with.
pub struct CallContext<'a> {
    pub client: &'a Client,
    pub conn: &'a Connection,
    pub images: &'a ImageCache,
    art: Vec<i64>,
    albums: Vec<i64>,
    artists: Vec<i64>,
}

impl CallContext<'_> {
    /// Album `album_id`'s details changed.
    pub fn album_changed(&mut self, album_id: i64) {
        self.albums.push(album_id);
    }

    /// Album `album_id`'s art may be different now.
    pub fn art_changed(&mut self, album_id: i64) {
        self.art.push(album_id);
    }

    pub fn artist_changed(&mut self, artist_id: i64) {
        self.artists.push(artist_id);
    }
}

struct Entry {
    priority: Priority,
    /// The highest automatic priority the job was also queued at, so a
    /// user's request that fails offline leaves it queued for that.
    automatic: Option<Priority>,
    replies: Vec<Reply>,
    /// Place within its priority; lower goes first.
    seq: i64,
}

/// The queue of jobs.
#[derive(Default)]
pub struct Jobs {
    entries: HashMap<Job, Entry>,
    order: BTreeSet<(Reverse<Priority>, i64, Job)>,
    back: i64,
    front: i64,
    /// Albums and artists with queued jobs, and how many.
    pending: HashMap<Subject, usize>,
    /// Albums and artists queued since the queue was last idle, for
    /// progress.
    seen: HashSet<Subject>,
}

impl Jobs {
    /// Queues `job` at `priority`, or moves it forward if it's queued lower.
    pub fn push(&mut self, job: Job, priority: Priority, reply: Option<Reply>) {
        let entry = Entry {
            priority,
            automatic: (priority != Priority::User).then_some(priority),
            replies: reply.into_iter().collect(),
            seq: 0,
        };
        self.insert(job, entry, false);
    }

    /// Queues `job` with what `entry` carries, at the back of its priority or
    /// the front.
    fn insert(&mut self, job: Job, mut entry: Entry, front: bool) {
        let seq = if front {
            self.front -= 1;
            self.front
        } else {
            self.back += 1;
            self.back
        };
        self.seen.insert(job.subject());
        if let Some(queued) = self.entries.get_mut(&job) {
            queued.automatic = queued.automatic.max(entry.automatic);
            queued.replies.append(&mut entry.replies);
            if entry.priority > queued.priority {
                self.order
                    .remove(&(Reverse(queued.priority), queued.seq, job));
                queued.priority = entry.priority;
                queued.seq = seq;
                self.order.insert((Reverse(queued.priority), seq, job));
            }
            return;
        }
        entry.seq = seq;
        self.order.insert((Reverse(entry.priority), seq, job));
        *self.pending.entry(job.subject()).or_default() += 1;
        self.entries.insert(job, entry);
    }

    /// Takes the first job, in order, that `runnable` accepts.
    fn take(&mut self, runnable: impl Fn(Job, Priority) -> bool) -> Option<(Job, Entry)> {
        let key = *self
            .order
            .iter()
            .find(|(Reverse(priority), _, job)| runnable(*job, *priority))?;
        self.order.remove(&key);
        let job = key.2;
        let entry = self.entries.remove(&job)?;
        if let Some(count) = self.pending.get_mut(&job.subject()) {
            *count -= 1;
            if *count == 0 {
                self.pending.remove(&job.subject());
            }
        }
        Some((job, entry))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The hosts that queued jobs talk to.
    fn hosts(&self) -> HashSet<&'static str> {
        self.entries
            .keys()
            .flat_map(|job| job.hosts().iter().copied())
            .collect()
    }

    /// Albums and artists finished, and in all, since the queue was last
    /// idle, with `running` being worked on.
    fn counts(&self, running: Option<Subject>) -> (usize, usize) {
        let total = self.seen.len();
        let busy =
            self.pending.len() + running.is_some_and(|id| !self.pending.contains_key(&id)) as usize;
        (total.saturating_sub(busy), total)
    }
}

/// The queue and the requests to the worker, shared with other threads.
#[derive(Default)]
pub struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

#[derive(Default)]
struct State {
    jobs: Jobs,
    calls: VecDeque<Call>,
    /// Set by every request and cleared when the worker looks at the queue,
    /// so a request made while it decides to wait isn't missed.
    woken: bool,
    enrich: bool,
    retry_now: bool,
    stop: bool,
    progress: Progress,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn wake(&self, f: impl FnOnce(&mut State)) {
        let mut state = self.lock();
        f(&mut state);
        state.woken = true;
        self.wake.notify_all();
    }

    /// Queues `job`. Once the worker has stopped, `reply` is dropped, so
    /// whoever waits on it hears at once.
    pub fn request(&self, job: Job, priority: Priority, reply: Option<Reply>) {
        self.wake(|state| {
            if !state.stop {
                state.jobs.push(job, priority, reply);
            }
        });
    }

    /// Runs `call` on the worker ahead of every job. Once the worker has
    /// stopped, it's dropped unrun.
    pub fn call(&self, call: Call) {
        self.wake(|state| {
            if !state.stop {
                state.calls.push_back(call);
            }
        });
    }

    /// Queues background enrichment of every album and album artist that
    /// needs it, if "match automatically" is on when the worker gets to it.
    pub fn enrich_library(&self) {
        self.wake(|state| state.enrich = true);
    }

    /// Tries unreachable services again now rather than after their wait.
    pub fn retry_now(&self) {
        self.wake(|state| state.retry_now = true);
    }

    /// Makes `Worker::run` return once the job it's on, if any, is done.
    /// Queued jobs are dropped with their replies.
    pub fn stop(&self) {
        self.wake(|state| {
            state.stop = true;
            state.jobs = Jobs::default();
            state.calls.clear();
        });
    }

    /// The progress last reported.
    pub fn progress(&self) -> Progress {
        self.lock().progress.clone()
    }

    /// Waits for a request, or until `timeout` if one is given.
    fn wait(&self, timeout: Option<Duration>) {
        let state = self.lock();
        let idle = |state: &mut State| !state.woken && !state.stop;
        match timeout {
            Some(timeout) => drop(self.wake.wait_timeout_while(state, timeout, idle)),
            None => drop(self.wake.wait_while(state, idle)),
        }
    }
}

/// The worker's progress, as the `metadata-progress` event sends it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Albums and artists finished, and in all, since the worker was last
    /// idle; both 0 when it is.
    pub done: usize,
    pub total: usize,
    /// The album or artist being worked on.
    pub current: Option<Current>,
    /// Automatic work waits for a service that couldn't be reached.
    pub paused: bool,
    /// Hosts that couldn't be reached, which are tried again later.
    pub unreachable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Current {
    #[serde(rename_all = "camelCase")]
    Album {
        album_id: i64,
        title: String,
        artist: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Artist { artist_id: i64, name: String },
}

/// Albums and artists whose details or art changed, as the
/// `metadata-changed` event sends them.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataChanged {
    pub albums: Vec<i64>,
    pub artists: Vec<i64>,
}

/// Where the worker reports: the app, or a fake in tests.
pub trait Host {
    /// Album `album_id`'s art may be different now: forget any copy kept.
    /// Called at once, before the batch naming the album is sent.
    fn art_changed(&mut self, album_id: i64);
    fn metadata_changed(&mut self, changed: &MetadataChanged);
    fn progress(&mut self, progress: &Progress);
}

/// What `Worker::step` did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    Ran,
    /// Nothing queued.
    Idle,
    /// Only automatic jobs for unreachable hosts are queued; the first of
    /// those hosts is tried again after this long.
    Paused(Duration),
    Stopped,
}

pub struct Worker<H: Host> {
    shared: Arc<Shared>,
    client: Client,
    clock: Box<dyn Clock>,
    conn: Connection,
    images: Arc<ImageCache>,
    host: H,
    albums_changed: BTreeSet<i64>,
    artists_changed: BTreeSet<i64>,
    flushed_at: Option<Instant>,
    reported_at: Option<Instant>,
    reported: Progress,
    running: Option<Subject>,
}

impl<H: Host> Worker<H> {
    /// A worker over `conn`, its own connection. `clock` is `client`'s.
    pub fn new(
        shared: Arc<Shared>,
        client: Client,
        clock: Box<dyn Clock>,
        conn: Connection,
        images: Arc<ImageCache>,
        host: H,
    ) -> Worker<H> {
        Worker {
            shared,
            client,
            clock,
            conn,
            images,
            host,
            albums_changed: BTreeSet::new(),
            artists_changed: BTreeSet::new(),
            flushed_at: None,
            reported_at: None,
            reported: Progress::default(),
            running: None,
        }
    }

    /// Runs jobs until `Shared::stop`, waiting when there are none.
    pub fn run(mut self) {
        loop {
            let wait = match self.step() {
                Step::Stopped => return,
                Step::Ran => continue,
                Step::Idle => None,
                Step::Paused(wait) => Some(wait),
            };
            self.shared.wait(wait);
        }
    }

    /// Runs the next job that can run now, if any.
    pub fn step(&mut self) -> Step {
        let (enrich, retry_now, call) = {
            let mut state = self.shared.lock();
            if state.stop {
                return Step::Stopped;
            }
            state.woken = false;
            (
                std::mem::take(&mut state.enrich),
                std::mem::take(&mut state.retry_now),
                state.calls.pop_front(),
            )
        };
        if retry_now {
            self.client.retry_now();
        }
        if let Some(call) = call {
            self.run_call(call);
            return Step::Ran;
        }
        if enrich {
            if let Err(error) = self.queue_enrichment() {
                eprintln!("[metadata] enrichment: {error}");
            }
        }

        let client = &self.client;
        let (next, retry_at) = {
            let mut state = self.shared.lock();
            let next = state.jobs.take(|job, priority| {
                priority == Priority::User
                    || job
                        .hosts()
                        .iter()
                        .all(|host| client.retry_at(host).is_none())
            });
            let retry_at = match next {
                Some(_) => None,
                None => {
                    let retry_at = state
                        .jobs
                        .hosts()
                        .into_iter()
                        .filter_map(|host| client.retry_at(host))
                        .min();
                    if state.jobs.len() == 0 {
                        state.jobs.seen.clear();
                    }
                    retry_at
                }
            };
            (next, retry_at)
        };
        match next {
            Some((job, entry)) => {
                self.running = Some(job.subject());
                self.report(false);
                self.run_job(job, entry);
                self.running = None;
                Step::Ran
            }
            None => {
                self.flush();
                self.report(true);
                match retry_at {
                    Some(at) => Step::Paused(at.saturating_duration_since(self.clock.now())),
                    None => Step::Idle,
                }
            }
        }
    }

    /// Queues every album that needs matching, a cover or a description,
    /// then every album artist that needs matching or a biography, as
    /// background work, if
    /// the settings allow it. Albums go first, since their matches help
    /// match their artists. Artists are looked up in the background only
    /// for a biography; the artist page looks them up when shown anyway.
    fn queue_enrichment(&mut self) -> Result<(), Error> {
        let settings = settings::service_settings(&self.conn)?;
        let usable = Usable::new(&settings);
        if !settings.auto_match || !usable.musicbrainz {
            return Ok(());
        }
        let now = unix_now();
        let album_ids: Vec<i64> = self
            .conn
            .prepare("SELECT id FROM albums ORDER BY id")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let mut jobs = Vec::new();
        for album_id in album_ids {
            if needs_match(&self.conn, album_id, now, false)? {
                jobs.push(Job::Match(album_id));
            } else if usable.cover_art
                && needs_cover(&self.conn, &self.images, album_id, now, false)?
            {
                jobs.push(Job::Cover(album_id));
            } else if usable.description && needs_description(&self.conn, album_id, now, false)? {
                jobs.push(Job::Description(album_id));
            }
        }
        if usable.biography {
            let artist_ids: Vec<i64> = self
                .conn
                .prepare(
                    "SELECT DISTINCT artist_id FROM albums WHERE artist_id IS NOT NULL
                     ORDER BY artist_id",
                )?
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            for artist_id in artist_ids {
                if needs_artist_match(&self.conn, artist_id, now, false)? {
                    jobs.push(Job::Artist(artist_id));
                } else if needs_biography(&self.conn, artist_id, now, false)? {
                    jobs.push(Job::Biography(artist_id));
                }
            }
        }
        let mut state = self.shared.lock();
        for job in jobs {
            state.jobs.push(job, Priority::Background, None);
        }
        Ok(())
    }

    fn run_call(&mut self, call: Call) {
        let mut context = CallContext {
            client: &self.client,
            conn: &self.conn,
            images: &self.images,
            art: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
        };
        let answer = call(&mut context);
        let CallContext {
            art,
            albums,
            artists,
            ..
        } = context;
        for album_id in art {
            self.art_changed(album_id);
        }
        self.albums_changed.extend(albums);
        self.artists_changed.extend(artists);
        self.flush();
        answer();
    }

    fn run_job(&mut self, job: Job, entry: Entry) {
        let entry_priority = entry.priority;
        let user = entry.priority == Priority::User;
        let result = self.attempt(job, user);
        let requeue = |state: &mut State, priority| {
            let entry = Entry {
                priority,
                automatic: Some(priority),
                replies: Vec::new(),
                seq: 0,
            };
            state.jobs.insert(job, entry, true);
        };
        let replies = match result {
            // What follows next, right away, with the replies waiting for it.
            Ok(Some(next)) => {
                self.shared.lock().jobs.insert(next, entry, true);
                Vec::new()
            }
            Ok(None) => entry
                .replies
                .into_iter()
                .map(|reply| (reply, Ok(())))
                .collect(),
            // Kept for when the host can be reached again.
            Err(Error::Offline(_)) if !user => {
                self.shared.lock().jobs.insert(job, entry, true);
                Vec::new()
            }
            Err(error) => {
                if let (Error::Offline(_), Some(priority)) = (&error, entry.automatic) {
                    requeue(&mut self.shared.lock(), priority);
                } else if !matches!(error, Error::Offline(_)) {
                    eprintln!("[metadata] {job:?}: {error}");
                }
                entry
                    .replies
                    .into_iter()
                    .map(|reply| (reply, Err(duplicate(&error))))
                    .collect()
            }
        };
        // Whoever waits for the answer, or the page shown, sees the changes
        // with it.
        let now = self.clock.now();
        if entry_priority >= Priority::Viewing
            || self.flushed_at.is_none_or(|at| now - at >= BATCH_INTERVAL)
        {
            self.flush();
        }
        for (reply, result) in replies {
            let _ = reply.send(result);
        }
    }

    /// Does `job` if it's still needed. Returns the job that follows it.
    fn attempt(&mut self, job: Job, user: bool) -> Result<Option<Job>, Error> {
        let settings = settings::service_settings(&self.conn)?;
        if !user && !settings.auto_match {
            return Ok(None);
        }
        let usable = Usable::new(&settings);
        let (sql, what, id) = match job.subject() {
            Subject::Album(id) => ("SELECT id FROM albums WHERE id = ?1", "album", id),
            Subject::Artist(id) => ("SELECT id FROM artists WHERE id = ?1", "artist", id),
        };
        let exists: Option<i64> = self
            .conn
            .query_row(sql, [id], |row| row.get(0))
            .optional()?;
        if exists.is_none() {
            // Gone in a rescan since it was queued.
            return match user {
                true => Err(Error::Invalid(format!("No {what} with id {id}"))),
                false => Ok(None),
            };
        }
        let now = unix_now();
        match job {
            Job::Match(album_id) => {
                if !usable.musicbrainz {
                    return match user {
                        true => Err(turned_off(&settings, SourceId::MusicBrainz)),
                        false => Ok(None),
                    };
                }
                if needs_match(&self.conn, album_id, now, user)? {
                    self.match_album(album_id)?;
                }
                if usable.cover_art && needs_cover(&self.conn, &self.images, album_id, now, user)? {
                    return Ok(Some(Job::Cover(album_id)));
                }
                self.description_next(&usable, album_id, now, user)
            }
            Job::Cover(album_id) => {
                if !usable.cover_art {
                    return match user {
                        true => Err(turned_off(&settings, SourceId::CoverArtArchive)),
                        false => Ok(None),
                    };
                }
                if needs_cover(&self.conn, &self.images, album_id, now, user)?
                    && coverartarchive::fetch_album_art(
                        &self.client,
                        &self.conn,
                        &self.images,
                        album_id,
                    )? == Fetched::Downloaded
                {
                    self.art_changed(album_id);
                }
                self.description_next(&usable, album_id, now, user)
            }
            Job::Description(album_id) => {
                if !usable.description {
                    return match user {
                        true => Err(turned_off(&settings, SourceId::Wikipedia)),
                        false => Ok(None),
                    };
                }
                if needs_description(&self.conn, album_id, now, user)? {
                    let before = wikipedia::description(&self.conn, album_id)?;
                    wikipedia::fetch_description(&self.client, &self.conn, album_id)?;
                    if wikipedia::description(&self.conn, album_id)? != before {
                        self.albums_changed.insert(album_id);
                    }
                }
                Ok(None)
            }
            Job::Artist(artist_id) => {
                if !usable.musicbrainz {
                    return match user {
                        true => Err(turned_off(&settings, SourceId::MusicBrainz)),
                        false => Ok(None),
                    };
                }
                if needs_artist_match(&self.conn, artist_id, now, user)? {
                    self.match_artist(artist_id)?;
                }
                let biography =
                    usable.biography && needs_biography(&self.conn, artist_id, now, user)?;
                Ok(biography.then_some(Job::Biography(artist_id)))
            }
            Job::Biography(artist_id) => {
                if !usable.biography {
                    return match user {
                        true => Err(turned_off(&settings, SourceId::Wikipedia)),
                        false => Ok(None),
                    };
                }
                if needs_biography(&self.conn, artist_id, now, user)? {
                    let before = wikipedia::biography(&self.conn, artist_id)?;
                    wikipedia::fetch_biography(&self.client, &self.conn, artist_id)?;
                    if wikipedia::biography(&self.conn, artist_id)? != before {
                        self.artists_changed.insert(artist_id);
                    }
                }
                Ok(None)
            }
        }
    }

    /// The description job that follows album `album_id`'s match or cover,
    /// if its description is wanted.
    fn description_next(
        &self,
        usable: &Usable,
        album_id: i64,
        now: i64,
        user: bool,
    ) -> Result<Option<Job>, Error> {
        let wanted = usable.description && needs_description(&self.conn, album_id, now, user)?;
        Ok(wanted.then_some(Job::Description(album_id)))
    }

    /// Matches the artist and notes whether the match changed.
    fn match_artist(&mut self, artist_id: i64) -> Result<(), Error> {
        let link = |conn: &Connection| -> Result<_, Error> {
            Ok(
                artists::artist_link(conn, artist_id, SourceId::MusicBrainz)?
                    .map(|link| (link.status, link.external_id, link.details)),
            )
        };
        let before = link(&self.conn)?;
        artists::match_artist(&self.client, &self.conn, artist_id)?;
        if link(&self.conn)? != before {
            self.artists_changed.insert(artist_id);
        }
        Ok(())
    }

    /// Matches the album and notes what changed. A different match can mean
    /// a different cover, so the old one must not keep showing: the art is
    /// looked up again (its URLs come from the match), and a "no cover"
    /// record for the old release no longer applies.
    fn match_album(&mut self, album_id: i64) -> Result<(), Error> {
        let link = |conn: &Connection| -> Result<_, Error> {
            Ok(albums::album_link(conn, album_id, SourceId::MusicBrainz)?
                .map(|link| (link.status, link.external_id)))
        };
        let (before, covers_before) = (
            link(&self.conn)?,
            coverartarchive::cover_urls(&self.conn, album_id)?,
        );
        albums::match_album(&self.client, &self.conn, album_id)?;
        if coverartarchive::cover_urls(&self.conn, album_id)? != covers_before {
            self.art_changed(album_id);
        } else if link(&self.conn)? != before {
            self.albums_changed.insert(album_id);
        }
        Ok(())
    }

    fn art_changed(&mut self, album_id: i64) {
        self.host.art_changed(album_id);
        self.albums_changed.insert(album_id);
    }

    /// Sends the albums and artists changed since the last batch, if any.
    fn flush(&mut self) {
        if self.albums_changed.is_empty() && self.artists_changed.is_empty() {
            return;
        }
        let changed = MetadataChanged {
            albums: std::mem::take(&mut self.albums_changed)
                .into_iter()
                .collect(),
            artists: std::mem::take(&mut self.artists_changed)
                .into_iter()
                .collect(),
        };
        self.host.metadata_changed(&changed);
        self.flushed_at = Some(self.clock.now());
    }

    /// Reports progress if it changed, unless it was reported very recently
    /// and this isn't `urgent`.
    fn report(&mut self, urgent: bool) {
        let now = self.clock.now();
        if !urgent
            && self
                .reported_at
                .is_some_and(|at| now - at < PROGRESS_INTERVAL)
        {
            return;
        }
        let (done, total) = self.shared.lock().jobs.counts(self.running);
        let unreachable = self.client.unreachable_hosts();
        let progress = Progress {
            done,
            total,
            current: self
                .running
                .and_then(|subject| current(&self.conn, subject).ok().flatten()),
            paused: self.running.is_none() && total > 0,
            unreachable,
        };
        if progress == self.reported {
            return;
        }
        if progress.paused != self.reported.paused {
            match progress.paused {
                true => eprintln!(
                    "[metadata] paused: {} can't be reached",
                    progress.unreachable.join(", ")
                ),
                false => eprintln!("[metadata] resumed"),
            }
        }
        self.host.progress(&progress);
        self.shared.lock().progress = progress.clone();
        self.reported = progress;
        self.reported_at = Some(now);
    }
}

/// Which online sources the settings allow now.
struct Usable {
    musicbrainz: bool,
    cover_art: bool,
    description: bool,
    biography: bool,
}

impl Usable {
    fn new(settings: &ServiceSettings) -> Usable {
        Usable {
            musicbrainz: settings
                .sources_for(Kind::Release)
                .contains(&SourceId::MusicBrainz),
            cover_art: settings
                .sources_for(Kind::AlbumArt)
                .contains(&SourceId::CoverArtArchive),
            description: settings
                .sources_for(Kind::AlbumInfo)
                .contains(&SourceId::Wikipedia),
            biography: settings
                .sources_for(Kind::ArtistInfo)
                .contains(&SourceId::Wikipedia),
        }
    }
}

/// The error for a request to `source` while the settings turn it off.
pub fn turned_off(settings: &ServiceSettings, source: SourceId) -> Error {
    Error::Invalid(if settings.online {
        format!("{} is turned off", source.info().name)
    } else {
        "Online services are turned off".into()
    })
}

/// A copy of `error` for each reply; a database error goes as its message.
fn duplicate(error: &Error) -> Error {
    match error {
        Error::Offline(message) => Error::Offline(message.clone()),
        Error::Status { url, status } => Error::Status {
            url: url.clone(),
            status: *status,
        },
        other => Error::Invalid(other.to_string()),
    }
}

fn current(conn: &Connection, subject: Subject) -> Result<Option<Current>, Error> {
    Ok(match subject {
        Subject::Album(album_id) => conn
            .query_row(
                "SELECT al.title, ar.name FROM albums al
                 LEFT JOIN artists ar ON ar.id = al.artist_id
                 WHERE al.id = ?1",
                [album_id],
                |row| {
                    Ok(Current::Album {
                        album_id,
                        title: row.get(0)?,
                        artist: row.get(1)?,
                    })
                },
            )
            .optional()?,
        Subject::Artist(artist_id) => conn
            .query_row(
                "SELECT name FROM artists WHERE id = ?1",
                [artist_id],
                |row| {
                    Ok(Current::Artist {
                        artist_id,
                        name: row.get(0)?,
                    })
                },
            )
            .optional()?,
    })
}

/// Whether album `album_id` should be matched on MusicBrainz: it never was,
/// or it was and nothing was found or only a doubtful candidate ('none',
/// 'review') more than `RETRY_AFTER` seconds before `now`, or at all with
/// `force`. An accepted match, or one the user chose, is kept.
///
/// 'review' is retried like 'none' because MusicBrainz grows and a better
/// release may be there later; meanwhile the candidate waits for the user
/// (the "Find details" dialog) and gets no cover.
pub fn needs_match(conn: &Connection, album_id: i64, now: i64, force: bool) -> Result<bool, Error> {
    Ok(
        match albums::album_link(conn, album_id, SourceId::MusicBrainz)? {
            None => true,
            Some(link) if link.chosen_by_user => false,
            Some(link) => match link.status {
                LinkStatus::Matched => false,
                LinkStatus::Review | LinkStatus::None => {
                    force || now - link.checked_at >= RETRY_AFTER
                }
            },
        },
    )
}

/// Whether album `album_id`'s cover should be fetched from the Cover Art
/// Archive: the archive picture the user chose if it isn't downloaded, else,
/// for an album matched to a release (by the user or not), its cover if none
/// of its cover URLs is downloaded, unless the archive had none for that
/// release less than `RETRY_AFTER` seconds before `now` (without `force`).
/// Never when the user chose a picture from another source.
pub fn needs_cover(
    conn: &Connection,
    images: &ImageCache,
    album_id: i64,
    now: i64,
    force: bool,
) -> Result<bool, Error> {
    let chosen: Option<String> = conn
        .prepare_cached("SELECT source FROM album_art WHERE album_id = ?1")?
        .query_row([album_id], |row| row.get(0))
        .optional()?;
    // A source this version doesn't know is ignored, as `art::lookup` does.
    if chosen
        .as_deref()
        .and_then(SourceId::from_str)
        .is_some_and(|source| source != SourceId::CoverArtArchive)
    {
        return Ok(false);
    }
    if let Some(url) = coverartarchive::chosen_image(conn, album_id)? {
        return Ok(!images.contains(&url));
    }
    let Some(link) = albums::album_link(conn, album_id, SourceId::MusicBrainz)? else {
        return Ok(false);
    };
    let urls = coverartarchive::link_cover_urls(&link);
    let Some(release_id) = coverartarchive::matched_release(&link) else {
        return Ok(false);
    };
    if urls.is_empty() || urls.iter().any(|url| images.contains(url)) {
        return Ok(false);
    }
    let missing_lately = coverartarchive::cover_check(conn, album_id)?.is_some_and(|check| {
        check.status == LinkStatus::None
            && check.external_id.as_deref() == Some(release_id)
            && now - check.checked_at < RETRY_AFTER
    });
    Ok(force || !missing_lately)
}

/// Whether album `album_id`'s description should be fetched: it's matched to
/// a MusicBrainz release (by the user or not), and no description was
/// fetched for that release's release group, or none was found more than
/// `RETRY_AFTER` seconds before `now` (or at all with `force`).
pub fn needs_description(
    conn: &Connection,
    album_id: i64,
    now: i64,
    force: bool,
) -> Result<bool, Error> {
    let Some(group_id) = albums::matched_release_group(conn, album_id)? else {
        return Ok(false);
    };
    Ok(match wikipedia::description_check(conn, album_id)? {
        Some(check) if check.external_id.as_deref() == Some(group_id.as_str()) => {
            match check.status {
                LinkStatus::Matched => false,
                LinkStatus::Review | LinkStatus::None => {
                    force || now - check.checked_at >= RETRY_AFTER
                }
            }
        }
        // Never fetched, or for another album.
        _ => true,
    })
}

/// Whether artist `artist_id` should be matched on MusicBrainz, by the
/// rules of `needs_match`.
pub fn needs_artist_match(
    conn: &Connection,
    artist_id: i64,
    now: i64,
    force: bool,
) -> Result<bool, Error> {
    Ok(
        match artists::artist_link(conn, artist_id, SourceId::MusicBrainz)? {
            None => true,
            Some(link) if link.chosen_by_user => false,
            Some(link) => match link.status {
                LinkStatus::Matched => false,
                LinkStatus::Review | LinkStatus::None => {
                    force || now - link.checked_at >= RETRY_AFTER
                }
            },
        },
    )
}

/// Whether artist `artist_id`'s biography should be fetched: it's matched
/// on MusicBrainz (by the user or not), and no biography was fetched for
/// that match, or none was found more than `RETRY_AFTER` seconds before
/// `now` (or at all with `force`).
pub fn needs_biography(
    conn: &Connection,
    artist_id: i64,
    now: i64,
    force: bool,
) -> Result<bool, Error> {
    let Some(link) = artists::artist_link(conn, artist_id, SourceId::MusicBrainz)? else {
        return Ok(false);
    };
    let Some(mbid) = link.matched_id() else {
        return Ok(false);
    };
    Ok(match wikipedia::biography_check(conn, artist_id)? {
        Some(check) if check.external_id.as_deref() == Some(mbid) => match check.status {
            LinkStatus::Matched => false,
            LinkStatus::Review | LinkStatus::None => force || now - check.checked_at >= RETRY_AFTER,
        },
        // Never fetched, or for another match.
        _ => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};
    use crate::metadata::coverartarchive::{release_front_url, release_group_front_url};
    use crate::metadata::http::testing::{response, FakeClock, FakeTransport};
    use crate::metadata::http::{Response, TransportError};
    use crate::metadata::images::testing::JPEG;
    use crate::metadata::musicbrainz::fixtures::{RELEASES, RELEASE_GROUP, SEARCH};
    use crate::metadata::musicbrainz::{release_url, search_url};
    use rusqlite::params;

    const MB: &str = "musicbrainz";
    const CAA: &str = "cover-art-archive";
    const DAY: i64 = 86400;

    #[derive(Default)]
    struct Recorded {
        art: Vec<i64>,
        changed: Vec<MetadataChanged>,
        progress: Vec<Progress>,
    }

    #[derive(Clone, Default)]
    struct FakeHost(Arc<Mutex<Recorded>>);

    impl FakeHost {
        fn take(&self) -> Recorded {
            std::mem::take(&mut self.0.lock().unwrap())
        }
    }

    impl Host for FakeHost {
        fn art_changed(&mut self, album_id: i64) {
            self.0.lock().unwrap().art.push(album_id);
        }
        fn metadata_changed(&mut self, changed: &MetadataChanged) {
            self.0.lock().unwrap().changed.push(changed.clone());
        }
        fn progress(&mut self, progress: &Progress) {
            self.0.lock().unwrap().progress.push(progress.clone());
        }
    }

    struct Test {
        worker: Worker<FakeHost>,
        shared: Arc<Shared>,
        host: FakeHost,
        transport: FakeTransport,
        clock: FakeClock,
        images: Arc<ImageCache>,
        _dir: tempfile::TempDir,
    }

    impl Test {
        /// A worker over `library`, with biographies turned off so the
        /// album tests see only album work.
        fn new(library: Library) -> Test {
            let test = Test::with_biographies(library);
            test.set_settings(|settings| settings.sources[4].enabled = false); // Wikipedia.
            test
        }

        fn with_biographies(library: Library) -> Test {
            let clock = FakeClock::new();
            let transport = FakeTransport::new(&clock);
            let client = Client::new(Box::new(transport.clone()), Box::new(clock.clone()));
            let dir = tempfile::tempdir().unwrap();
            let images = Arc::new(ImageCache::new(dir.path().to_path_buf()));
            let shared = Arc::new(Shared::default());
            let host = FakeHost::default();
            let worker = Worker::new(
                shared.clone(),
                client,
                Box::new(clock.clone()),
                library.conn,
                images.clone(),
                host.clone(),
            );
            Test {
                worker,
                shared,
                host,
                transport,
                clock,
                images,
                _dir: dir,
            }
        }

        fn conn(&self) -> &Connection {
            &self.worker.conn
        }

        /// Steps until the worker is idle or paused; returns that step.
        fn run(&mut self) -> Step {
            for _ in 0..1000 {
                match self.worker.step() {
                    Step::Ran => {}
                    step => return step,
                }
            }
            panic!("the worker never stopped");
        }

        fn request(&self, job: Job) -> mpsc::Receiver<Result<(), Error>> {
            let (reply, answer) = mpsc::channel();
            self.shared.request(job, Priority::User, Some(reply));
            answer
        }

        fn serve_jpeg(&self, url: &str) {
            self.transport.push(
                url,
                Ok(Response {
                    content_type: Some("image/jpeg".into()),
                    body: JPEG.to_vec(),
                    ..response(200, "")
                }),
            );
        }

        fn link(&self, album_id: i64) -> Option<albums::AlbumLink> {
            albums::album_link(self.conn(), album_id, SourceId::MusicBrainz).unwrap()
        }

        fn set_settings(&self, change: impl FnOnce(&mut ServiceSettings)) {
            let mut settings = settings::service_settings(self.conn()).unwrap();
            change(&mut settings);
            settings::save_service_settings(self.conn(), settings).unwrap();
        }
    }

    /// The durations of the 2016 release of "In Rainbows", plus a little
    /// encoder padding (as in `albums`' tests).
    const DURATIONS: [f64; 10] = [
        237.4, 242.9, 255.6, 318.5, 228.3, 129.8, 290.5, 328.9, 248.6, 279.2,
    ];

    /// "In Rainbows" by Radiohead (album 1), then one track each of
    /// `others` by "Nobody" (albums 2, 3…).
    fn library(others: &[&str]) -> Library {
        let library = Library::new(
            (1..=10)
                .map(|n| {
                    track(&format!("Radiohead/In Rainbows/{n:02}.flac"))
                        .artist("Radiohead")
                        .album("In Rainbows")
                        .year(2007)
                        .number(n)
                })
                .chain(others.iter().map(|title| {
                    track(&format!("Nobody/{title}/01.flac"))
                        .artist("Nobody")
                        .album(title)
                })),
        );
        for (n, duration) in DURATIONS.iter().enumerate() {
            library
                .conn
                .execute(
                    "UPDATE tracks SET duration = ?1, track_total = 10
                     WHERE album_id = 1 AND track_number = ?2",
                    params![duration, n as u32 + 1],
                )
                .unwrap();
        }
        library
    }

    fn serve_search(transport: &FakeTransport) {
        transport.push_status(
            &search_url("In Rainbows", Some("Radiohead"), Some(10)),
            200,
            SEARCH,
        );
        for (id, json) in RELEASES {
            transport.push_status(&release_url(id), 200, json);
        }
    }

    /// Serves an empty search for album `title` by "Nobody".
    fn serve_nothing(transport: &FakeTransport, title: &str) {
        transport.push_status(
            &search_url(title, Some("Nobody"), Some(1)),
            200,
            r#"{"releases": []}"#,
        );
    }

    fn set_link(
        conn: &Connection,
        album_id: i64,
        source: &str,
        status: &str,
        external_id: Option<&str>,
        chosen_by: &str,
        checked_at: i64,
    ) {
        conn.execute(
            "INSERT OR REPLACE INTO album_links
                 (album_id, source, status, external_id, score, chosen_by, details, checked_at)
             VALUES (?1, ?2, ?3, ?4, 1.0, ?5, NULL, ?6)",
            params![album_id, source, status, external_id, chosen_by, checked_at],
        )
        .unwrap();
    }

    /// A made-up release MBID; its only cover URL is the release's own,
    /// since a link without details has no release group.
    fn mbid(n: u32) -> String {
        format!("00000000-0000-0000-0000-{n:012}")
    }

    #[test]
    fn queues_by_priority_once() {
        let mut jobs = Jobs::default();
        for id in 1..=3 {
            jobs.push(Job::Match(id), Priority::Background, None);
        }
        jobs.push(Job::Match(2), Priority::Background, None);
        assert_eq!(jobs.len(), 3, "queued once");
        jobs.push(Job::Match(3), Priority::User, None);
        jobs.push(Job::Match(4), Priority::Playing, None);
        jobs.push(Job::Match(1), Priority::Playing, None);
        // The user's request moved forward; the album playing next, first
        // come first; then the rest in order.
        let mut order = Vec::new();
        while let Some((job, entry)) = jobs.take(|_, _| true) {
            order.push((job, entry.priority));
        }
        assert_eq!(
            order,
            [
                (Job::Match(3), Priority::User),
                (Job::Match(4), Priority::Playing),
                (Job::Match(1), Priority::Playing),
                (Job::Match(2), Priority::Background),
            ]
        );

        // A lower request doesn't move a job back, but is remembered.
        jobs.push(Job::Cover(5), Priority::User, None);
        jobs.push(Job::Cover(5), Priority::Background, None);
        let (_, entry) = jobs.take(|_, _| true).unwrap();
        assert_eq!(
            (entry.priority, entry.automatic),
            (Priority::User, Some(Priority::Background))
        );

        // At the front of its priority; skipping what isn't runnable.
        jobs.push(Job::Match(6), Priority::Background, None);
        jobs.insert(
            Job::Cover(7),
            Entry {
                priority: Priority::Background,
                automatic: Some(Priority::Background),
                replies: Vec::new(),
                seq: 0,
            },
            true,
        );
        jobs.push(Job::Match(8), Priority::Background, None);
        let covers_first = jobs.take(|_, _| true).unwrap().0;
        assert_eq!(covers_first, Job::Cover(7));
        let not_mb = jobs.take(|job, _| !job.hosts().contains(&musicbrainz::HOST));
        assert!(not_mb.is_none());
        assert_eq!(jobs.len(), 2);
    }

    #[test]
    fn runs_calls_first_and_reports_their_changes_before_answering() {
        let mut test = Test::new(library(&["Other"]));
        test.shared.request(Job::Match(2), Priority::User, None);
        let (reply, answer) = mpsc::channel();
        let host = test.host.clone();
        test.shared.call(Box::new(move |context| {
            let albums: i64 = context
                .conn
                .query_row("SELECT count(*) FROM albums", [], |row| row.get(0))
                .unwrap();
            context.art_changed(1);
            context.artist_changed(7);
            Box::new(move || {
                // The changes went out first.
                let recorded = host.take();
                let _ = reply.send((albums, recorded.art, recorded.changed));
            }) as Answer
        }));
        assert_eq!(test.worker.step(), Step::Ran);
        let (albums, art, changed) = answer.try_recv().unwrap();
        assert_eq!(albums, 2);
        assert_eq!(art, [1]);
        assert_eq!(
            changed,
            [MetadataChanged {
                albums: vec![1],
                artists: vec![7]
            }]
        );
        assert!(test.transport.urls().is_empty(), "the job waits");
        assert_eq!(test.shared.lock().jobs.len(), 1);

        // Calls queued when the worker stops are dropped unrun.
        let (reply, answer) = mpsc::channel::<()>();
        test.shared.call(Box::new(move |_| {
            Box::new(move || {
                let _ = reply.send(());
            }) as Answer
        }));
        test.shared.stop();
        assert_eq!(test.worker.step(), Step::Stopped);
        assert!(answer.recv().is_err());
        let (reply, answer) = mpsc::channel::<()>();
        test.shared.call(Box::new(move |_| {
            Box::new(move || {
                let _ = reply.send(());
            }) as Answer
        }));
        assert!(answer.recv().is_err(), "refused after stopping");
    }

    #[test]
    fn counts_albums_for_progress() {
        let mut jobs = Jobs::default();
        jobs.push(Job::Match(1), Priority::Background, None);
        jobs.push(Job::Match(2), Priority::Background, None);
        assert_eq!(jobs.counts(None), (0, 2));
        let (job, entry) = jobs.take(|_, _| true).unwrap();
        assert_eq!(job, Job::Match(1));
        assert_eq!(jobs.counts(Some(Subject::Album(1))), (0, 2));
        // Its cover follows: still not done.
        jobs.insert(Job::Cover(1), entry, true);
        assert_eq!(jobs.counts(None), (0, 2));
        jobs.take(|_, _| true).unwrap();
        assert_eq!(jobs.counts(None), (1, 2));
        // An artist with the same id is another subject.
        jobs.push(Job::Artist(1), Priority::Background, None);
        assert_eq!(jobs.counts(None), (1, 3));
    }

    #[test]
    fn match_skip_rules() {
        let library = library(&[]);
        let conn = &library.conn;
        let now = unix_now();
        assert!(needs_match(conn, 1, now, false).unwrap(), "never matched");
        let case = |status: &str, chosen_by: &str, checked_at: i64, force: bool| {
            set_link(conn, 1, MB, status, Some("x"), chosen_by, checked_at);
            needs_match(conn, 1, now, force).unwrap()
        };
        assert!(!case("matched", "auto", now - 100 * DAY, true));
        assert!(!case("matched", "user", now, true));
        assert!(!case("none", "user", 0, true), "the user's choice");
        for status in ["none", "review"] {
            assert!(!case(status, "auto", now - DAY, false), "{status} lately");
            assert!(case(status, "auto", now - DAY, true), "{status} asked for");
            assert!(
                case(status, "auto", now - 31 * DAY, false),
                "{status} long ago"
            );
        }
    }

    #[test]
    fn cover_skip_rules() {
        let library = library(&[]);
        let conn = &library.conn;
        let dir = tempfile::tempdir().unwrap();
        let images = ImageCache::new(dir.path().to_path_buf());
        let now = unix_now();
        let needs = |force: bool| needs_cover(conn, &images, 1, now, force).unwrap();
        let release = mbid(1);
        let url = release_front_url(&release);
        assert!(!needs(true), "not matched");
        set_link(conn, 1, MB, "review", Some(&release), "auto", now);
        assert!(!needs(true), "only a candidate");
        set_link(conn, 1, MB, "matched", Some(&release), "user", now);
        assert!(needs(false), "matched by the user");
        set_link(conn, 1, MB, "matched", Some(&release), "auto", now);
        assert!(needs(false));

        // The archive had none for this release lately; for another one, or
        // long ago, doesn't count.
        set_link(conn, 1, CAA, "none", Some(&release), "auto", now - DAY);
        assert!(!needs(false));
        assert!(needs(true));
        set_link(conn, 1, CAA, "none", Some(&mbid(2)), "auto", now - DAY);
        assert!(needs(false));
        set_link(conn, 1, CAA, "none", Some(&release), "auto", now - 31 * DAY);
        assert!(needs(false));

        // Downloaded (maybe for another album of the same release).
        images.store(&url, JPEG).unwrap();
        assert!(!needs(true));

        // The user's pick: from another source, none; from the archive,
        // until it's downloaded, whatever the match.
        let choose = |source: &str, reference: Option<&str>| {
            conn.execute(
                "INSERT OR REPLACE INTO album_art (album_id, source, reference) VALUES (1, ?1, ?2)",
                params![source, reference],
            )
            .unwrap();
        };
        let picked = release_front_url(&mbid(3));
        conn.execute("DELETE FROM album_links", []).unwrap();
        choose(CAA, Some(&picked));
        assert!(needs(false));
        images.store(&picked, JPEG).unwrap();
        assert!(!needs(false));
        set_link(conn, 1, MB, "matched", Some(&mbid(4)), "auto", now);
        choose("folder", Some("Radiohead/In Rainbows/cover.jpg"));
        assert!(!needs(true));
        // An unknown source is ignored.
        choose("lastfm", None);
        assert!(needs(false));
    }

    #[test]
    fn matches_the_library_then_fetches_covers() {
        let mut test = Test::new(library(&["Nothing"]));
        serve_search(&test.transport);
        serve_nothing(&test.transport, "Nothing");
        for (id, _) in RELEASES {
            test.serve_jpeg(&release_front_url(id));
        }
        test.shared.enrich_library();
        assert_eq!(test.run(), Step::Idle);

        let link = test.link(1).unwrap();
        assert_eq!(link.status, LinkStatus::Matched);
        let release = link.external_id.unwrap();
        assert!(test.images.contains(&release_front_url(&release)));
        assert!(!test
            .images
            .contains(&release_group_front_url(RELEASE_GROUP)));
        assert_eq!(test.link(2).unwrap().status, LinkStatus::None);

        let recorded = test.host.take();
        assert_eq!(recorded.art, [1, 1], "matched, then its cover");
        let albums: Vec<i64> = recorded
            .changed
            .iter()
            .flat_map(|changed| changed.albums.clone())
            .collect();
        assert!(albums.contains(&1) && albums.contains(&2), "{albums:?}");
        assert!(recorded
            .changed
            .iter()
            .all(|changed| changed.artists.is_empty()));
        let progress = &recorded.progress;
        assert!(progress.iter().any(|p| p.total == 2
            && matches!(&p.current, Some(Current::Album { title, .. }) if title == "In Rainbows")));
        assert_eq!(progress.last(), Some(&Progress::default()), "idle");
        assert_eq!(test.shared.progress(), Progress::default());

        // Nothing more to do: nothing is requested.
        let requests = test.transport.urls().len();
        test.shared.enrich_library();
        assert_eq!(test.run(), Step::Idle);
        assert_eq!(test.transport.urls().len(), requests);
        let recorded = test.host.take();
        assert!(recorded.changed.is_empty() && recorded.art.is_empty());
    }

    #[test]
    fn records_a_missing_cover_and_retries_it_later() {
        let mut test = Test::new(library(&[]));
        let release = mbid(1);
        set_link(
            test.conn(),
            1,
            MB,
            "matched",
            Some(&release),
            "auto",
            unix_now(),
        );
        test.shared.enrich_library();
        test.run();
        // The fake answers 404 to anything not scripted.
        assert_eq!(test.transport.urls(), [release_front_url(&release)]);
        let check = coverartarchive::cover_check(test.conn(), 1)
            .unwrap()
            .unwrap();
        assert_eq!(check.status, LinkStatus::None);

        test.shared.enrich_library();
        test.run();
        assert_eq!(test.transport.urls().len(), 1, "not asked again");

        // A month on, or when the user asks, it is.
        test.conn()
            .execute(
                "UPDATE album_links SET checked_at = checked_at - ?1",
                [31 * DAY],
            )
            .unwrap();
        test.shared.enrich_library();
        test.run();
        assert_eq!(test.transport.urls().len(), 2);
        let answer = test.request(Job::Cover(1));
        test.run();
        assert!(answer.recv().unwrap().is_ok());
        assert_eq!(test.transport.urls().len(), 3);
    }

    #[test]
    fn user_requests_go_first() {
        let mut test = Test::new(library(&["A", "B"]));
        serve_nothing(&test.transport, "A");
        serve_nothing(&test.transport, "B");
        serve_search(&test.transport);
        test.shared.enrich_library();
        let answer = test.request(Job::Match(3));
        test.shared.request(Job::Match(2), Priority::Playing, None);
        test.run();
        let searched: Vec<String> = test
            .transport
            .urls()
            .into_iter()
            .filter(|url| url.contains("query="))
            .collect();
        assert_eq!(
            searched,
            [
                search_url("B", Some("Nobody"), Some(1)),
                search_url("A", Some("Nobody"), Some(1)),
                search_url("In Rainbows", Some("Radiohead"), Some(10)),
            ]
        );
        assert!(answer.recv().unwrap().is_ok());
    }

    #[test]
    fn a_user_request_waits_for_the_cover() {
        let mut test = Test::new(library(&[]));
        let (id, json) = RELEASES[3];
        test.conn()
            .execute("UPDATE albums SET musicbrainz_release_id = ?1", [id])
            .unwrap();
        test.transport.push_status(&release_url(id), 200, json);
        test.serve_jpeg(&release_front_url(id));
        let answer = test.request(Job::Match(1));
        assert_eq!(test.worker.step(), Step::Ran);
        assert!(answer.try_recv().is_err(), "not before the cover");
        // Each step's changes are sent at once for a user's request, the
        // last before the answer.
        assert_eq!(test.host.take().changed.len(), 1);
        assert_eq!(test.worker.step(), Step::Ran);
        assert!(answer.try_recv().unwrap().is_ok());
        assert_eq!(test.host.take().changed.len(), 1);
    }

    #[test]
    fn pauses_while_a_service_is_unreachable() {
        let mut test = Test::new(library(&["A"]));
        test.transport.push(
            &search_url("In Rainbows", Some("Radiohead"), Some(10)),
            Err(TransportError::Unreachable("no route".into())),
        );
        test.shared.enrich_library();
        let wait = match test.run() {
            Step::Paused(wait) => wait,
            step => panic!("{step:?}"),
        };
        assert_eq!(wait, Duration::from_secs(60));
        assert_eq!(test.transport.urls().len(), 1, "the rest waits");
        let progress = test.shared.progress();
        assert!(progress.paused, "{progress:?}");
        assert_eq!((progress.done, progress.total), (0, 2));
        assert_eq!(progress.unreachable, [musicbrainz::HOST]);

        // A user's request fails at once, without a request, and the album
        // stays queued for the background run.
        let answer = test.request(Job::Match(2));
        assert_eq!(test.worker.step(), Step::Ran);
        let error = answer.recv().unwrap().unwrap_err();
        assert!(matches!(error, Error::Offline(_)), "{error}");
        assert_eq!(test.transport.urls().len(), 1);
        assert!(matches!(test.worker.step(), Step::Paused(_)));
        assert_eq!(test.shared.lock().jobs.len(), 2);

        // Once the wait is over, work resumes.
        test.clock.advance(wait);
        serve_search(&test.transport);
        serve_nothing(&test.transport, "A");
        assert_eq!(test.run(), Step::Idle);
        assert_eq!(test.link(1).unwrap().status, LinkStatus::Matched);
        assert_eq!(test.link(2).unwrap().status, LinkStatus::None);
        assert!(!test.shared.progress().paused);
    }

    #[test]
    fn retry_now_resumes_at_once() {
        let mut test = Test::new(library(&[]));
        test.transport.push(
            &search_url("In Rainbows", Some("Radiohead"), Some(10)),
            Err(TransportError::Unreachable("no route".into())),
        );
        test.shared.enrich_library();
        assert!(matches!(test.run(), Step::Paused(_)));
        serve_search(&test.transport);
        test.shared.retry_now();
        assert_eq!(test.run(), Step::Idle);
        assert_eq!(test.link(1).unwrap().status, LinkStatus::Matched);
    }

    #[test]
    fn covers_go_on_while_musicbrainz_is_unreachable() {
        let mut test = Test::new(library(&["A"]));
        let release = mbid(1);
        set_link(
            test.conn(),
            2,
            MB,
            "matched",
            Some(&release),
            "auto",
            unix_now(),
        );
        test.serve_jpeg(&release_front_url(&release));
        test.transport.push(
            &search_url("In Rainbows", Some("Radiohead"), Some(10)),
            Err(TransportError::Unreachable("no route".into())),
        );
        test.shared.enrich_library();
        assert!(matches!(test.run(), Step::Paused(_)));
        assert!(test.images.contains(&release_front_url(&release)));
    }

    #[test]
    fn batches_changes() {
        let titles: Vec<String> = (1..=8).map(|n| format!("Album {n}")).collect();
        let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
        let mut test = Test::new(library(&titles));
        for album_id in 2..=9 {
            let release = mbid(album_id as u32);
            set_link(
                test.conn(),
                album_id,
                MB,
                "matched",
                Some(&release),
                "auto",
                unix_now(),
            );
            test.serve_jpeg(&release_front_url(&release));
        }
        // "In Rainbows" stays as it is.
        set_link(test.conn(), 1, MB, "none", None, "auto", unix_now());
        test.shared.enrich_library();
        test.run();
        let recorded = test.host.take();
        assert_eq!(recorded.art, (2..=9).collect::<Vec<_>>(), "at once");
        // Covers come 250 ms apart: the first at once, then a batch a second.
        let batches: Vec<Vec<i64>> = recorded.changed.into_iter().map(|c| c.albums).collect();
        assert!(batches.len() < 5, "{batches:?}");
        assert_eq!(batches.concat(), (2..=9).collect::<Vec<_>>());
    }

    #[test]
    fn a_new_match_replaces_the_old_cover() {
        let mut test = Test::new(library(&[]));
        // Matched to one release, with its cover downloaded.
        let (old, old_json) = RELEASES[0];
        test.transport.push_status(&release_url(old), 200, old_json);
        test.conn()
            .execute("UPDATE albums SET musicbrainz_release_id = ?1", [old])
            .unwrap();
        test.serve_jpeg(&release_front_url(old));
        let answer = test.request(Job::Match(1));
        test.run();
        answer.recv().unwrap().unwrap();
        assert!(coverartarchive::cover_urls(test.conn(), 1)
            .unwrap()
            .contains(&release_front_url(old)));
        test.host.take();

        // The tags now name another release, and the match is cleared ("Use
        // automatic" in the 4.6 dialog): the new release's cover replaces
        // the old one, which is no longer among the album's cover URLs.
        let (new, new_json) = RELEASES[3];
        test.conn()
            .execute("UPDATE albums SET musicbrainz_release_id = ?1", [new])
            .unwrap();
        albums::clear_link(test.conn(), 1, SourceId::MusicBrainz).unwrap();
        test.transport.push_status(&release_url(new), 200, new_json);
        test.serve_jpeg(&release_front_url(new));
        let answer = test.request(Job::Match(1));
        test.run();
        answer.recv().unwrap().unwrap();
        let urls = coverartarchive::cover_urls(test.conn(), 1).unwrap();
        assert_eq!(
            urls,
            [
                release_front_url(new),
                release_group_front_url(RELEASE_GROUP)
            ]
        );
        assert!(test.images.contains(&urls[0]));
        let check = coverartarchive::cover_check(test.conn(), 1)
            .unwrap()
            .unwrap();
        assert_eq!(check.external_id.as_deref(), Some(new));
        assert_eq!(test.host.take().art, [1, 1]);
    }

    #[test]
    fn follows_the_settings_when_jobs_run() {
        let mut test = Test::new(library(&[]));
        // Queued while automatic matching was on, run after it was turned
        // off: skipped.
        test.shared.enrich_library();
        test.shared.request(Job::Match(1), Priority::Playing, None);
        test.set_settings(|settings| settings.auto_match = false);
        assert_eq!(test.run(), Step::Idle);
        assert!(test.transport.urls().is_empty());

        // The user can still ask, unless the service is off.
        test.set_settings(|settings| settings.online = false);
        let answer = test.request(Job::Match(1));
        test.run();
        let error = answer.recv().unwrap().unwrap_err();
        assert_eq!(error.to_string(), "Online services are turned off");
        test.set_settings(|settings| {
            settings.online = true;
            settings.sources[3].enabled = false;
        });
        let answer = test.request(Job::Cover(1));
        test.run();
        let error = answer.recv().unwrap().unwrap_err();
        assert_eq!(error.to_string(), "Cover Art Archive is turned off");
        assert!(test.transport.urls().is_empty());

        // An album gone since it was queued.
        let answer = test.request(Job::Match(99));
        test.run();
        assert!(answer.recv().unwrap().is_err());
    }

    #[test]
    fn runs_on_its_thread_until_stopped() {
        let test = Test::new(library(&[]));
        let (shared, transport) = (test.shared.clone(), test.transport.clone());
        set_link(test.conn(), 1, MB, "none", None, "auto", unix_now());
        serve_search(&transport);
        for (id, _) in RELEASES {
            test.transport.push(
                &release_front_url(id),
                Ok(Response {
                    body: JPEG.to_vec(),
                    ..response(200, "")
                }),
            );
        }
        let thread = std::thread::spawn(move || test.worker.run());
        let (reply, answer) = mpsc::channel();
        shared.request(Job::Match(1), Priority::User, Some(reply));
        let result = answer.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(result.is_ok(), "{result:?}");
        assert!(transport.urls().len() > 1);
        shared.stop();
        thread.join().unwrap();

        // Asking a stopped worker gets an answer at once.
        let (reply, answer) = mpsc::channel();
        shared.request(Job::Match(1), Priority::User, Some(reply));
        assert!(answer.recv().is_err());
    }

    fn artist_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM artists WHERE name = ?1", [name], |row| {
            row.get(0)
        })
        .unwrap()
    }

    /// Serves Radiohead's MusicBrainz entry and Wikipedia biography.
    fn serve_radiohead(transport: &FakeTransport) {
        use crate::metadata::musicbrainz::{artist_url, fixtures as mb};
        use crate::metadata::wikipedia::{extract_url, fixtures as wp, sitelink_url};
        transport.push_status(&artist_url(mb::ARTIST), 200, mb::ARTIST_JSON);
        transport.push_status(&sitelink_url(wp::WIKIDATA_ID), 200, wp::SITELINK);
        transport.push_status(&extract_url("Radiohead"), 200, wp::EXTRACT);
    }

    /// Serves the description of "In Rainbows": its release group's links,
    /// and the article.
    fn serve_in_rainbows(transport: &FakeTransport) {
        use crate::metadata::musicbrainz::{fixtures as mb, release_group_url};
        use crate::metadata::wikipedia::{extract_url, fixtures as wp, sitelink_url};
        transport.push_status(
            &release_group_url(mb::RELEASE_GROUP),
            200,
            mb::RELEASE_GROUP_JSON,
        );
        transport.push_status(
            &sitelink_url(wp::ALBUM_WIKIDATA_ID),
            200,
            wp::ALBUM_SITELINK,
        );
        transport.push_status(&extract_url("In Rainbows"), 200, wp::ALBUM_EXTRACT);
    }

    #[test]
    fn enriches_album_artists_after_their_albums() {
        let mut test = Test::with_biographies(library(&[]));
        serve_search(&test.transport);
        for (id, _) in RELEASES {
            test.serve_jpeg(&release_front_url(id));
        }
        serve_in_rainbows(&test.transport);
        serve_radiohead(&test.transport);
        test.shared.enrich_library();
        assert_eq!(test.run(), Step::Idle);

        // Found through the album's release: no artist search.
        let radiohead = artist_id(test.conn(), "Radiohead");
        let urls = test.transport.urls();
        assert!(
            !urls.iter().any(|url| url.contains("/artist/?query=")),
            "{urls:?}"
        );
        let biography = wikipedia::biography(test.conn(), radiohead)
            .unwrap()
            .unwrap();
        assert_eq!(biography.title, "Radiohead");
        assert!(wikipedia::description(test.conn(), 1).unwrap().is_some());
        let recorded = test.host.take();
        let artists: Vec<i64> = recorded
            .changed
            .iter()
            .flat_map(|changed| changed.artists.clone())
            .collect();
        assert!(artists.contains(&radiohead), "{artists:?}");
        assert!(recorded.progress.iter().any(|p| matches!(
            &p.current,
            Some(Current::Artist { name, .. }) if name == "Radiohead"
        )));

        // Nothing more to do.
        let requests = test.transport.urls().len();
        test.shared.enrich_library();
        assert_eq!(test.run(), Step::Idle);
        assert_eq!(test.transport.urls().len(), requests);
    }

    #[test]
    fn no_artists_in_the_background_without_a_biography_source() {
        let mut test = Test::new(library(&[]));
        set_link(test.conn(), 1, MB, "none", None, "auto", unix_now());
        test.shared.enrich_library();
        assert_eq!(test.run(), Step::Idle);
        assert!(test.transport.urls().is_empty());

        // The artist page still looks the artist up.
        let radiohead = artist_id(test.conn(), "Radiohead");
        test.shared
            .request(Job::Artist(radiohead), Priority::Viewing, None);
        test.run();
        assert_eq!(
            test.transport.urls().len(),
            1,
            "{:?}",
            test.transport.urls()
        );
    }

    #[test]
    fn the_artist_page_goes_before_the_album_playing() {
        let mut test = Test::with_biographies(library(&[]));
        let radiohead = artist_id(test.conn(), "Radiohead");
        test.transport.push_status(
            &crate::metadata::musicbrainz::artist_search_url("Radiohead"),
            200,
            r#"{"artists": []}"#,
        );
        test.shared.request(Job::Match(1), Priority::Playing, None);
        test.shared
            .request(Job::Artist(radiohead), Priority::Viewing, None);
        assert_eq!(test.worker.step(), Step::Ran);
        assert!(
            test.transport.urls()[0].contains("/artist/?query="),
            "{:?}",
            test.transport.urls()
        );
        // Its changes are sent at once, like a user's.
        assert_eq!(test.host.take().changed.len(), 1);

        // Automatic: skipped while "match automatically" is off, unlike the
        // user's request, which fails only when the source is off.
        test.run();
        test.conn().execute("DELETE FROM artist_links", []).unwrap();
        test.set_settings(|settings| settings.auto_match = false);
        let requests = test.transport.urls().len();
        test.shared
            .request(Job::Artist(radiohead), Priority::Viewing, None);
        test.run();
        assert_eq!(test.transport.urls().len(), requests);
        test.set_settings(|settings| settings.sources[4].enabled = false);
        let answer = test.request(Job::Biography(radiohead));
        test.run();
        let error = answer.recv().unwrap().unwrap_err();
        assert_eq!(error.to_string(), "Wikipedia is turned off");
        let answer = test.request(Job::Artist(99));
        test.run();
        assert_eq!(
            answer.recv().unwrap().unwrap_err().to_string(),
            "No artist with id 99"
        );
    }

    #[test]
    fn describes_albums_after_their_covers() {
        let mut test = Test::with_biographies(library(&[]));
        serve_search(&test.transport);
        for (id, _) in RELEASES {
            test.serve_jpeg(&release_front_url(id));
        }
        serve_in_rainbows(&test.transport);
        let answer = test.request(Job::Match(1));
        test.run();
        answer.recv().unwrap().unwrap();
        let description = wikipedia::description(test.conn(), 1).unwrap().unwrap();
        assert_eq!(description.title, "In Rainbows");
        // Match, cover, then description, answered once all were done.
        let urls = test.transport.urls();
        let at = |part: &str| urls.iter().position(|url| url.contains(part)).unwrap();
        assert!(at("front-500") < at("/release-group/"), "{urls:?}");
        assert!(at("/release-group/") < at("wikidata.org"), "{urls:?}");
        let changed: Vec<i64> = test
            .host
            .take()
            .changed
            .iter()
            .flat_map(|changed| changed.albums.clone())
            .collect();
        assert!(changed.contains(&1));

        // Asked for again: nothing to do.
        let requests = urls.len();
        let answer = test.request(Job::Description(1));
        test.run();
        answer.recv().unwrap().unwrap();
        assert_eq!(test.transport.urls().len(), requests);

        // A user's request fails while the source is off.
        test.set_settings(|settings| settings.sources[4].enabled = false);
        let answer = test.request(Job::Description(1));
        test.run();
        let error = answer.recv().unwrap().unwrap_err();
        assert_eq!(error.to_string(), "Wikipedia is turned off");
    }

    #[test]
    fn description_skip_rules() {
        use crate::metadata::musicbrainz::{self, fixtures::RELEASE_GROUP};
        let library = library(&[]);
        let conn = &library.conn;
        let now = unix_now();
        let needs = |force: bool| needs_description(conn, 1, now, force).unwrap();
        let release = musicbrainz::parse_release(RELEASES[1].1).unwrap();
        let matched = |status: LinkStatus| {
            albums::store_source_link(
                conn,
                1,
                SourceId::MusicBrainz,
                status,
                Some(&release.id),
                1.0,
                Some(&release),
            )
            .unwrap();
        };
        assert!(!needs(true), "not matched");
        matched(LinkStatus::Review);
        assert!(!needs(true), "only a candidate");
        matched(LinkStatus::Matched);
        assert!(needs(false), "never fetched");
        set_link(
            conn,
            1,
            "wikipedia",
            "matched",
            Some(RELEASE_GROUP),
            "auto",
            now - 100 * DAY,
        );
        assert!(!needs(true), "fetched");
        set_link(
            conn,
            1,
            "wikipedia",
            "none",
            Some(RELEASE_GROUP),
            "auto",
            now - DAY,
        );
        assert!(!needs(false), "not found lately");
        assert!(needs(true));
        set_link(
            conn,
            1,
            "wikipedia",
            "none",
            Some(RELEASE_GROUP),
            "auto",
            now - 31 * DAY,
        );
        assert!(needs(false), "not found long ago");
        set_link(conn, 1, "wikipedia", "matched", Some(&mbid(1)), "auto", now);
        assert!(needs(false), "fetched for another album");
    }

    #[test]
    fn biography_skip_rules() {
        let library = library(&[]);
        let conn = &library.conn;
        let radiohead = artist_id(conn, "Radiohead");
        let now = unix_now();
        let needs = |force: bool| needs_biography(conn, radiohead, now, force).unwrap();
        let link = |source: &str, status: &str, id: Option<&str>, checked_at: i64| {
            conn.execute(
                "INSERT OR REPLACE INTO artist_links
                     (artist_id, source, status, external_id, score, chosen_by, details, checked_at)
                 VALUES (?1, ?2, ?3, ?4, 1.0, 'auto', NULL, ?5)",
                params![radiohead, source, status, id, checked_at],
            )
            .unwrap();
        };
        assert!(!needs(true), "not matched");
        link(MB, "review", Some("a"), now);
        assert!(!needs(true), "only a candidate");
        assert!(!needs_artist_match(conn, radiohead, now, false).unwrap());
        assert!(needs_artist_match(conn, radiohead, now, true).unwrap());
        link(MB, "matched", Some("a"), now);
        assert!(!needs_artist_match(conn, radiohead, now, true).unwrap());
        assert!(needs(false), "never fetched");
        link("wikipedia", "matched", Some("a"), now - 100 * DAY);
        assert!(!needs(true), "fetched");
        link("wikipedia", "none", Some("a"), now - DAY);
        assert!(!needs(false), "not found lately");
        assert!(needs(true));
        link("wikipedia", "none", Some("a"), now - 31 * DAY);
        assert!(needs(false), "not found long ago");
        link("wikipedia", "matched", Some("b"), now);
        assert!(needs(false), "fetched for another match");
    }
}
