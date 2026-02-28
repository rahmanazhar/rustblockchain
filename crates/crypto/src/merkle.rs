use crate::hash::{hash, hash_multiple, Blake3Hash};

/// A simple Merkle tree for computing transaction roots.
#[derive(Debug, Clone)]
pub struct MerkleTree {
    leaves: Vec<Blake3Hash>,
    layers: Vec<Vec<Blake3Hash>>,
}

/// Position of a sibling in a Merkle proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Left,
    Right,
}

/// A Merkle proof for a specific leaf.
#[derive(Debug, Clone)]
pub struct MerkleProof {
    pub leaf: Blake3Hash,
    pub siblings: Vec<(Blake3Hash, Position)>,
}

impl MerkleTree {
    /// Build a Merkle tree from a list of leaf hashes.
    /// If empty, the root is the zero hash.
    pub fn new(leaves: &[Blake3Hash]) -> Self {
        if leaves.is_empty() {
            return Self {
                leaves: vec![],
                layers: vec![vec![Blake3Hash::ZERO]],
            };
        }

        let mut layers = vec![leaves.to_vec()];
        let mut current = leaves.to_vec();

        while current.len() > 1 {
            // If odd number, duplicate the last element
            if !current.len().is_multiple_of(2) {
                current.push(*current.last().unwrap());
            }

            let mut next = Vec::with_capacity(current.len() / 2);
            for chunk in current.chunks(2) {
                let combined = hash_multiple(&[chunk[0].as_ref(), chunk[1].as_ref()]);
                next.push(combined);
            }
            layers.push(next.clone());
            current = next;
        }

        Self {
            leaves: leaves.to_vec(),
            layers,
        }
    }

    /// Get the root hash.
    pub fn root(&self) -> Blake3Hash {
        self.layers
            .last()
            .and_then(|l| l.first())
            .copied()
            .unwrap_or(Blake3Hash::ZERO)
    }

    /// Generate a proof for the leaf at the given index.
    pub fn proof(&self, index: usize) -> Option<MerkleProof> {
        if index >= self.leaves.len() {
            return None;
        }

        let mut siblings = Vec::new();
        let mut idx = index;

        for layer in &self.layers[..self.layers.len() - 1] {
            let sibling_idx = if idx.is_multiple_of(2) { idx + 1 } else { idx - 1 };
            let sibling_idx = sibling_idx.min(layer.len() - 1);
            let position = if idx.is_multiple_of(2) {
                Position::Right
            } else {
                Position::Left
            };
            siblings.push((layer[sibling_idx], position));
            idx /= 2;
        }

        Some(MerkleProof {
            leaf: self.leaves[index],
            siblings,
        })
    }

    /// Verify a Merkle proof against a known root.
    pub fn verify_proof(root: &Blake3Hash, proof: &MerkleProof) -> bool {
        let mut current = proof.leaf;

        for (sibling, position) in &proof.siblings {
            current = match position {
                Position::Left => hash_multiple(&[sibling.as_ref(), current.as_ref()]),
                Position::Right => hash_multiple(&[current.as_ref(), sibling.as_ref()]),
            };
        }

        current == *root
    }

    /// Number of leaves.
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }
}

/// Compute a Merkle root from raw data items.
pub fn compute_merkle_root(items: &[&[u8]]) -> Blake3Hash {
    let leaves: Vec<Blake3Hash> = items.iter().map(|item| hash(item)).collect();
    MerkleTree::new(&leaves).root()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tree() {
        let tree = MerkleTree::new(&[]);
        assert_eq!(tree.root(), Blake3Hash::ZERO);
    }

    #[test]
    fn test_single_leaf() {
        let leaf = hash(b"hello");
        let tree = MerkleTree::new(&[leaf]);
        assert_eq!(tree.root(), leaf);
    }

    #[test]
    fn test_two_leaves() {
        let a = hash(b"a");
        let b = hash(b"b");
        let tree = MerkleTree::new(&[a, b]);
        let expected = hash_multiple(&[a.as_ref(), b.as_ref()]);
        assert_eq!(tree.root(), expected);
    }

    #[test]
    fn test_proof_verification() {
        let leaves: Vec<Blake3Hash> = (0..8u8).map(|i| hash(&[i])).collect();
        let tree = MerkleTree::new(&leaves);
        let root = tree.root();

        for i in 0..leaves.len() {
            let proof = tree.proof(i).unwrap();
            assert!(MerkleTree::verify_proof(&root, &proof));
        }
    }

    #[test]
    fn test_proof_invalid_root() {
        let leaves: Vec<Blake3Hash> = (0..4u8).map(|i| hash(&[i])).collect();
        let tree = MerkleTree::new(&leaves);
        let proof = tree.proof(0).unwrap();
        let wrong_root = hash(b"wrong");
        assert!(!MerkleTree::verify_proof(&wrong_root, &proof));
    }

    #[test]
    fn test_odd_number_of_leaves() {
        let leaves: Vec<Blake3Hash> = (0..5u8).map(|i| hash(&[i])).collect();
        let tree = MerkleTree::new(&leaves);
        let root = tree.root();

        for i in 0..leaves.len() {
            let proof = tree.proof(i).unwrap();
            assert!(MerkleTree::verify_proof(&root, &proof));
        }
    }

    #[test]
    fn test_deterministic() {
        let leaves: Vec<Blake3Hash> = (0..10u8).map(|i| hash(&[i])).collect();
        let tree1 = MerkleTree::new(&leaves);
        let tree2 = MerkleTree::new(&leaves);
        assert_eq!(tree1.root(), tree2.root());
    }
}
