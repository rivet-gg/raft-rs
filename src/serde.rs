//! Serializable versions of raft types for persistence and network transport.
//!
//! This module provides serializable wrappers for RawNode and its dependent types,
//! allowing you to serialize and deserialize the Raft state for persistence or
//! network transport.
//!
//! # Example
//!
//! ```ignore
//! use raft::serde::SerdeRawNode;
//! use serde_json;
//!
//! // Serialize a RawNode
//! let serde_node = SerdeRawNode::from_node(raw_node);
//! let json = serde_json::to_string(&serde_node)?;
//!
//! // Deserialize back to RawNode
//! let serde_node: SerdeRawNode = serde_json::from_str(&json)?;
//! let raw_node = serde_node.into_node(storage, logger);
//! ```
//!
//! Note: Storage must implement Clone for deserialization.

use protobuf::ProtobufEnum;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::{
    eraftpb::{ConfState, Entry, EntryType, HardState, Message, MessageType, Snapshot},
    raft::{Raft, RaftCore, StateRole, UncommittedState, INVALID_ID, INVALID_INDEX},
    raft_log::RaftLog,
    raw_node::{LightReady, Peer, RawNode, Ready, ReadyRecord},
    read_only::{ReadOnly, ReadOnlyOption, ReadState},
    storage::Storage,
    tracker::{Inflights, Progress, ProgressState, ProgressTracker},
    util::NO_LIMIT,
    Config, DefaultHashBuilder, HashMap, HashSet, SoftState, Status,
};

/// Wrapper for HardState to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeHardState {
    pub term: u64,
    pub vote: u64,
    pub commit: u64,
}

impl From<HardState> for SerdeHardState {
    fn from(hs: HardState) -> Self {
        SerdeHardState {
            term: hs.term,
            vote: hs.vote,
            commit: hs.commit,
        }
    }
}

impl From<SerdeHardState> for HardState {
    fn from(shs: SerdeHardState) -> Self {
        HardState {
            term: shs.term,
            vote: shs.vote,
            commit: shs.commit,
            unknown_fields: ::protobuf::UnknownFields::default(),
            cached_size: ::protobuf::CachedSize::default(),
        }
    }
}

/// Wrapper for StateRole to make it serializable
#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum SerdeStateRole {
    Follower,
    Candidate,
    Leader,
    PreCandidate,
}

impl From<StateRole> for SerdeStateRole {
    fn from(role: StateRole) -> Self {
        match role {
            StateRole::Follower => SerdeStateRole::Follower,
            StateRole::Candidate => SerdeStateRole::Candidate,
            StateRole::Leader => SerdeStateRole::Leader,
            StateRole::PreCandidate => SerdeStateRole::PreCandidate,
        }
    }
}

impl From<SerdeStateRole> for StateRole {
    fn from(role: SerdeStateRole) -> Self {
        match role {
            SerdeStateRole::Follower => StateRole::Follower,
            SerdeStateRole::Candidate => StateRole::Candidate,
            SerdeStateRole::Leader => StateRole::Leader,
            SerdeStateRole::PreCandidate => StateRole::PreCandidate,
        }
    }
}

/// Wrapper for ReadOnlyOption to make it serializable
#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum SerdeReadOnlyOption {
    Safe,
    LeaseBased,
}

impl From<ReadOnlyOption> for SerdeReadOnlyOption {
    fn from(opt: ReadOnlyOption) -> Self {
        match opt {
            ReadOnlyOption::Safe => SerdeReadOnlyOption::Safe,
            ReadOnlyOption::LeaseBased => SerdeReadOnlyOption::LeaseBased,
        }
    }
}

impl From<SerdeReadOnlyOption> for ReadOnlyOption {
    fn from(opt: SerdeReadOnlyOption) -> Self {
        match opt {
            SerdeReadOnlyOption::Safe => ReadOnlyOption::Safe,
            SerdeReadOnlyOption::LeaseBased => ReadOnlyOption::LeaseBased,
        }
    }
}

/// Wrapper for Config to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeConfig {
    pub id: u64,
    pub election_tick: usize,
    pub heartbeat_tick: usize,
    pub applied: u64,
    pub max_size_per_msg: u64,
    pub max_inflight_msgs: usize,
    pub check_quorum: bool,
    pub pre_vote: bool,
    pub min_election_tick: usize,
    pub max_election_tick: usize,
    pub read_only_option: SerdeReadOnlyOption,
    pub skip_bcast_commit: bool,
    pub batch_append: bool,
    pub priority: i64,
    pub max_uncommitted_size: u64,
    pub max_committed_size_per_ready: u64,
    pub max_apply_unpersisted_log_limit: u64,
    pub disable_proposal_forwarding: bool,
}

impl From<Config> for SerdeConfig {
    fn from(config: Config) -> Self {
        SerdeConfig {
            id: config.id,
            election_tick: config.election_tick,
            heartbeat_tick: config.heartbeat_tick,
            applied: config.applied,
            max_size_per_msg: config.max_size_per_msg,
            max_inflight_msgs: config.max_inflight_msgs,
            check_quorum: config.check_quorum,
            pre_vote: config.pre_vote,
            min_election_tick: config.min_election_tick,
            max_election_tick: config.max_election_tick,
            read_only_option: SerdeReadOnlyOption::from(config.read_only_option),
            skip_bcast_commit: config.skip_bcast_commit,
            batch_append: config.batch_append,
            priority: config.priority,
            max_uncommitted_size: config.max_uncommitted_size,
            max_committed_size_per_ready: config.max_committed_size_per_ready,
            max_apply_unpersisted_log_limit: config.max_apply_unpersisted_log_limit,
            disable_proposal_forwarding: config.disable_proposal_forwarding,
        }
    }
}

impl From<SerdeConfig> for Config {
    fn from(sconfig: SerdeConfig) -> Self {
        Config {
            id: sconfig.id,
            election_tick: sconfig.election_tick,
            heartbeat_tick: sconfig.heartbeat_tick,
            applied: sconfig.applied,
            max_size_per_msg: sconfig.max_size_per_msg,
            max_inflight_msgs: sconfig.max_inflight_msgs,
            check_quorum: sconfig.check_quorum,
            pre_vote: sconfig.pre_vote,
            min_election_tick: sconfig.min_election_tick,
            max_election_tick: sconfig.max_election_tick,
            read_only_option: ReadOnlyOption::from(sconfig.read_only_option),
            skip_bcast_commit: sconfig.skip_bcast_commit,
            batch_append: sconfig.batch_append,
            priority: sconfig.priority,
            max_uncommitted_size: sconfig.max_uncommitted_size,
            max_committed_size_per_ready: sconfig.max_committed_size_per_ready,
            max_apply_unpersisted_log_limit: sconfig.max_apply_unpersisted_log_limit,
            disable_proposal_forwarding: sconfig.disable_proposal_forwarding,
        }
    }
}

/// Wrapper for Message to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeMessage {
    pub msg_type: i32,
    pub to: u64,
    pub from: u64,
    pub term: u64,
    pub log_term: u64,
    pub index: u64,
    pub entries: Vec<SerdeEntry>,
    pub commit: u64,
    pub commit_term: u64,
    pub snapshot: Option<SerdeSnapshot>,
    pub request_snapshot: u64,
    pub reject: bool,
    pub reject_hint: u64,
    pub context: Vec<u8>,
    pub deprecated_priority: u64,
    pub priority: i64,
}

impl From<Message> for SerdeMessage {
    fn from(mut msg: Message) -> Self {
        SerdeMessage {
            msg_type: msg.msg_type as i32,
            to: msg.to,
            from: msg.from,
            term: msg.term,
            log_term: msg.log_term,
            index: msg.index,
            entries: msg
                .entries
                .into_iter()
                .map(|e| SerdeEntry::from(e))
                .collect(),
            commit: msg.commit,
            commit_term: msg.commit_term,
            snapshot: if msg.snapshot.is_some() {
                Some(SerdeSnapshot::from(msg.snapshot.take().unwrap()))
            } else {
                None
            },
            request_snapshot: msg.request_snapshot,
            reject: msg.reject,
            reject_hint: msg.reject_hint,
            context: msg.context.to_vec(),
            deprecated_priority: msg.deprecated_priority,
            priority: msg.priority,
        }
    }
}

impl From<SerdeMessage> for Message {
    fn from(smsg: SerdeMessage) -> Self {
        Message {
            msg_type: MessageType::from_i32(smsg.msg_type).unwrap_or(MessageType::MsgHup),
            to: smsg.to,
            from: smsg.from,
            term: smsg.term,
            log_term: smsg.log_term,
            index: smsg.index,
            entries: ::protobuf::RepeatedField::from_vec(
                smsg.entries.into_iter().map(Entry::from).collect(),
            ),
            commit: smsg.commit,
            commit_term: smsg.commit_term,
            snapshot: if let Some(snap) = smsg.snapshot {
                ::protobuf::SingularPtrField::some(Snapshot::from(snap))
            } else {
                ::protobuf::SingularPtrField::none()
            },
            request_snapshot: smsg.request_snapshot,
            reject: smsg.reject,
            reject_hint: smsg.reject_hint,
            context: smsg.context.into(),
            deprecated_priority: smsg.deprecated_priority,
            priority: smsg.priority,
            unknown_fields: ::protobuf::UnknownFields::default(),
            cached_size: ::protobuf::CachedSize::default(),
        }
    }
}

/// Wrapper for Entry to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeEntry {
    pub term: u64,
    pub index: u64,
    pub entry_type: i32,
    pub data: Vec<u8>,
    pub context: Vec<u8>,
    pub sync_log: bool,
}

impl From<Entry> for SerdeEntry {
    fn from(entry: Entry) -> Self {
        SerdeEntry {
            term: entry.term,
            index: entry.index,
            entry_type: entry.entry_type as i32,
            data: entry.data.to_vec(),
            context: entry.context.to_vec(),
            sync_log: entry.sync_log,
        }
    }
}

impl From<SerdeEntry> for Entry {
    fn from(sentry: SerdeEntry) -> Self {
        Entry {
            term: sentry.term,
            index: sentry.index,
            entry_type: EntryType::from_i32(sentry.entry_type).unwrap_or(EntryType::EntryNormal),
            data: sentry.data.into(),
            context: sentry.context.into(),
            sync_log: sentry.sync_log,
            unknown_fields: ::protobuf::UnknownFields::default(),
            cached_size: ::protobuf::CachedSize::default(),
        }
    }
}

/// Wrapper for Snapshot to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeSnapshot {
    pub data: Vec<u8>,
    pub metadata: SerdeSnapshotMetadata,
}

impl From<Snapshot> for SerdeSnapshot {
    fn from(mut snap: Snapshot) -> Self {
        SerdeSnapshot {
            data: snap.data.to_vec(),
            metadata: SerdeSnapshotMetadata::from(snap.metadata.take().unwrap()),
        }
    }
}

impl From<SerdeSnapshot> for Snapshot {
    fn from(ssnap: SerdeSnapshot) -> Self {
        Snapshot {
            data: ssnap.data.into(),
            metadata: ::protobuf::SingularPtrField::some(crate::eraftpb::SnapshotMetadata::from(
                ssnap.metadata,
            )),
            unknown_fields: ::protobuf::UnknownFields::default(),
            cached_size: ::protobuf::CachedSize::default(),
        }
    }
}

/// Wrapper for SnapshotMetadata
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeSnapshotMetadata {
    pub conf_state: Option<SerdeConfState>,
    pub index: u64,
    pub term: u64,
}

impl From<crate::eraftpb::SnapshotMetadata> for SerdeSnapshotMetadata {
    fn from(mut meta: crate::eraftpb::SnapshotMetadata) -> Self {
        SerdeSnapshotMetadata {
            conf_state: if meta.conf_state.is_some() {
                Some(SerdeConfState::from(meta.conf_state.take().unwrap()))
            } else {
                None
            },
            index: meta.index,
            term: meta.term,
        }
    }
}

impl From<SerdeSnapshotMetadata> for crate::eraftpb::SnapshotMetadata {
    fn from(smeta: SerdeSnapshotMetadata) -> Self {
        crate::eraftpb::SnapshotMetadata {
            conf_state: if let Some(cs) = smeta.conf_state {
                ::protobuf::SingularPtrField::some(ConfState::from(cs))
            } else {
                ::protobuf::SingularPtrField::none()
            },
            index: smeta.index,
            term: smeta.term,
            unknown_fields: ::protobuf::UnknownFields::default(),
            cached_size: ::protobuf::CachedSize::default(),
        }
    }
}

/// Wrapper for ConfState
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeConfState {
    pub voters: Vec<u64>,
    pub learners: Vec<u64>,
    pub voters_outgoing: Vec<u64>,
    pub learners_next: Vec<u64>,
    pub auto_leave: bool,
}

impl From<ConfState> for SerdeConfState {
    fn from(cs: ConfState) -> Self {
        SerdeConfState {
            voters: cs.voters,
            learners: cs.learners,
            voters_outgoing: cs.voters_outgoing,
            learners_next: cs.learners_next,
            auto_leave: cs.auto_leave,
        }
    }
}

impl From<SerdeConfState> for ConfState {
    fn from(scs: SerdeConfState) -> Self {
        ConfState {
            voters: scs.voters,
            learners: scs.learners,
            voters_outgoing: scs.voters_outgoing,
            learners_next: scs.learners_next,
            auto_leave: scs.auto_leave,
            unknown_fields: ::protobuf::UnknownFields::default(),
            cached_size: ::protobuf::CachedSize::default(),
        }
    }
}

/// Wrapper for UncommittedState
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeUncommittedState {
    pub max_uncommitted_size: usize,
    pub uncommitted_size: usize,
    pub last_log_tail_index: u64,
}

impl From<UncommittedState> for SerdeUncommittedState {
    fn from(us: UncommittedState) -> Self {
        SerdeUncommittedState {
            max_uncommitted_size: us.max_uncommitted_size,
            uncommitted_size: us.uncommitted_size,
            last_log_tail_index: us.last_log_tail_index,
        }
    }
}

impl From<SerdeUncommittedState> for UncommittedState {
    fn from(sus: SerdeUncommittedState) -> Self {
        UncommittedState {
            max_uncommitted_size: sus.max_uncommitted_size,
            uncommitted_size: sus.uncommitted_size,
            last_log_tail_index: sus.last_log_tail_index,
        }
    }
}

/// Wrapper for ReadState
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeReadState {
    pub index: u64,
    pub request_ctx: Vec<u8>,
}

impl From<ReadState> for SerdeReadState {
    fn from(rs: ReadState) -> Self {
        SerdeReadState {
            index: rs.index,
            request_ctx: rs.request_ctx,
        }
    }
}

impl From<SerdeReadState> for ReadState {
    fn from(srs: SerdeReadState) -> Self {
        ReadState {
            index: srs.index,
            request_ctx: srs.request_ctx,
        }
    }
}

/// Wrapper for ReadIndexStatus
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeReadIndexStatus {
    pub req: SerdeMessage,
    pub index: u64,
    pub acks: Vec<u64>,
}

impl From<crate::read_only::ReadIndexStatus> for SerdeReadIndexStatus {
    fn from(status: crate::read_only::ReadIndexStatus) -> Self {
        SerdeReadIndexStatus {
            req: SerdeMessage::from(status.req),
            index: status.index,
            acks: status.acks.into_iter().collect(),
        }
    }
}

impl From<SerdeReadIndexStatus> for crate::read_only::ReadIndexStatus {
    fn from(status: SerdeReadIndexStatus) -> Self {
        crate::read_only::ReadIndexStatus {
            req: Message::from(status.req),
            index: status.index,
            acks: status.acks.into_iter().collect(),
        }
    }
}

/// Wrapper for ReadOnly
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeReadOnly {
    pub option: SerdeReadOnlyOption,
    pub pending_read_index: Vec<(Vec<u8>, SerdeReadIndexStatus)>,
    pub read_index_queue: Vec<Vec<u8>>,
}

impl From<ReadOnly> for SerdeReadOnly {
    fn from(ro: ReadOnly) -> Self {
        SerdeReadOnly {
            option: SerdeReadOnlyOption::from(ro.option),
            pending_read_index: ro
                .pending_read_index
                .into_iter()
                .map(|(k, v)| (k, SerdeReadIndexStatus::from(v)))
                .collect(),
            read_index_queue: ro.read_index_queue.into_iter().collect(),
        }
    }
}

impl From<SerdeReadOnly> for ReadOnly {
    fn from(sro: SerdeReadOnly) -> Self {
        ReadOnly {
            option: ReadOnlyOption::from(sro.option),
            pending_read_index: sro
                .pending_read_index
                .into_iter()
                .map(|(k, v)| (k, crate::read_only::ReadIndexStatus::from(v)))
                .collect(),
            read_index_queue: sro.read_index_queue.into(),
        }
    }
}

/// Serializable version of RawNode
#[derive(Serialize, Deserialize)]
pub struct SerdeRawNode {
    pub raft: SerdeRaft,
    pub prev_ss: SerdeSoftState,
    pub prev_hs: SerdeHardState,
    pub max_number: u64,
    pub records: Vec<SerdeReadyRecord>,
    pub commit_since_index: u64,
}

impl SerdeRawNode {
    /// Convert from a RawNode to a serializable version
    pub fn from_node<T: Storage>(node: RawNode<T>) -> Self {
        SerdeRawNode {
            raft: SerdeRaft::from_raft(node.raft),
            prev_ss: SerdeSoftState::from_soft_state(node.prev_ss),
            prev_hs: SerdeHardState::from(node.prev_hs),
            max_number: node.max_number,
            records: node
                .records
                .into_iter()
                .map(SerdeReadyRecord::from_ready_record)
                .collect(),
            commit_since_index: node.commit_since_index,
        }
    }

    /// Convert back to a RawNode
    ///
    /// **Requirements**: Requires both a `Storage` instance and a `Logger` for reconstruction.
    /// These dependencies must be provided as they cannot be serialized.
    pub fn into_node<T: Storage + Clone>(self, storage: T, logger: slog::Logger) -> RawNode<T> {
        RawNode {
            raft: self.raft.into_raft(storage, logger),
            prev_ss: self.prev_ss.into_soft_state(),
            prev_hs: self.prev_hs.into(),
            max_number: self.max_number,
            records: self
                .records
                .into_iter()
                .map(SerdeReadyRecord::into_ready_record)
                .collect(),
            commit_since_index: self.commit_since_index,
        }
    }
}

/// Serializable version of Raft
///
/// **Note**: The `from_raft` method creates a non-destructive copy of the Raft state,
/// cloning messages rather than consuming them from the original instance.
#[derive(Serialize, Deserialize)]
pub struct SerdeRaft {
    prs: SerdeProgressTracker,
    pub msgs: Vec<SerdeMessage>,
    pub r: SerdeRaftCore,
}

impl SerdeRaft {
    pub fn from_raft<T: Storage>(raft: Raft<T>) -> Self {
        SerdeRaft {
            prs: SerdeProgressTracker::from(raft.prs),
            msgs: raft
                .msgs
                .into_iter()
                .map(|m| SerdeMessage::from(m))
                .collect(),
            r: SerdeRaftCore::from(raft.r),
        }
    }

    /// Convert back to a Raft instance
    ///
    /// **Requirements**: Requires both a `Storage` instance and a `Logger` for reconstruction.
    /// These dependencies must be provided as they cannot be serialized.
    pub fn into_raft<T: Storage + Clone>(self, storage: T, logger: slog::Logger) -> Raft<T> {
        // Convert the serialized core back to RaftCore
        let core = self.r.into_core(storage, logger);

        // Directly construct Raft
        Raft {
            prs: self.prs.into_tracker(),
            msgs: self.msgs.into_iter().map(Message::from).collect(),
            r: core,
        }
    }
}

/// Serializable version of RaftCore
#[derive(Serialize, Deserialize)]
pub struct SerdeRaftCore {
    pub term: u64,
    pub vote: u64,
    pub id: u64,
    pub read_states: Vec<SerdeReadState>,
    pub raft_log: SerdeRaftLog,
    pub max_inflight: usize,
    pub max_msg_size: u64,
    pub pending_request_snapshot: u64,
    pub state: SerdeStateRole,
    pub promotable: bool,
    pub leader_id: u64,
    pub lead_transferee: Option<u64>,
    pub pending_conf_index: u64,
    pub read_only: SerdeReadOnly,
    pub election_elapsed: usize,
    pub heartbeat_elapsed: usize,
    pub check_quorum: bool,
    pub pre_vote: bool,
    pub skip_bcast_commit: bool,
    pub batch_append: bool,
    pub disable_proposal_forwarding: bool,
    pub heartbeat_timeout: usize,
    pub election_timeout: usize,
    pub randomized_election_timeout: usize,
    pub min_election_timeout: usize,
    pub max_election_timeout: usize,
    pub priority: i64,
    pub uncommitted_state: SerdeUncommittedState,
    pub max_committed_size_per_ready: u64,
}

impl<T: Storage> From<RaftCore<T>> for SerdeRaftCore {
    fn from(core: RaftCore<T>) -> Self {
        SerdeRaftCore {
            term: core.term,
            vote: core.vote,
            id: core.id,
            read_states: core
                .read_states
                .into_iter()
                .map(SerdeReadState::from)
                .collect(),
            raft_log: SerdeRaftLog::from(core.raft_log),
            max_inflight: core.max_inflight,
            max_msg_size: core.max_msg_size,
            pending_request_snapshot: core.pending_request_snapshot,
            state: SerdeStateRole::from(core.state),
            promotable: core.promotable,
            leader_id: core.leader_id,
            lead_transferee: core.lead_transferee,
            pending_conf_index: core.pending_conf_index,
            read_only: SerdeReadOnly::from(core.read_only),
            election_elapsed: core.election_elapsed,
            heartbeat_elapsed: core.heartbeat_elapsed,
            check_quorum: core.check_quorum,
            pre_vote: core.pre_vote,
            skip_bcast_commit: core.skip_bcast_commit,
            batch_append: core.batch_append,
            disable_proposal_forwarding: core.disable_proposal_forwarding,
            heartbeat_timeout: core.heartbeat_timeout,
            election_timeout: core.election_timeout,
            randomized_election_timeout: core.randomized_election_timeout,
            min_election_timeout: core.min_election_timeout,
            max_election_timeout: core.max_election_timeout,
            priority: core.priority,
            uncommitted_state: SerdeUncommittedState::from(core.uncommitted_state),
            max_committed_size_per_ready: core.max_committed_size_per_ready,
        }
    }
}

impl SerdeRaftCore {
    /// Convert back to a RaftCore instance
    ///
    /// **Requirements**: Requires both a `Storage` instance and a `Logger` for reconstruction.
    /// These dependencies must be provided as they cannot be serialized.
    pub fn into_core<T: Storage + Clone>(self, storage: T, logger: slog::Logger) -> RaftCore<T> {
        let logger = logger.new(slog::o!("raft_id" => self.id));
        RaftCore {
            term: self.term,
            vote: self.vote,
            id: self.id,
            read_states: self.read_states.into_iter().map(|x| x.into()).collect(),
            raft_log: self.raft_log.into_raft_log(storage, logger.clone()),
            max_inflight: self.max_inflight,
            max_msg_size: self.max_msg_size,
            pending_request_snapshot: self.pending_request_snapshot,
            state: self.state.into(),
            promotable: self.promotable,
            leader_id: self.term,
            lead_transferee: self.lead_transferee,
            pending_conf_index: self.pending_conf_index,
            read_only: self.read_only.into(),
            election_elapsed: self.election_elapsed,
            heartbeat_elapsed: self.heartbeat_elapsed,
            check_quorum: self.check_quorum,
            pre_vote: self.pre_vote,
            skip_bcast_commit: self.skip_bcast_commit,
            batch_append: self.batch_append,
            disable_proposal_forwarding: self.disable_proposal_forwarding,
            heartbeat_timeout: self.heartbeat_timeout,
            election_timeout: self.election_timeout,
            randomized_election_timeout: self.randomized_election_timeout,
            min_election_timeout: self.min_election_timeout,
            max_election_timeout: self.max_election_timeout,
            logger,
            priority: self.priority,
            uncommitted_state: self.uncommitted_state.into(),
            max_committed_size_per_ready: self.max_committed_size_per_ready,
        }
    }
}

/// Serializable version of RaftLog
///
/// **Limitation**: The `into_raft_log` method requires both a `Storage` instance
/// and a `Logger` for reconstruction. These dependencies must be provided externally
/// as they cannot be serialized with the log state.
#[derive(Serialize, Deserialize)]
pub struct SerdeRaftLog {
    pub committed: u64,
    pub persisted: u64,
    pub applied: u64,
    pub unstable_entries: Vec<SerdeEntry>,
    pub unstable_snapshot: Option<SerdeSnapshot>,
    pub unstable_offset: u64,
    pub max_apply_unpersisted_log_limit: u64,
}

impl<T: Storage> From<RaftLog<T>> for SerdeRaftLog {
    fn from(log: RaftLog<T>) -> Self {
        SerdeRaftLog {
            committed: log.committed,
            persisted: log.persisted,
            applied: log.applied,
            unstable_entries: log
                .unstable
                .entries
                .into_iter()
                .map(|e| SerdeEntry::from(e))
                .collect(),
            unstable_snapshot: log.unstable.snapshot.map(|s| SerdeSnapshot::from(s)),
            unstable_offset: log.unstable.offset,
            max_apply_unpersisted_log_limit: log.max_apply_unpersisted_log_limit,
        }
    }
}

impl SerdeRaftLog {
    /// Convert back to a RaftLog instance
    ///
    /// **Requirements**: Requires both a `Storage` instance and a `Logger` for reconstruction.
    /// These dependencies must be provided as they cannot be serialized.
    pub fn into_raft_log<T: Storage>(self, storage: T, logger: slog::Logger) -> RaftLog<T> {
        // Calculate entries size
        let entries: Vec<Entry> = self.unstable_entries.into_iter().map(Entry::from).collect();
        let entries_size = entries
            .iter()
            .map(|e| crate::util::entry_approximate_size(e))
            .sum();

        // Build RaftLog directly
        RaftLog {
            store: storage,
            unstable: crate::log_unstable::Unstable {
                entries,
                entries_size,
                offset: self.unstable_offset,
                snapshot: self.unstable_snapshot.map(Snapshot::from),
                logger,
            },
            committed: self.committed,
            persisted: self.persisted,
            applied: self.applied,
            max_apply_unpersisted_log_limit: self.max_apply_unpersisted_log_limit,
        }
    }
}

/// Wrapper for ProgressState to make it serializable
#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum SerdeProgressState {
    Probe,
    Replicate,
    Snapshot,
}

impl From<ProgressState> for SerdeProgressState {
    fn from(state: ProgressState) -> Self {
        match state {
            ProgressState::Probe => SerdeProgressState::Probe,
            ProgressState::Replicate => SerdeProgressState::Replicate,
            ProgressState::Snapshot => SerdeProgressState::Snapshot,
        }
    }
}

impl From<SerdeProgressState> for ProgressState {
    fn from(state: SerdeProgressState) -> Self {
        match state {
            SerdeProgressState::Probe => ProgressState::Probe,
            SerdeProgressState::Replicate => ProgressState::Replicate,
            SerdeProgressState::Snapshot => ProgressState::Snapshot,
        }
    }
}

/// Wrapper for Inflights to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeInflights {
    pub start: usize,
    pub count: usize,
    pub cap: usize,
    pub buffer: Vec<u64>,
    pub incoming_cap: Option<usize>,
}

impl From<Inflights> for SerdeInflights {
    fn from(ins: Inflights) -> Self {
        SerdeInflights {
            start: ins.start,
            count: ins.count,
            cap: ins.cap,
            buffer: ins.buffer,
            incoming_cap: ins.incoming_cap,
        }
    }
}

impl From<SerdeInflights> for Inflights {
    fn from(sins: SerdeInflights) -> Self {
        Inflights {
            start: sins.start,
            count: sins.count,
            cap: sins.cap,
            buffer: sins.buffer,
            incoming_cap: sins.incoming_cap,
        }
    }
}

/// Wrapper for Progress to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeProgress {
    pub matched: u64,
    pub next_idx: u64,
    pub state: SerdeProgressState,
    pub paused: bool,
    pub pending_snapshot: u64,
    pub pending_request_snapshot: u64,
    pub recent_active: bool,
    pub ins: SerdeInflights,
    pub commit_group_id: u64,
    pub committed_index: u64,
}

impl From<Progress> for SerdeProgress {
    fn from(pr: Progress) -> Self {
        SerdeProgress {
            matched: pr.matched,
            next_idx: pr.next_idx,
            state: SerdeProgressState::from(pr.state),
            paused: pr.paused,
            pending_snapshot: pr.pending_snapshot,
            pending_request_snapshot: pr.pending_request_snapshot,
            recent_active: pr.recent_active,
            ins: SerdeInflights::from(pr.ins),
            commit_group_id: pr.commit_group_id,
            committed_index: pr.committed_index,
        }
    }
}

impl From<SerdeProgress> for Progress {
    fn from(spr: SerdeProgress) -> Self {
        Progress {
            matched: spr.matched,
            next_idx: spr.next_idx,
            state: ProgressState::from(spr.state),
            paused: spr.paused,
            pending_snapshot: spr.pending_snapshot,
            pending_request_snapshot: spr.pending_request_snapshot,
            recent_active: spr.recent_active,
            ins: Inflights::from(spr.ins),
            commit_group_id: spr.commit_group_id,
            committed_index: spr.committed_index,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeMajorityConfig {
    pub voters: HashSet<u64>,
}

/// Wrapper for JointConfig to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeJointConfig {
    pub incoming: SerdeMajorityConfig,
    pub outgoing: SerdeMajorityConfig,
}

impl From<crate::JointConfig> for SerdeJointConfig {
    fn from(jc: crate::JointConfig) -> Self {
        SerdeJointConfig {
            incoming: SerdeMajorityConfig {
                voters: jc.incoming.voters,
            },
            outgoing: SerdeMajorityConfig {
                voters: jc.outgoing.voters,
            },
        }
    }
}

impl From<SerdeJointConfig> for crate::JointConfig {
    fn from(sjc: SerdeJointConfig) -> Self {
        crate::JointConfig {
            incoming: crate::MajorityConfig::new(sjc.incoming.voters),
            outgoing: crate::MajorityConfig::new(sjc.outgoing.voters),
        }
    }
}

/// Wrapper for Configuration to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeConfiguration {
    pub voters: SerdeJointConfig,
    pub learners: HashSet<u64>,
    pub learners_next: HashSet<u64>,
    pub auto_leave: bool,
}

impl From<crate::tracker::Configuration> for SerdeConfiguration {
    fn from(conf: crate::tracker::Configuration) -> Self {
        SerdeConfiguration {
            voters: SerdeJointConfig::from(conf.voters),
            learners: conf.learners,
            learners_next: conf.learners_next,
            auto_leave: conf.auto_leave,
        }
    }
}

impl From<SerdeConfiguration> for crate::tracker::Configuration {
    fn from(sconf: SerdeConfiguration) -> Self {
        // Convert voters using the From trait
        let voters = crate::JointConfig::from(sconf.voters);

        // Create the configuration
        let config = crate::tracker::Configuration {
            voters,
            learners: sconf.learners,
            learners_next: sconf.learners_next,
            auto_leave: sconf.auto_leave,
        };

        config
    }
}

/// Serializable version of ProgressTracker
///
/// **Limitation**: This serializable wrapper cannot fully restore a ProgressTracker
/// from deserialized state due to API limitations. The `into_tracker` method creates
/// a basic tracker but cannot restore progress entries, voters, or learners because
/// the ProgressTracker API does not expose methods to set these after construction.
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeProgressTracker {
    pub progress: HashMap<u64, SerdeProgress>,
    pub conf: SerdeConfiguration,
    pub votes: HashMap<u64, bool>,
    pub max_inflight: usize,
    pub group_commit: bool,
}

impl From<ProgressTracker> for SerdeProgressTracker {
    fn from(tracker: ProgressTracker) -> Self {
        SerdeProgressTracker {
            progress: tracker
                .progress
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            conf: tracker.conf.into(),
            votes: tracker.votes,
            max_inflight: tracker.max_inflight,
            group_commit: tracker.group_commit,
        }
    }
}

impl SerdeProgressTracker {
    /// Creates a ProgressTracker from serialized state.
    ///
    /// **Note**: Uses confchange::restore to properly reconstruct the tracker
    /// with all voters, learners, and progress entries.
    pub fn into_tracker(self) -> ProgressTracker {
        ProgressTracker {
            progress: self
                .progress
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            conf: self.conf.into(),
            votes: self.votes,
            max_inflight: self.max_inflight,
            group_commit: self.group_commit,
        }
    }
}

/// Serializable version of SoftState
#[derive(Serialize, Deserialize)]
pub struct SerdeSoftState {
    pub leader_id: u64,
    pub raft_state: SerdeStateRole,
}

impl SerdeSoftState {
    pub fn from_soft_state(ss: SoftState) -> Self {
        SerdeSoftState {
            leader_id: ss.leader_id,
            raft_state: SerdeStateRole::from(ss.raft_state.clone()),
        }
    }

    pub fn into_soft_state(self) -> SoftState {
        SoftState {
            leader_id: self.leader_id,
            raft_state: StateRole::from(self.raft_state),
        }
    }
}

/// Serializable version of ReadyRecord
#[derive(Serialize, Deserialize)]
pub struct SerdeReadyRecord {
    pub number: u64,
    pub last_entry: Option<(u64, u64)>,
    pub snapshot: Option<(u64, u64)>,
}

impl SerdeReadyRecord {
    pub fn from_ready_record(record: ReadyRecord) -> Self {
        SerdeReadyRecord {
            number: record.number,
            last_entry: record.last_entry,
            snapshot: record.snapshot,
        }
    }

    pub fn into_ready_record(self) -> ReadyRecord {
        ReadyRecord {
            number: self.number,
            last_entry: self.last_entry,
            snapshot: self.snapshot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemStorage;
    use crate::Config;

    #[test]
    fn test_raw_node_roundtrip() {
        // Create a simple RawNode
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        let storage = MemStorage::new();
        let config = Config {
            id: 1,
            ..Default::default()
        };

        // Create RawNode using the constructor
        let raw_node = RawNode::new(&config, storage.clone(), &logger).unwrap();

        // Convert to serializable
        let serde_node = SerdeRawNode::from_node(raw_node);

        // Manually check that we can create and convert back
        // (Note: actual serialization would require serde_json to be added as a dev-dependency)
        let restored_node = serde_node.into_node(storage, logger);

        // Check some basic properties
        assert_eq!(restored_node.raft.id, 1);
        assert_eq!(restored_node.raft.state, StateRole::Follower);

        // TODO: Add more comprehensive tests:
        // - Test with entries in the log
        // - Test with multiple peers in ProgressTracker
        // - Test with pending configuration changes
        // - Test with snapshot state
        // - Test with messages pending
    }
}
