//! The merkle constructions the target networks use.
//!
//! Each target network builds its merkle trees a different way. The module
//! holds one function per construction and names the network that uses it.
//! Every function works on bytes alone, so a unit test runs the function
//! against a recorded fixture with no chain around it.
//!
//! # The three constructions
//!
//! 1. **The prefixed pair tree.** A Cosmos network hashes a leaf behind a
//!    zero byte and an internal node behind a one byte. The split of a
//!    list of leaves falls at the largest power of two below the count.
//!    The prefixes keep a leaf hash apart from an internal node hash.
//! 2. **The sixteen way tree.** A Solana network groups sixteen children
//!    under one parent. A parent hashes the concatenation of the hashes of
//!    its children. A leaf hashes its own bytes.
//! 3. **The doubled pair tree.** Bitcoin hashes each pair twice with the
//!    same function. A level holding an odd count repeats its last entry.

use alloc::vec::Vec;

/// The prefix a Cosmos leaf hash carries.
pub const LEAF_PREFIX: u8 = 0x00;

/// The prefix a Cosmos internal node hash carries.
pub const NODE_PREFIX: u8 = 0x01;

/// The number of children one Solana parent holds.
pub const SOLANA_FANOUT: usize = 16;

/// The hash of one leaf in the prefixed pair tree.
pub fn prefixed_leaf(bytes: &[u8]) -> [u8; 32] {
    let mut held: Vec<u8> = Vec::with_capacity(bytes.len() + 1);
    held.push(LEAF_PREFIX);
    held.extend_from_slice(bytes);
    sp_crypto_hashing::sha2_256(&held)
}

/// The hash of one internal node in the prefixed pair tree.
pub fn prefixed_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut held = [0u8; 65];
    held[0] = NODE_PREFIX;
    held[1..33].copy_from_slice(left);
    held[33..].copy_from_slice(right);
    sp_crypto_hashing::sha2_256(&held)
}

/// The root of a prefixed pair tree over a list of leaves.
///
/// The split of a list falls at the largest power of two below the count.
/// A list of one leaf answers with the leaf hash. An empty list answers
/// with the hash of no bytes.
pub fn prefixed_root(leaves: &[Vec<u8>]) -> [u8; 32] {
    match leaves.len() {
        0 => sp_crypto_hashing::sha2_256(&[]),
        1 => prefixed_leaf(&leaves[0]),
        count => {
            let split = split_point(count);
            let left = prefixed_root(&leaves[..split]);
            let right = prefixed_root(&leaves[split..]);
            prefixed_node(&left, &right)
        }
    }
}

/// The largest power of two below a count.
pub fn split_point(count: usize) -> usize {
    let mut held = 1usize;
    while held.saturating_mul(2) < count {
        held = held.saturating_mul(2);
    }
    held
}

/// The root a prefixed pair branch folds to.
///
/// The branch carries the sibling hashes from the leaf upward. The index
/// gives the position of the leaf among the total count, so the fold knows
/// which side each sibling sits on.
pub fn prefixed_fold(
    leaf: &[u8],
    index: usize,
    total: usize,
    siblings: &[[u8; 32]],
) -> Option<[u8; 32]> {
    if total == 0 || index >= total {
        return None;
    }
    fold_step(prefixed_leaf(leaf), index, total, siblings, 0)
}

/// Fold one level of a prefixed pair branch.
///
/// The sibling list runs from the root level downward, so the walk
/// descends to the leaf first and joins the siblings on the way back up.
fn fold_step(
    current: [u8; 32],
    index: usize,
    total: usize,
    siblings: &[[u8; 32]],
    depth: usize,
) -> Option<[u8; 32]> {
    if total == 1 {
        return if depth == siblings.len() {
            Some(current)
        } else {
            None
        };
    }
    let sibling = *siblings.get(depth)?;
    let split = split_point(total);
    if index < split {
        let inner = fold_step(current, index, split, siblings, depth + 1)?;
        Some(prefixed_node(&inner, &sibling))
    } else {
        let inner = fold_step(current, index - split, total - split, siblings, depth + 1)?;
        Some(prefixed_node(&sibling, &inner))
    }
}

/// Build the sibling list of one leaf in a prefixed pair tree.
///
/// The list runs from the leaf level upward. A fold of the leaf against
/// this list reaches the root of the same tree.
pub fn prefixed_branch(leaves: &[Vec<u8>], index: usize) -> Vec<[u8; 32]> {
    let mut found: Vec<[u8; 32]> = Vec::new();
    collect_branch(leaves, index, &mut found);
    found
}

/// Collect one level of a prefixed pair branch.
fn collect_branch(leaves: &[Vec<u8>], index: usize, into: &mut Vec<[u8; 32]>) {
    if leaves.len() <= 1 {
        return;
    }
    let split = split_point(leaves.len());
    if index < split {
        into.push(prefixed_root(&leaves[split..]));
        collect_branch(&leaves[..split], index, into);
    } else {
        into.push(prefixed_root(&leaves[..split]));
        collect_branch(&leaves[split..], index - split, into);
    }
}

/// The hash of a pair under the doubled pair tree.
pub fn doubled_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut held = [0u8; 64];
    held[..32].copy_from_slice(left);
    held[32..].copy_from_slice(right);
    doubled(&held)
}

/// The doubled hash of one byte string.
pub fn doubled(bytes: &[u8]) -> [u8; 32] {
    sp_crypto_hashing::sha2_256(&sp_crypto_hashing::sha2_256(bytes))
}

/// The root a doubled pair branch folds to.
///
/// The position counts leaves from the left. A bit of the position picks
/// the side of the sibling at each level. Bitcoin builds its transaction
/// tree this way.
pub fn doubled_fold(leaf: &[u8; 32], position: u32, branch: &[[u8; 32]]) -> [u8; 32] {
    let mut current = *leaf;
    let mut index = position;
    for sibling in branch.iter() {
        current = if index & 1 == 0 {
            doubled_pair(&current, sibling)
        } else {
            doubled_pair(sibling, &current)
        };
        index >>= 1;
    }
    current
}

/// The root of a doubled pair tree over a list of leaf hashes.
///
/// A level holding an odd count repeats its last entry to make the pair.
pub fn doubled_root(leaves: &[[u8; 32]]) -> Option<[u8; 32]> {
    if leaves.is_empty() {
        return None;
    }
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        let mut next: Vec<[u8; 32]> = Vec::with_capacity(level.len().div_ceil(2));
        let mut position = 0usize;
        while position < level.len() {
            let left = level[position];
            let right = if position + 1 < level.len() {
                level[position + 1]
            } else {
                left
            };
            next.push(doubled_pair(&left, &right));
            position += 2;
        }
        level = next;
    }
    level.first().copied()
}

/// The hash of one leaf in the sixteen way tree.
pub fn wide_leaf(bytes: &[u8]) -> [u8; 32] {
    sp_crypto_hashing::sha2_256(bytes)
}

/// The hash of one parent in the sixteen way tree.
pub fn wide_node(children: &[[u8; 32]]) -> [u8; 32] {
    let mut held: Vec<u8> = Vec::with_capacity(children.len() * 32);
    for child in children.iter() {
        held.extend_from_slice(child);
    }
    sp_crypto_hashing::sha2_256(&held)
}

/// The root of a sixteen way tree over a list of leaf hashes.
pub fn wide_root(leaves: &[[u8; 32]]) -> Option<[u8; 32]> {
    if leaves.is_empty() {
        return None;
    }
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        let mut next: Vec<[u8; 32]> = Vec::new();
        let mut position = 0usize;
        while position < level.len() {
            let end = (position + SOLANA_FANOUT).min(level.len());
            next.push(wide_node(&level[position..end]));
            position = end;
        }
        level = next;
    }
    level.first().copied()
}

/// One level of a sixteen way branch.
///
/// The entry holds the hashes of every child of the parent and the
/// position the path takes among them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WideStep {
    /// The hashes of the children of the parent, in order.
    pub children: Vec<[u8; 32]>,
    /// The position the path takes among the children.
    pub position: u32,
}

/// The root a sixteen way branch folds to.
///
/// Each step names the children of the parent. The fold checks that the
/// hash it carries sits at the stated position, then hashes the children
/// together and moves up.
pub fn wide_fold(leaf: &[u8; 32], steps: &[WideStep]) -> Option<[u8; 32]> {
    let mut current = *leaf;
    for step in steps.iter() {
        let position = step.position as usize;
        if step.children.is_empty() || position >= step.children.len() {
            return None;
        }
        if step.children.len() > SOLANA_FANOUT {
            return None;
        }
        if step.children[position] != current {
            return None;
        }
        current = wide_node(&step.children);
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn a_split_point_lands_below_the_count() {
        assert_eq!(split_point(2), 1);
        assert_eq!(split_point(3), 2);
        assert_eq!(split_point(4), 2);
        assert_eq!(split_point(5), 4);
        assert_eq!(split_point(9), 8);
    }

    #[test]
    fn a_prefixed_leaf_differs_from_a_prefixed_node() {
        let leaf = prefixed_leaf(&[0u8; 32]);
        let node = prefixed_node(&[0u8; 32], &[0u8; 32]);
        assert_ne!(leaf, node);
    }

    #[test]
    fn a_prefixed_branch_folds_to_the_root() {
        let leaves: Vec<Vec<u8>> = (0u8..5).map(|held| vec![held; 4]).collect();
        let root = prefixed_root(&leaves);
        for index in 0..leaves.len() {
            let siblings = prefixed_branch(&leaves, index);
            let folded = prefixed_fold(&leaves[index], index, leaves.len(), &siblings);
            assert_eq!(folded, Some(root), "the branch at {index} folds");
        }
    }

    #[test]
    fn a_prefixed_branch_of_the_wrong_leaf_fails() {
        let leaves: Vec<Vec<u8>> = (0u8..4).map(|held| vec![held; 4]).collect();
        let root = prefixed_root(&leaves);
        let siblings = prefixed_branch(&leaves, 1);
        let folded = prefixed_fold(&leaves[2], 1, leaves.len(), &siblings);
        assert_ne!(folded, Some(root));
    }

    #[test]
    fn a_doubled_branch_folds_to_the_root() {
        let leaves: Vec<[u8; 32]> = (0u8..4).map(|held| [held; 32]).collect();
        let root = doubled_root(&leaves).expect("the root builds");
        let branch = [leaves[1], doubled_pair(&leaves[2], &leaves[3])];
        assert_eq!(doubled_fold(&leaves[0], 0, &branch), root);
    }

    #[test]
    fn a_doubled_level_repeats_its_last_entry() {
        let leaves: Vec<[u8; 32]> = (0u8..3).map(|held| [held; 32]).collect();
        let root = doubled_root(&leaves).expect("the root builds");
        let expected = doubled_pair(
            &doubled_pair(&leaves[0], &leaves[1]),
            &doubled_pair(&leaves[2], &leaves[2]),
        );
        assert_eq!(root, expected);
    }

    #[test]
    fn a_sixteen_way_branch_folds_to_the_root() {
        let leaves: Vec<[u8; 32]> = (0u8..20).map(|held| wide_leaf(&[held; 4])).collect();
        let root = wide_root(&leaves).expect("the root builds");
        let first = wide_node(&leaves[..16]);
        let second = wide_node(&leaves[16..]);
        let steps = vec![
            WideStep {
                children: leaves[..16].to_vec(),
                position: 3,
            },
            WideStep {
                children: vec![first, second],
                position: 0,
            },
        ];
        assert_eq!(wide_fold(&leaves[3], &steps), Some(root));
    }

    #[test]
    fn a_sixteen_way_branch_naming_the_wrong_position_fails() {
        let leaves: Vec<[u8; 32]> = (0u8..4).map(|held| wide_leaf(&[held; 4])).collect();
        let steps = vec![WideStep {
            children: leaves.clone(),
            position: 1,
        }];
        assert_eq!(wide_fold(&leaves[0], &steps), None);
    }
}
