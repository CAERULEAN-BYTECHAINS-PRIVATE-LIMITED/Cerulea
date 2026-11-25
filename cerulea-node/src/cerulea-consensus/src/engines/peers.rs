//! Peer diversity against the eclipse attack.
//!
//! An eclipse attack isolates a node behind attacker-controlled peers. Every
//! block, every vote and every transaction the node sees then comes from one
//! party. The node keeps following its own rules and still reaches a false view
//! of the network, so authoring from that node harms the chain.
//!
//! Address diversity raises the cost of that isolation. An attacker holding one
//! network range fills a peer set from that range alone. Counting the distinct
//! address prefixes of the peer set turns that into a number the node checks.
//!
//! # What the peer set enforces
//!
//! 1. **A minimum peer count.** A node with too few peers sees too little of
//!    the network.
//! 2. **A minimum count of distinct address prefixes.** Peers from one range
//!    count once. An attacker therefore needs addresses across several ranges.
//! 3. **The validator allow-list on a private chain.** A private chain names
//!    the validators that peer with each other. The registry carries the list.
//!    A peer outside the list never joins the set.
//!
//! A node that fails the first or the second condition is eclipsed. The node
//! halts authoring and raises an alert. Reading stays open throughout. Import
//! stays open throughout. The node resumes authoring as soon as the peer set
//! recovers.
//!
//! # The prefix width
//!
//! An address of the fourth version counts by its leading three bytes. An
//! address of the sixth version counts by its leading six bytes. Those widths
//! are the smallest blocks a single party usually controls, so two peers
//! sharing a prefix usually share an operator.

use std::collections::{BTreeMap, BTreeSet};

use crate::engines::EngineError;

/// The smallest number of peers a node authors with.
pub const DEFAULT_MIN_PEERS: usize = 4;

/// The smallest number of distinct address prefixes a node authors with.
pub const DEFAULT_MIN_PREFIXES: usize = 3;

/// The leading bytes that name the range of an address of the fourth version.
pub const IPV4_PREFIX_BYTES: usize = 3;

/// The leading bytes that name the range of an address of the sixth version.
pub const IPV6_PREFIX_BYTES: usize = 6;

/// One peer address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PeerAddress {
    /// An address of the fourth version.
    V4([u8; 4]),
    /// An address of the sixth version.
    V6([u8; 16]),
}

impl PeerAddress {
    /// The range this address belongs to.
    ///
    /// Two peers sharing a prefix count once toward diversity.
    pub fn prefix(&self) -> AddressPrefix {
        let mut bytes = [0u8; IPV6_PREFIX_BYTES];
        match self {
            PeerAddress::V4(raw) => {
                bytes[..IPV4_PREFIX_BYTES].copy_from_slice(&raw[..IPV4_PREFIX_BYTES]);
                AddressPrefix { family: 4, bytes }
            }
            PeerAddress::V6(raw) => {
                bytes.copy_from_slice(&raw[..IPV6_PREFIX_BYTES]);
                AddressPrefix { family: 6, bytes }
            }
        }
    }

    /// Read one address out of a text form.
    ///
    /// The call reads the four part form of the fourth version and the colon
    /// form of the sixth version. A text form the reader fails to read yields
    /// nothing, and the caller then refuses that peer.
    pub fn parse(text: &str) -> Option<PeerAddress> {
        if let Ok(address) = text.parse::<std::net::Ipv4Addr>() {
            return Some(PeerAddress::V4(address.octets()));
        }
        if let Ok(address) = text.parse::<std::net::Ipv6Addr>() {
            return Some(PeerAddress::V6(address.octets()));
        }
        None
    }
}

/// The range one peer address belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressPrefix {
    /// The address version, four or six.
    pub family: u8,
    /// The leading bytes that name the range.
    pub bytes: [u8; IPV6_PREFIX_BYTES],
}

/// One peer the node holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerRecord {
    /// The identifier the propagation layer gives this peer.
    pub peer_id: String,
    /// The address this peer reaches the node from.
    pub address: PeerAddress,
    /// The validator seal key this peer presents, where it holds a seat.
    pub validator: Option<[u8; 32]>,
}

impl PeerRecord {
    /// Build one peer record.
    pub fn new(peer_id: &str, address: PeerAddress, validator: Option<[u8; 32]>) -> Self {
        PeerRecord {
            peer_id: peer_id.to_string(),
            address,
            validator,
        }
    }
}

/// The diversity a deployment requires of its peer set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiversityPolicy {
    /// The smallest number of peers a node authors with.
    pub min_peers: usize,
    /// The smallest number of distinct address prefixes a node authors with.
    pub min_prefixes: usize,
    /// Whether this deployment is a private chain.
    ///
    /// A private chain admits the validators its registry names and nobody
    /// else.
    pub private_chain: bool,
}

impl Default for DiversityPolicy {
    fn default() -> Self {
        DiversityPolicy::public_chain()
    }
}

impl DiversityPolicy {
    /// The policy a public-access deployment starts from.
    pub const fn public_chain() -> Self {
        DiversityPolicy {
            min_peers: DEFAULT_MIN_PEERS,
            min_prefixes: DEFAULT_MIN_PREFIXES,
            private_chain: false,
        }
    }

    /// The policy a private deployment starts from.
    pub const fn private_chain() -> Self {
        DiversityPolicy {
            min_peers: DEFAULT_MIN_PEERS,
            min_prefixes: DEFAULT_MIN_PREFIXES,
            private_chain: true,
        }
    }

    /// Build a policy holding the stated figures.
    pub const fn new(min_peers: usize, min_prefixes: usize, private_chain: bool) -> Self {
        DiversityPolicy {
            min_peers,
            min_prefixes,
            private_chain,
        }
    }
}

/// Why a node counts as eclipsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EclipseReason {
    /// The peer set holds too few peers.
    TooFewPeers {
        /// The peers the node holds.
        held: usize,
        /// The peers the policy requires.
        needed: usize,
    },
    /// The peer set spans too few address ranges.
    TooFewPrefixes {
        /// The distinct ranges the node holds.
        held: usize,
        /// The distinct ranges the policy requires.
        needed: usize,
    },
}

impl EclipseReason {
    /// One plain sentence stating the problem and the fix.
    pub fn alert(&self) -> String {
        match self {
            EclipseReason::TooFewPeers { held, needed } => format!(
                "This node holds {} peers and authoring needs {}. Block production stopped. \
                 Reading stays open. Fix: add reachable peer addresses and check that the \
                 listening port accepts connections.",
                held, needed
            ),
            EclipseReason::TooFewPrefixes { held, needed } => format!(
                "This node reaches {} distinct address ranges and authoring needs {}. Block \
                 production stopped. Reading stays open. Fix: add peers in other address ranges \
                 so one operator holds fewer of them.",
                held, needed
            ),
        }
    }
}

/// What the peer set says about the health of this node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiversityAssessment {
    /// The peers the node holds.
    pub peers: usize,
    /// The distinct address ranges the node reaches.
    pub prefixes: usize,
    /// The peers holding a validator seat.
    pub validator_peers: usize,
    /// Why the node counts as eclipsed, where it does.
    pub eclipse: Option<EclipseReason>,
}

impl DiversityAssessment {
    /// Report whether this node may author a block.
    ///
    /// An eclipsed node halts authoring. Reading and import stay open.
    pub fn authoring_allowed(&self) -> bool {
        self.eclipse.is_none()
    }

    /// The alert this node raises, where it counts as eclipsed.
    pub fn alert(&self) -> Option<String> {
        self.eclipse.map(|reason| reason.alert())
    }
}

/// The peer set one node holds.
#[derive(Clone, Debug)]
pub struct PeerSet {
    records: BTreeMap<String, PeerRecord>,
    policy: DiversityPolicy,
    allow_list: BTreeSet<[u8; 32]>,
    refusals: u64,
}

impl Default for PeerSet {
    fn default() -> Self {
        PeerSet::new(DiversityPolicy::public_chain())
    }
}

impl PeerSet {
    /// Build an empty peer set under one policy.
    pub fn new(policy: DiversityPolicy) -> Self {
        PeerSet {
            records: BTreeMap::new(),
            policy,
            allow_list: BTreeSet::new(),
            refusals: 0,
        }
    }

    /// The policy this set applies.
    pub fn policy(&self) -> DiversityPolicy {
        self.policy
    }

    /// Replace the validator allow-list the registry publishes.
    ///
    /// A private chain admits the validators this list names and nobody else.
    pub fn set_allow_list(&mut self, keys: impl IntoIterator<Item = [u8; 32]>) {
        self.allow_list = keys.into_iter().collect();
    }

    /// The validator seal keys the allow-list names.
    pub fn allow_list(&self) -> Vec<[u8; 32]> {
        self.allow_list.iter().copied().collect()
    }

    /// The peers this node holds.
    pub fn records(&self) -> Vec<&PeerRecord> {
        self.records.values().collect()
    }

    /// The number of peers this node holds.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Report whether this node holds no peer.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The number of peers this set refused.
    pub fn refusals(&self) -> u64 {
        self.refusals
    }

    /// Admit one peer into the set.
    ///
    /// On a private chain the peer presents a validator seal key the registry
    /// allow-list names. Every other peer is refused with a sentence naming the
    /// fix.
    pub fn admit(&mut self, record: PeerRecord) -> Result<(), EngineError> {
        if self.policy.private_chain {
            let allowed = record
                .validator
                .map(|key| self.allow_list.contains(&key))
                .unwrap_or(false);
            if !allowed {
                self.refusals = self.refusals.saturating_add(1);
                return Err(EngineError::PeerOutsideAllowList {
                    peer: record.peer_id.clone(),
                });
            }
        }
        self.records.insert(record.peer_id.clone(), record);
        Ok(())
    }

    /// Drop one peer from the set.
    pub fn drop_peer(&mut self, peer_id: &str) -> bool {
        self.records.remove(peer_id).is_some()
    }

    /// The distinct address ranges this node reaches.
    pub fn prefixes(&self) -> BTreeSet<AddressPrefix> {
        self.records
            .values()
            .map(|record| record.address.prefix())
            .collect()
    }

    /// The number of distinct address ranges this node reaches.
    pub fn distinct_prefixes(&self) -> usize {
        self.prefixes().len()
    }

    /// The peers holding a validator seat.
    pub fn validator_peers(&self) -> usize {
        self.records
            .values()
            .filter(|record| record.validator.is_some())
            .count()
    }

    /// What this peer set says about the health of the node.
    pub fn assess(&self) -> DiversityAssessment {
        let peers = self.len();
        let prefixes = self.distinct_prefixes();
        let eclipse = if peers < self.policy.min_peers {
            Some(EclipseReason::TooFewPeers {
                held: peers,
                needed: self.policy.min_peers,
            })
        } else if prefixes < self.policy.min_prefixes {
            Some(EclipseReason::TooFewPrefixes {
                held: prefixes,
                needed: self.policy.min_prefixes,
            })
        } else {
            None
        };
        DiversityAssessment {
            peers,
            prefixes,
            validator_peers: self.validator_peers(),
            eclipse,
        }
    }

    /// Report whether this node may author a block.
    pub fn authoring_allowed(&self) -> bool {
        self.assess().authoring_allowed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(name: &str, address: [u8; 4]) -> PeerRecord {
        PeerRecord::new(name, PeerAddress::V4(address), None)
    }

    fn seated(name: &str, address: [u8; 4], key: u8) -> PeerRecord {
        PeerRecord::new(name, PeerAddress::V4(address), Some([key; 32]))
    }

    /// A healthy set holds four peers across four ranges.
    fn healthy_set() -> PeerSet {
        let mut set = PeerSet::new(DiversityPolicy::public_chain());
        for index in 0..4u8 {
            set.admit(peer(&format!("peer-{}", index), [10, 0, index, 5]))
                .expect("a public chain admits every peer");
        }
        set
    }

    #[test]
    fn two_addresses_in_one_range_share_one_prefix() {
        let one = PeerAddress::V4([10, 0, 1, 5]);
        let two = PeerAddress::V4([10, 0, 1, 200]);
        let other = PeerAddress::V4([10, 0, 2, 5]);
        assert_eq!(one.prefix(), two.prefix());
        assert_ne!(one.prefix(), other.prefix());
    }

    #[test]
    fn an_address_of_the_sixth_version_counts_by_six_bytes() {
        let one = PeerAddress::V6([
            0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let two = PeerAddress::V6([
            0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
        ]);
        let other = PeerAddress::V6([
            0x20, 0x01, 0x0d, 0xb8, 0x00, 0x02, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        assert_eq!(one.prefix(), two.prefix());
        assert_ne!(one.prefix(), other.prefix());
        assert_eq!(one.prefix().family, 6);
    }

    #[test]
    fn a_text_address_reads_back_into_a_peer_address() {
        assert_eq!(
            PeerAddress::parse("192.168.4.7"),
            Some(PeerAddress::V4([192, 168, 4, 7]))
        );
        assert!(PeerAddress::parse("2001:db8::1").is_some());
        assert!(PeerAddress::parse("not an address").is_none());
    }

    #[test]
    fn a_healthy_peer_set_authors() {
        let set = healthy_set();
        let assessment = set.assess();
        assert_eq!(assessment.peers, 4);
        assert_eq!(assessment.prefixes, 4);
        assert!(assessment.authoring_allowed());
        assert!(assessment.alert().is_none());
        assert!(set.authoring_allowed());
    }

    /// An eclipsed node halts authoring and raises an alert.
    #[test]
    fn a_node_behind_one_address_range_halts_authoring() {
        let mut set = PeerSet::new(DiversityPolicy::public_chain());
        for index in 0..8u8 {
            set.admit(peer(&format!("peer-{}", index), [10, 0, 1, index]))
                .expect("a public chain admits every peer");
        }
        let assessment = set.assess();
        assert_eq!(assessment.peers, 8);
        assert_eq!(assessment.prefixes, 1);
        assert!(!assessment.authoring_allowed());
        let alert = assessment.alert().expect("the node raises an alert");
        assert!(alert.contains("Fix:"));
        assert!(alert.contains("Reading stays open"));
        assert_eq!(
            assessment.eclipse,
            Some(EclipseReason::TooFewPrefixes {
                held: 1,
                needed: DEFAULT_MIN_PREFIXES
            })
        );
    }

    #[test]
    fn a_node_holding_too_few_peers_halts_authoring() {
        let mut set = PeerSet::new(DiversityPolicy::public_chain());
        set.admit(peer("peer-0", [10, 0, 1, 1]))
            .expect("a public chain admits every peer");
        let assessment = set.assess();
        assert!(!assessment.authoring_allowed());
        assert_eq!(
            assessment.eclipse,
            Some(EclipseReason::TooFewPeers {
                held: 1,
                needed: DEFAULT_MIN_PEERS
            })
        );
        assert!(assessment
            .alert()
            .expect("the node raises an alert")
            .contains("Fix:"));
    }

    #[test]
    fn a_node_resumes_authoring_once_the_peer_set_recovers() {
        let mut set = PeerSet::new(DiversityPolicy::public_chain());
        for index in 0..8u8 {
            set.admit(peer(&format!("peer-{}", index), [10, 0, 1, index]))
                .expect("a public chain admits every peer");
        }
        assert!(!set.authoring_allowed());
        set.admit(peer("peer-a", [10, 0, 2, 1]))
            .expect("a public chain admits every peer");
        set.admit(peer("peer-b", [10, 0, 3, 1]))
            .expect("a public chain admits every peer");
        assert!(set.authoring_allowed());
        assert_eq!(set.distinct_prefixes(), 3);
    }

    #[test]
    fn dropping_peers_returns_a_node_to_the_eclipsed_state() {
        let mut set = healthy_set();
        assert!(set.authoring_allowed());
        assert!(set.drop_peer("peer-0"));
        assert!(!set.authoring_allowed());
        assert!(!set.drop_peer("peer-0"));
    }

    /// A private chain admits the validators the registry names.
    #[test]
    fn a_private_chain_refuses_a_peer_outside_the_allow_list() {
        let mut set = PeerSet::new(DiversityPolicy::private_chain());
        set.set_allow_list([[1u8; 32], [2u8; 32]]);
        set.admit(seated("peer-1", [10, 0, 1, 1], 1))
            .expect("the allow-list names this validator");
        let error = set
            .admit(seated("peer-9", [10, 0, 9, 1], 9))
            .expect_err("the allow-list leaves this validator out");
        assert!(matches!(error, EngineError::PeerOutsideAllowList { .. }));
        assert!(error.to_string().contains("Fix:"));
        assert_eq!(set.len(), 1);
        assert_eq!(set.refusals(), 1);
    }

    #[test]
    fn a_private_chain_refuses_a_peer_presenting_no_validator_key() {
        let mut set = PeerSet::new(DiversityPolicy::private_chain());
        set.set_allow_list([[1u8; 32]]);
        assert!(set.admit(peer("peer-0", [10, 0, 1, 1])).is_err());
        assert!(set.is_empty());
    }

    #[test]
    fn a_private_chain_counts_its_validator_peers() {
        let mut set = PeerSet::new(DiversityPolicy::private_chain());
        set.set_allow_list([[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]]);
        for index in 1..=4u8 {
            set.admit(seated(&format!("peer-{}", index), [10, 0, index, 1], index))
                .expect("the allow-list names this validator");
        }
        let assessment = set.assess();
        assert_eq!(assessment.validator_peers, 4);
        assert!(assessment.authoring_allowed());
        assert_eq!(set.allow_list().len(), 4);
    }

    #[test]
    fn a_public_chain_admits_a_peer_with_no_validator_key() {
        let mut set = PeerSet::new(DiversityPolicy::public_chain());
        assert!(set.admit(peer("peer-0", [10, 0, 1, 1])).is_ok());
        assert_eq!(set.len(), 1);
        assert_eq!(set.refusals(), 0);
    }

    #[test]
    fn one_peer_admitted_twice_counts_once() {
        let mut set = healthy_set();
        let before = set.len();
        set.admit(peer("peer-0", [10, 0, 0, 5]))
            .expect("a public chain admits every peer");
        assert_eq!(set.len(), before);
    }

    #[test]
    fn the_starting_policy_holds_its_stated_figures() {
        let policy = DiversityPolicy::public_chain();
        assert_eq!(policy.min_peers, DEFAULT_MIN_PEERS);
        assert_eq!(policy.min_prefixes, DEFAULT_MIN_PREFIXES);
        assert!(!policy.private_chain);
        assert!(DiversityPolicy::private_chain().private_chain);
        assert_eq!(DiversityPolicy::default(), DiversityPolicy::public_chain());
        let custom = DiversityPolicy::new(9, 5, true);
        assert_eq!(custom.min_peers, 9);
        assert_eq!(custom.min_prefixes, 5);
    }
}
