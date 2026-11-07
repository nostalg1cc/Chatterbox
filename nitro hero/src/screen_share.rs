//! The steps of sharing and watching a screen, and the local half of its usage limits.
//! The server holds the real limits; this stops media as soon as they say so.
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// Renew at this rate. The server ends a lease that's quiet for 45 seconds.
const RENEW_INTERVAL: Duration = Duration::from_secs(15);
/// A short network hiccup is fine, but media doesn't keep flowing unconfirmed.
const MAX_FAILED_RENEWALS: u32 = 2;
/// Signals can be lost in a socket reconnect, and the Watch button depends on this one.
const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenSource {
    pub id: String,
    pub kind: String,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Stage {
    #[default]
    Idle,
    Starting,
    Live,
}

/// What the interface shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenStatus {
    pub sources: Vec<ScreenSource>,
    pub share: Stage,
    pub share_title: String,
    pub share_ends: Option<Instant>,
    pub partner_sharing: bool,
    pub watch: Stage,
    pub watch_ends: Option<Instant>,
    /// Counts up with each new message, so each is shown once.
    pub notice_id: u64,
    pub notice: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Effect {
    Engine(Value),
    Signal(Value),
    /// A call to the screen-share function. Its reply comes back through `on_broker`.
    Broker(Op, Value),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    PublishStart,
    Publish,
    ViewStart,
    Renegotiate,
    RenewShare,
    RenewWatch,
    Close,
}

#[derive(Default)]
struct Lease {
    id: String,
    session: String,
    renewed: Option<Instant>,
    failed_renewals: u32,
    renew_pending: bool,
}

#[derive(Default)]
struct Share {
    source_id: String,
    source_kind: String,
    preview_port: u16,
    preview_token: String,
    track: String,
    lease: Lease,
    revision: u64,
    announced: Option<Instant>,
}

#[derive(Default)]
struct Watch {
    port: u16,
    token: String,
    lease: Lease,
}

#[derive(Default)]
pub struct ScreenFlow {
    pub status: ScreenStatus,
    share: Option<Share>,
    watch: Option<Watch>,
    remote: Option<(String, Vec<String>, u64)>,
}

fn revision_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_secs())
}

impl ScreenFlow {
    pub fn sharing(&self) -> bool {
        self.status.share == Stage::Live
    }

    fn note(&mut self, message: impl Into<String>) {
        self.status.notice_id += 1;
        self.status.notice = Some(message.into());
    }

    pub fn start_share(
        &mut self,
        id: String,
        kind: String,
        title: String,
        preview_port: u16,
        preview_token: String,
    ) -> Vec<Effect> {
        if self.share.is_some() {
            return vec![];
        }
        self.share = Some(Share {
            source_id: id,
            source_kind: kind,
            preview_port,
            preview_token,
            track: format!("nitro-screen-{}", uuid::Uuid::new_v4().simple()),
            ..Default::default()
        });
        self.status.share = Stage::Starting;
        self.status.share_title = title;
        vec![Effect::Broker(Op::PublishStart, json!({"action":"publish-start"}))]
    }

    pub fn stop_share(&mut self, notice: Option<&str>) -> Vec<Effect> {
        let Some(share) = self.share.take() else { return vec![] };
        let was_live = self.status.share == Stage::Live;
        self.status.share = Stage::Idle;
        self.status.share_ends = None;
        if let Some(notice) = notice {
            self.note(notice);
        }
        let mut effects = vec![Effect::Engine(json!({"command":"screen-stop"}))];
        if was_live {
            effects.push(Effect::Signal(
                json!({"type":"screen-stopped","screenRevision":share.revision}),
            ));
        }
        if !share.lease.id.is_empty() {
            effects.push(Effect::Broker(Op::Close, json!({"action":"close","leaseId":share.lease.id})));
        }
        effects
    }

    pub fn start_watch(&mut self, port: u16, token: String) -> Vec<Effect> {
        let Some((session, tracks, _)) = &self.remote else {
            self.note("They've stopped sharing.");
            return vec![];
        };
        if self.watch.is_some() {
            return vec![];
        }
        let effect = Effect::Broker(
            Op::ViewStart,
            json!({"action":"view-start","remoteSessionId":session,"trackNames":tracks}),
        );
        self.watch = Some(Watch { port, token, ..Default::default() });
        self.status.watch = Stage::Starting;
        vec![effect]
    }

    pub fn stop_watch(&mut self, notice: Option<&str>) -> Vec<Effect> {
        let Some(watch) = self.watch.take() else { return vec![] };
        self.status.watch = Stage::Idle;
        self.status.watch_ends = None;
        if let Some(notice) = notice {
            self.note(notice);
        }
        let mut effects = vec![Effect::Engine(json!({"command":"screen-view-stop"}))];
        if !watch.lease.id.is_empty() {
            effects.push(Effect::Broker(Op::Close, json!({"action":"close","leaseId":watch.lease.id})));
        }
        effects
    }

    pub fn on_engine(&mut self, event: &Value) -> Vec<Effect> {
        match event["event"].as_str() {
            Some("screen-sources") => {
                self.status.sources = event["sources"]
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .map(|source| ScreenSource {
                                id: source["id"].as_str().unwrap_or("").into(),
                                kind: source["kind"].as_str().unwrap_or("").into(),
                                title: source["title"].as_str().unwrap_or("").into(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                vec![]
            }
            Some("screen-offer") => {
                let Some(share) = &self.share else { return vec![] };
                vec![Effect::Broker(
                    Op::Publish,
                    json!({
                        "action":"publish",
                        "leaseId":share.lease.id,
                        "sessionId":share.lease.session,
                        "mid":event["mid"],
                        "trackName":share.track,
                        "sessionDescription":{"type":"offer","sdp":event["sdp"]},
                    }),
                )]
            }
            Some("screen-state") => match event["state"].as_str() {
                Some("connected") => {
                    let Some(share) = &mut self.share else { return vec![] };
                    if self.status.share == Stage::Live {
                        return vec![];
                    }
                    self.status.share = Stage::Live;
                    share.revision = revision_now();
                    share.announced = Some(Instant::now());
                    vec![Effect::Signal(self.published_signal())]
                }
                Some("failed") => self.stop_share(Some("The screen share lost its connection.")),
                _ => vec![],
            },
            Some("screen-ended") => {
                let notice = if event["reason"] == "source-gone" {
                    "What you were sharing has closed."
                } else {
                    "Screen capture isn't available."
                };
                self.stop_share(Some(notice))
            }
            Some("screen-view-answer") => {
                let Some(watch) = &self.watch else { return vec![] };
                vec![Effect::Broker(
                    Op::Renegotiate,
                    json!({
                        "action":"renegotiate",
                        "leaseId":watch.lease.id,
                        "sessionId":watch.lease.session,
                        "sessionDescription":{"type":"answer","sdp":event["sdp"]},
                    }),
                )]
            }
            Some("screen-first-frame") => {
                if self.watch.is_some() {
                    self.status.watch = Stage::Live;
                }
                vec![]
            }
            Some("screen-view-state") if event["state"] == "failed" => {
                self.stop_watch(Some("Watching lost its connection."))
            }
            Some("screen-view-ended") => self.stop_watch(Some("Couldn't show their screen.")),
            Some("screen-error") => {
                let message = event["message"].as_str().unwrap_or("Screen sharing failed.");
                let mut effects = self.stop_share(Some(message));
                effects.extend(self.stop_watch(Some(message)));
                effects
            }
            _ => vec![],
        }
    }

    fn published_signal(&self) -> Value {
        let share = self.share.as_ref();
        json!({
            "type":"screen-published",
            "cloudflareSessionId":share.map(|share| share.lease.session.clone()),
            "trackNames":share.map(|share| vec![share.track.clone()]),
            "screenRevision":share.map(|share| share.revision),
        })
    }

    /// The partner has just joined or reconnected, and may have missed the announcement.
    pub fn on_partner_ready(&self) -> Vec<Effect> {
        if self.sharing() { vec![Effect::Signal(self.published_signal())] } else { vec![] }
    }

    pub fn on_partner_left(&mut self) -> Vec<Effect> {
        self.remote = None;
        self.status.partner_sharing = false;
        self.stop_watch(None)
    }

    pub fn on_signal(&mut self, signal: &Value) -> Vec<Effect> {
        let revision = signal["screenRevision"].as_u64().unwrap_or(0);
        let current = self.remote.as_ref().map_or(0, |remote| remote.2);
        match signal["type"].as_str() {
            Some("screen-published") if revision >= current => {
                let session = signal["cloudflareSessionId"].as_str();
                let tracks: Option<Vec<String>> = signal["trackNames"].as_array().map(|names| {
                    names.iter().filter_map(|name| name.as_str().map(String::from)).collect()
                });
                if let (Some(session), Some(tracks)) = (session, tracks)
                    && !tracks.is_empty()
                {
                    self.remote = Some((session.to_string(), tracks, revision));
                    self.status.partner_sharing = true;
                }
                vec![]
            }
            Some("screen-stopped") if revision >= current => {
                self.remote = None;
                self.status.partner_sharing = false;
                self.stop_watch(None)
            }
            _ => vec![],
        }
    }

    pub fn on_broker(&mut self, op: Op, result: Result<Value, String>) -> Vec<Effect> {
        match (op, result) {
            (Op::PublishStart, Ok(reply)) => {
                let Some(share) = &mut self.share else {
                    return lease_close(&reply);
                };
                share.lease = lease_from(&reply);
                self.status.share_ends = seconds_left(&reply);
                vec![Effect::Engine(json!({
                    "command":"screen-publish",
                    "id":share.source_id,
                    "kind":share.source_kind,
                    "previewPort":share.preview_port,
                    "previewToken":share.preview_token,
                }))]
            }
            (Op::Publish, Ok(reply)) => {
                if self.share.is_none() {
                    return vec![];
                }
                vec![Effect::Engine(json!({
                    "command":"screen-answer",
                    "sdp":reply["sessionDescription"]["sdp"],
                }))]
            }
            (Op::PublishStart | Op::Publish, Err(message)) => self.stop_share(Some(&message)),
            (Op::ViewStart, Ok(reply)) => {
                let Some(watch) = &mut self.watch else {
                    return lease_close(&reply);
                };
                watch.lease = lease_from(&reply);
                self.status.watch_ends = seconds_left(&reply);
                vec![Effect::Engine(json!({
                    "command":"screen-view",
                    "sdp":reply["sessionDescription"]["sdp"],
                    "port":watch.port,
                    "token":watch.token,
                }))]
            }
            (Op::ViewStart | Op::Renegotiate, Err(message)) => self.stop_watch(Some(&message)),
            (Op::RenewShare, result) => {
                let Some(share) = &mut self.share else { return vec![] };
                if renewal_ended(&mut share.lease, result) {
                    return self.stop_share(Some("The screen share's time allowance ended."));
                }
                vec![]
            }
            (Op::RenewWatch, result) => {
                let Some(watch) = &mut self.watch else { return vec![] };
                if renewal_ended(&mut watch.lease, result) {
                    return self.stop_watch(Some("Watching stopped because it couldn't be renewed."));
                }
                vec![]
            }
            _ => vec![],
        }
    }

    /// Run often. Ends anything past its time and keeps live leases renewed.
    pub fn tick(&mut self) -> Vec<Effect> {
        let now = Instant::now();
        let mut effects = vec![];
        if self.status.share_ends.is_some_and(|ends| now >= ends) {
            effects.extend(self.stop_share(Some("Screen shares stop after 10 minutes.")));
        }
        if self.status.watch_ends.is_some_and(|ends| now >= ends) {
            effects.extend(self.stop_watch(Some("Watching stops after 10 minutes.")));
        }
        if let Some(share) = &mut self.share
            && let Some(body) = renewal_due(&mut share.lease, now)
        {
            effects.push(Effect::Broker(Op::RenewShare, body));
        }
        if let Some(share) = &mut self.share
            && share.announced.is_some_and(|last| now.duration_since(last) >= ANNOUNCE_INTERVAL)
        {
            share.announced = Some(now);
            effects.push(Effect::Signal(self.published_signal()));
        }
        if let Some(watch) = &mut self.watch
            && let Some(body) = renewal_due(&mut watch.lease, now)
        {
            effects.push(Effect::Broker(Op::RenewWatch, body));
        }
        effects
    }

    /// Leases still open, for when the call's engine goes away without a chance to stop properly.
    pub fn open_leases(&self) -> Vec<String> {
        let share = self.share.as_ref().map(|share| &share.lease);
        let watch = self.watch.as_ref().map(|watch| &watch.lease);
        share.into_iter().chain(watch).filter(|lease| !lease.id.is_empty()).map(|lease| lease.id.clone()).collect()
    }

    /// Tells the partner any earlier share of ours is over. Used after an engine restart, which
    /// can't send it itself. Harmless if there wasn't one.
    pub fn ended_signal() -> Value {
        json!({"type":"screen-stopped","screenRevision":revision_now()})
    }

    pub fn shutdown(&mut self) -> Vec<Effect> {
        let mut effects = self.stop_share(None);
        effects.extend(self.stop_watch(None));
        effects
    }
}

fn lease_from(reply: &Value) -> Lease {
    Lease {
        id: reply["leaseId"].as_str().unwrap_or("").into(),
        session: reply["sessionId"].as_str().unwrap_or("").into(),
        renewed: Some(Instant::now()),
        ..Default::default()
    }
}

fn lease_close(reply: &Value) -> Vec<Effect> {
    // It was stopped while the server was still saying yes.
    vec![Effect::Broker(Op::Close, json!({"action":"close","leaseId":reply["leaseId"]}))]
}

fn seconds_left(reply: &Value) -> Option<Instant> {
    reply["secondsLeft"]
        .as_u64()
        .map(|seconds| Instant::now() + Duration::from_secs(seconds))
}

fn renewal_due(lease: &mut Lease, now: Instant) -> Option<Value> {
    if lease.id.is_empty() || lease.renew_pending {
        return None;
    }
    let due = lease.renewed.is_none_or(|last| now.duration_since(last) >= RENEW_INTERVAL);
    if !due {
        return None;
    }
    lease.renew_pending = true;
    Some(json!({"action":"renew","leaseId":lease.id}))
}

/// True if this lease must stop now.
fn renewal_ended(lease: &mut Lease, result: Result<Value, String>) -> bool {
    lease.renew_pending = false;
    lease.renewed = Some(Instant::now());
    match result {
        Ok(reply) if reply["ok"] == true => {
            lease.failed_renewals = 0;
            false
        }
        // The server has answered no, so there's no reason to wait.
        Ok(_) => true,
        Err(_) => {
            lease.failed_renewals += 1;
            lease.failed_renewals >= MAX_FAILED_RENEWALS
        }
    }
}

#[cfg(test)]
mod announce_tests {
    use super::*;

    #[test]
    fn a_live_share_keeps_announcing_itself_until_it_stops() {
        let mut flow = ScreenFlow::default();
        flow.start_share("1".into(), "screen".into(), "Screen 1".into(), 1, "t".into());
        flow.on_engine(&json!({"event":"screen-state","state":"connected"}));
        let announced = |flow: &mut ScreenFlow| {
            flow.tick().iter().filter(|effect| matches!(effect, Effect::Signal(_))).count()
        };
        assert_eq!(announced(&mut flow), 0);
        flow.share.as_mut().unwrap().announced = Some(Instant::now() - ANNOUNCE_INTERVAL);
        assert_eq!(announced(&mut flow), 1);
        assert_eq!(announced(&mut flow), 0);
        flow.stop_share(None);
        assert_eq!(announced(&mut flow), 0);
    }

    #[test]
    fn the_announcement_is_harmless_to_receive_twice() {
        let mut flow = ScreenFlow::default();
        let signal = json!({"type":"screen-published","cloudflareSessionId":"s","trackNames":["t"],"screenRevision":5});
        flow.on_signal(&signal);
        assert!(flow.on_signal(&signal).is_empty());
        assert!(flow.status.partner_sharing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(effect: &Effect) -> &str {
        match effect {
            Effect::Engine(value) => value["command"].as_str().unwrap(),
            _ => "",
        }
    }

    fn live_share() -> ScreenFlow {
        let mut flow = ScreenFlow::default();
        flow.start_share("1".into(), "screen".into(), "Screen 1".into(), 1, "t".into());
        let reply = json!({"leaseId":"lease","sessionId":"session","secondsLeft":600});
        let effects = flow.on_broker(Op::PublishStart, Ok(reply));
        assert_eq!(engine(&effects[0]), "screen-publish");
        flow.on_engine(&json!({"event":"screen-state","state":"connected"}));
        flow
    }

    #[test]
    fn a_refused_share_never_touches_the_screen() {
        let mut flow = ScreenFlow::default();
        let effects = flow.start_share("1".into(), "screen".into(), "Screen 1".into(), 1, "t".into());
        assert!(matches!(effects[0], Effect::Broker(Op::PublishStart, _)));
        let effects = flow.on_broker(Op::PublishStart, Err("Monthly limit reached.".into()));
        assert_eq!(flow.status.share, Stage::Idle);
        assert_eq!(flow.status.notice.as_deref(), Some("Monthly limit reached."));
        // Only the harmless stop, and no publish.
        assert!(effects.iter().all(|effect| engine(effect) != "screen-publish"));
    }

    #[test]
    fn time_limit_stops_the_share_announces_it_and_closes_the_lease() {
        let mut flow = live_share();
        flow.status.share_ends = Some(Instant::now() - Duration::from_secs(1));
        let effects = flow.tick();
        assert_eq!(flow.status.share, Stage::Idle);
        assert_eq!(engine(&effects[0]), "screen-stop");
        assert!(matches!(&effects[1], Effect::Signal(signal) if signal["type"] == "screen-stopped"));
        assert!(matches!(&effects[2], Effect::Broker(Op::Close, body) if body["leaseId"] == "lease"));
    }

    #[test]
    fn a_no_from_the_server_stops_at_once_but_one_network_blip_does_not() {
        let mut flow = live_share();
        flow.on_broker(Op::RenewShare, Err("offline".into()));
        assert_eq!(flow.status.share, Stage::Live);
        flow.on_broker(Op::RenewShare, Ok(json!({"ok":true})));
        flow.on_broker(Op::RenewShare, Err("offline".into()));
        assert_eq!(flow.status.share, Stage::Live);
        flow.on_broker(Op::RenewShare, Err("offline".into()));
        assert_eq!(flow.status.share, Stage::Idle);

        let mut flow = live_share();
        flow.on_broker(Op::RenewShare, Ok(json!({"ok":false})));
        assert_eq!(flow.status.share, Stage::Idle);
    }

    #[test]
    fn watching_needs_a_share_and_ends_with_it() {
        let mut flow = ScreenFlow::default();
        assert!(flow.start_watch(1, "t".into()).is_empty());
        flow.on_signal(&json!({"type":"screen-published","cloudflareSessionId":"s","trackNames":["a"],"screenRevision":5}));
        assert!(flow.status.partner_sharing);
        // Nothing is received or charged until someone chooses to watch.
        let effects = flow.start_watch(1, "t".into());
        assert!(matches!(effects[0], Effect::Broker(Op::ViewStart, _)));
        flow.on_broker(Op::ViewStart, Ok(json!({"leaseId":"l","sessionId":"v","secondsLeft":600,"sessionDescription":{"sdp":"x"}})));
        let effects = flow.on_signal(&json!({"type":"screen-stopped","screenRevision":5}));
        assert_eq!(engine(&effects[0]), "screen-view-stop");
        assert!(matches!(&effects[1], Effect::Broker(Op::Close, body) if body["leaseId"] == "l"));
        assert!(!flow.status.partner_sharing);
    }

    #[test]
    fn stopping_while_the_server_answers_gives_the_lease_back() {
        let mut flow = ScreenFlow::default();
        flow.start_share("1".into(), "screen".into(), "Screen 1".into(), 1, "t".into());
        flow.stop_share(None);
        let effects = flow.on_broker(Op::PublishStart, Ok(json!({"leaseId":"late"})));
        assert!(matches!(&effects[0], Effect::Broker(Op::Close, body) if body["leaseId"] == "late"));
    }

    #[test]
    fn stale_announcements_are_ignored() {
        let mut flow = ScreenFlow::default();
        flow.on_signal(&json!({"type":"screen-published","cloudflareSessionId":"s","trackNames":["a"],"screenRevision":9}));
        flow.on_signal(&json!({"type":"screen-stopped","screenRevision":3}));
        assert!(flow.status.partner_sharing);
    }
}
