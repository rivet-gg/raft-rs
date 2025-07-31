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

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use protobuf::ProtobufEnum;

use crate::{
    eraftpb::{ConfState, Entry, EntryType, HardState, Message, MessageType, Snapshot},
    raft::{Raft, RaftCore, StateRole, UncommittedState, INVALID_INDEX, INVALID_ID},
    raw_node::{LightReady, Peer, RawNode, Ready, ReadyRecord},
    read_only::{ReadOnly, ReadOnlyOption, ReadState},
    raft_log::RaftLog,
    storage::Storage,
    tracker::{ProgressTracker, Progress, ProgressState, Inflights},
    util::NO_LIMIT,
    Config, SoftState, Status,
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
            term: hs.get_term(),
            vote: hs.get_vote(),
            commit: hs.get_commit(),
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
            msg_type: msg.get_msg_type() as i32,
            to: msg.get_to(),
            from: msg.get_from(),
            term: msg.get_term(),
            log_term: msg.get_log_term(),
            index: msg.get_index(),
            entries: msg.entries.clone().into_iter().map(|e| SerdeEntry::from(e)).collect(),
            commit: msg.get_commit(),
            commit_term: msg.get_commit_term(),
            snapshot: if msg.has_snapshot() {
                Some(SerdeSnapshot::from(msg.snapshot.take().unwrap()))
            } else {
                None
            },
            request_snapshot: msg.get_request_snapshot(),
            reject: msg.get_reject(),
            reject_hint: msg.get_reject_hint(),
            context: msg.get_context().to_vec(),
            deprecated_priority: msg.get_deprecated_priority(),
            priority: msg.get_priority(),
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
            entries: ::protobuf::RepeatedField::from_vec(smsg.entries.into_iter().map(Entry::from).collect()),
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
            term: entry.get_term(),
            index: entry.get_index(),
            entry_type: entry.get_entry_type() as i32,
            data: entry.get_data().to_vec(),
            context: entry.get_context().to_vec(),
            sync_log: entry.get_sync_log(),
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
            data: snap.get_data().to_vec(),
            metadata: SerdeSnapshotMetadata::from(snap.metadata.take().unwrap()),
        }
    }
}

impl From<SerdeSnapshot> for Snapshot {
    fn from(ssnap: SerdeSnapshot) -> Self {
        Snapshot {
            data: ssnap.data.into(),
            metadata: ::protobuf::SingularPtrField::some(crate::eraftpb::SnapshotMetadata::from(ssnap.metadata)),
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
            conf_state: if meta.has_conf_state() {
                Some(SerdeConfState::from(meta.conf_state.take().unwrap()))
            } else {
                None
            },
            index: meta.get_index(),
            term: meta.get_term(),
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
            voters: cs.get_voters().to_vec(),
            learners: cs.get_learners().to_vec(),
            voters_outgoing: cs.get_voters_outgoing().to_vec(),
            learners_next: cs.get_learners_next().to_vec(),
            auto_leave: cs.get_auto_leave(),
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
            pending_read_index: ro.pending_read_index.into_iter()
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
            pending_read_index: sro.pending_read_index.into_iter()
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
    // State tracking fields
    pub soft_state: SerdeSoftState,
    pub hard_state: SerdeHardState,
    pub prev_ss: SerdeSoftState,
    pub prev_hs: SerdeHardState,
    // Ready tracking fields
    pub max_number: u64,
    pub records: Vec<SerdeReadyRecord>,
    pub commit_since_index: u64,
}

impl SerdeRawNode {
    /// Convert from a RawNode to a serializable version
    pub fn from_node<T: Storage>(node: &RawNode<T>) -> Self {
        let soft_state = node.raft.soft_state();
        let hard_state = node.raft.hard_state();
        SerdeRawNode {
            raft: SerdeRaft::from_raft(&node.raft),
            soft_state: SerdeSoftState::from_soft_state(soft_state),
            hard_state: SerdeHardState::from(hard_state),
            prev_ss: SerdeSoftState::from_soft_state(SoftState {
                leader_id: node.prev_ss.leader_id,
                raft_state: node.prev_ss.raft_state,
            }),
            prev_hs: SerdeHardState::from(node.prev_hs.clone()),
            max_number: node.max_number,
            records: node.records.iter().map(|r| SerdeReadyRecord {
                number: r.number,
                last_entry: r.last_entry,
                snapshot: r.snapshot,
            }).collect(),
            commit_since_index: node.commit_since_index,
        }
    }

    /// Convert back to a RawNode
    /// 
    /// **Requirements**: Requires both a `Storage` instance and a `Logger` for reconstruction.
    /// These dependencies must be provided as they cannot be serialized.
    pub fn into_node<T: Storage + Clone>(self, storage: T, logger: slog::Logger) -> RawNode<T> {
        let raft = self.raft.into_raft(storage.clone(), logger.clone());
        
        // Directly construct RawNode with all fields
        RawNode {
            raft,
            prev_ss: self.prev_ss.into_soft_state(),
            prev_hs: self.prev_hs.into(),
            max_number: self.max_number,
            records: self.records.into_iter().map(SerdeReadyRecord::into_ready_record).collect(),
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
    pub fn from_raft<T: Storage>(raft: &Raft<T>) -> Self {
        SerdeRaft {
            prs: SerdeProgressTracker::from(raft.prs().clone()),
            msgs: raft.msgs.clone().into_iter().map(|m| SerdeMessage::from(m)).collect(),
            r: SerdeRaftCore::from(raft),
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
    pub state: SerdeStateRole,
    pub leader_id: u64,
    pub lead_transferee: Option<u64>,
    pub pending_conf_index: u64,
    pub pending_request_snapshot: u64,
    pub election_elapsed: usize,
    pub heartbeat_elapsed: usize,
    pub check_quorum: bool,
    pub pre_vote: bool,
    pub skip_bcast_commit: bool,
    pub batch_append: bool,
    pub heartbeat_timeout: usize,
    pub election_timeout: usize,
    pub randomized_election_timeout: usize,
    pub min_election_timeout: usize,
    pub max_election_timeout: usize,
    pub config: SerdeConfig,
    pub promote: bool,
    pub max_inflight: usize,
    pub max_msg_size: u64,
    pub priority: i64,
    pub read_only: SerdeReadOnly,
    pub uncommitted_state: SerdeUncommittedState,
    pub max_committed_size_per_ready: u64,
    pub disable_proposal_forwarding: bool,
}

impl<T: Storage> From<&Raft<T>> for SerdeRaftCore {
    fn from(raft: &Raft<T>) -> Self {
        let core = &raft.r;
        SerdeRaftCore {
            term: core.term,
            vote: core.vote,
            id: core.id,
            read_states: core.read_states.clone().into_iter().map(|rs| SerdeReadState::from(rs)).collect(),
            raft_log: SerdeRaftLog::from(&core.raft_log),
            state: SerdeStateRole::from(core.state),
            leader_id: core.leader_id,
            lead_transferee: core.lead_transferee,
            pending_conf_index: core.pending_conf_index,
            pending_request_snapshot: core.pending_request_snapshot,
            election_elapsed: core.election_elapsed,
            heartbeat_elapsed: core.heartbeat_elapsed,
            check_quorum: core.check_quorum,
            pre_vote: core.pre_vote,
            skip_bcast_commit: core.skip_bcast_commit,
            batch_append: core.batch_append,
            heartbeat_timeout: core.heartbeat_timeout,
            election_timeout: core.election_timeout,
            randomized_election_timeout: core.randomized_election_timeout,
            min_election_timeout: core.min_election_timeout,
            max_election_timeout: core.max_election_timeout,
            config: SerdeConfig {
                id: core.id,
                election_tick: core.election_timeout,
                heartbeat_tick: core.heartbeat_timeout,
                applied: core.raft_log.applied,
                max_size_per_msg: core.max_msg_size,
                max_inflight_msgs: core.max_inflight,
                check_quorum: core.check_quorum,
                pre_vote: core.pre_vote,
                min_election_tick: core.min_election_timeout,
                max_election_tick: core.max_election_timeout,
                read_only_option: SerdeReadOnlyOption::from(core.read_only.option),
                skip_bcast_commit: core.skip_bcast_commit,
                batch_append: core.batch_append,
                priority: core.priority,
                max_uncommitted_size: core.uncommitted_state.max_uncommitted_size as u64,
                max_committed_size_per_ready: core.max_committed_size_per_ready,
                max_apply_unpersisted_log_limit: core.raft_log.max_apply_unpersisted_log_limit,
                disable_proposal_forwarding: core.disable_proposal_forwarding,
            },
            promote: raft.promotable(),
            max_inflight: core.max_inflight,
            max_msg_size: core.max_msg_size,
            priority: core.priority,
            read_only: SerdeReadOnly::from(core.read_only.clone()),
            uncommitted_state: SerdeUncommittedState::from(UncommittedState {
                max_uncommitted_size: core.uncommitted_state.max_uncommitted_size,
                uncommitted_size: core.uncommitted_state.uncommitted_size,
                last_log_tail_index: core.uncommitted_state.last_log_tail_index,
            }),
            max_committed_size_per_ready: core.max_committed_size_per_ready,
            disable_proposal_forwarding: core.disable_proposal_forwarding,
        }
    }
}

impl SerdeRaftCore {
    /// Convert back to a RaftCore instance
    /// 
    /// **Requirements**: Requires both a `Storage` instance and a `Logger` for reconstruction.
    /// These dependencies must be provided as they cannot be serialized.
    pub fn into_core<T: Storage + Clone>(self, storage: T, logger: slog::Logger) -> RaftCore<T> {
        let config = Config::from(self.config);
        // Set timeouts in config
        // config.heartbeat_tick = self.heartbeat_timeout;
        // config.election_tick = self.election_timeout;
        // config.min_election_tick = self.min_election_timeout;
        // config.max_election_tick = self.max_election_timeout;
        // config.max_inflight_msgs = self.max_inflight;
        // config.max_size_per_msg = self.max_msg_size;
        // config.pre_vote = self.pre_vote;
        // config.check_quorum = self.check_quorum;
        // config.skip_bcast_commit = self.skip_bcast_commit;
        // config.batch_append = self.batch_append;
        // config.priority = self.priority;
        
        // Build RaftLog using the dedicated method
        let raft_log = self.raft_log.into_raft_log(storage.clone(), logger.clone());
        
        // Build the RaftCore struct directly (copying from raft.rs line 333)
        let raft_core = RaftCore {
            id: self.id,
            read_states: self.read_states.into_iter().map(ReadState::from).collect(),
            raft_log,
            max_inflight: config.max_inflight_msgs,
            max_msg_size: config.max_size_per_msg,
            pending_request_snapshot: self.pending_request_snapshot,
            state: StateRole::from(self.state),
            promotable: self.promote,
            check_quorum: config.check_quorum,
            pre_vote: config.pre_vote,
            read_only: ReadOnly::from(self.read_only),
            heartbeat_timeout: config.heartbeat_tick,
            election_timeout: config.election_tick,
            leader_id: self.leader_id,
            lead_transferee: self.lead_transferee,
            term: self.term,
            election_elapsed: self.election_elapsed,
            pending_conf_index: self.pending_conf_index,
            vote: self.vote,
            heartbeat_elapsed: self.heartbeat_elapsed,
            randomized_election_timeout: self.randomized_election_timeout,
            min_election_timeout: config.min_election_tick(),
            max_election_timeout: config.max_election_tick(),
            skip_bcast_commit: config.skip_bcast_commit,
            batch_append: config.batch_append,
            logger: logger.new(slog::o!("raft_id" => config.id)),
            priority: config.priority,
            uncommitted_state: UncommittedState::from(self.uncommitted_state),
            max_committed_size_per_ready: config.max_committed_size_per_ready,
            disable_proposal_forwarding: config.disable_proposal_forwarding,
        };
        
        // Return the RaftCore
        raft_core
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

impl<T: Storage> From<&RaftLog<T>> for SerdeRaftLog {
    fn from(log: &RaftLog<T>) -> Self {
        SerdeRaftLog {
            committed: log.committed,
            persisted: log.persisted,
            applied: log.applied,
            unstable_entries: log.unstable.entries.clone().into_iter().map(|e| SerdeEntry::from(e)).collect(),
            unstable_snapshot: log.unstable.snapshot.clone().map(|s| SerdeSnapshot::from(s)),
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
        let entries_size = entries.iter().map(|e| crate::util::entry_approximate_size(e)).sum();
        
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

/// Wrapper for JointConfig to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeJointConfig {
    pub incoming: Vec<u64>,
    pub outgoing: Vec<u64>,
}

impl From<crate::JointConfig> for SerdeJointConfig {
    fn from(jc: crate::JointConfig) -> Self {
        SerdeJointConfig {
            incoming: jc.incoming.ids().cloned().collect(),
            outgoing: jc.outgoing.ids().cloned().collect(),
        }
    }
}

impl From<SerdeJointConfig> for crate::JointConfig {
    fn from(sjc: SerdeJointConfig) -> Self {
        let incoming = crate::MajorityConfig::new(sjc.incoming.into_iter().collect());
        let outgoing = crate::MajorityConfig::new(sjc.outgoing.into_iter().collect());
        crate::JointConfig {
            incoming,
            outgoing,
        }
    }
}

/// Wrapper for Configuration to make it serializable
#[derive(Serialize, Deserialize, Clone)]
pub struct SerdeConfiguration {
    pub voters: SerdeJointConfig,
    pub learners: Vec<u64>,
    pub learners_next: Vec<u64>,
    pub auto_leave: bool,
}

impl From<crate::tracker::Configuration> for SerdeConfiguration {
    fn from(conf: crate::tracker::Configuration) -> Self {
        SerdeConfiguration {
            voters: SerdeJointConfig::from(conf.voters().clone()),
            learners: conf.learners().iter().cloned().collect(),
            learners_next: conf.learners_next().iter().cloned().collect(),
            auto_leave: *conf.auto_leave(),
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
            learners: sconf.learners.into_iter().collect(),
            learners_next: sconf.learners_next.into_iter().collect(),
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
    pub progress: std::collections::HashMap<u64, SerdeProgress>,
    pub conf: SerdeConfiguration,
    pub votes: std::collections::HashMap<u64, bool>,
    pub max_inflight: usize,
    pub group_commit: bool,
}

impl From<ProgressTracker> for SerdeProgressTracker {
    fn from(tracker: ProgressTracker) -> Self {
        // We need to extract data before consuming the tracker
        let progress_map: std::collections::HashMap<u64, SerdeProgress> = tracker.iter()
            .map(|(id, pr)| (*id, SerdeProgress::from(pr.clone())))
            .collect();
        let conf = SerdeConfiguration::from(tracker.conf().clone());
        let votes = tracker.votes().iter().map(|(k, v)| (*k, *v)).collect();
        let max_inflight = *tracker.max_inflight();
        let group_commit = tracker.group_commit();
        
        SerdeProgressTracker {
            progress: progress_map,
            conf,
            votes,
            max_inflight,
            group_commit,
        }
    }
}

impl SerdeProgressTracker {

    /// Creates a ProgressTracker from serialized state.
    /// 
    /// **Note**: Uses confchange::restore to properly reconstruct the tracker
    /// with all voters, learners, and progress entries.
    pub fn into_tracker(self) -> ProgressTracker {
        // Use with_capacity for better initialization
        let voters_count = self.conf.voters.incoming.len() + self.conf.voters.outgoing.len();
        let learners_count = self.conf.learners.len();
        let mut tracker = ProgressTracker::with_capacity(voters_count, learners_count, self.max_inflight);
        
        // Set group commit
        tracker.enable_group_commit(self.group_commit);
        
        // Reconstruct the configuration state
        let mut conf_state = ConfState::default();
        conf_state.set_voters(self.conf.voters.incoming.clone());
        conf_state.set_voters_outgoing(self.conf.voters.outgoing.clone());
        conf_state.set_learners(self.conf.learners.clone().into_iter().collect());
        conf_state.set_learners_next(self.conf.learners_next.clone().into_iter().collect());
        conf_state.set_auto_leave(self.conf.auto_leave);
        
        // Find the highest next_idx from progress entries
        let next_idx = self.progress.values()
            .map(|p| p.next_idx)
            .max()
            .unwrap_or(1);
        
        // Use restore to properly set up the tracker with all progress entries
        if let Err(e) = crate::confchange::restore(&mut tracker, next_idx, &conf_state) {
            // Log error but continue - the tracker will at least have basic structure
            // In production, you might want to handle this differently
            eprintln!("Warning: Failed to fully restore ProgressTracker: {:?}", e);
        }
        
        // Restore individual progress states that might not be set by restore
        for (id, sprogress) in self.progress {
            if let Some(progress) = tracker.get_mut(id) {
                // Update progress fields from serialized state
                progress.matched = sprogress.matched;
                progress.next_idx = sprogress.next_idx;
                progress.state = sprogress.state.into();
                progress.paused = sprogress.paused;
                progress.pending_snapshot = sprogress.pending_snapshot;
                progress.pending_request_snapshot = sprogress.pending_request_snapshot;
                progress.recent_active = sprogress.recent_active;
                progress.ins = sprogress.ins.into();
                progress.commit_group_id = sprogress.commit_group_id;
                progress.committed_index = sprogress.committed_index;
            }
        }
        
        // Restore votes
        for (id, vote) in self.votes {
            tracker.record_vote(id, vote);
        }
        
        tracker
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
        let serde_node = SerdeRawNode::from_node(&raw_node);

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
