use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, Vec};

/// Computes the leaf hash committed to by an allowlisted address:
/// sha256 of the address's XDR encoding.
pub fn leaf_hash(env: &Env, address: &Address) -> BytesN<32> {
    let encoded: Bytes = address.clone().to_xdr(env);
    env.crypto().sha256(&encoded).to_bytes()
}

/// Combines two sibling nodes into their parent using sorted-pair sha256
/// hashing, so a proof does not need to encode left/right ordering.
pub(crate) fn hash_pair(env: &Env, a: &BytesN<32>, b: &BytesN<32>) -> BytesN<32> {
    let mut combined = Bytes::new(env);
    if a <= b {
        combined.append(&Bytes::from(a.clone()));
        combined.append(&Bytes::from(b.clone()));
    } else {
        combined.append(&Bytes::from(b.clone()));
        combined.append(&Bytes::from(a.clone()));
    }
    env.crypto().sha256(&combined).to_bytes()
}

/// Verifies that `address` is a member of the Merkle tree committed to by
/// `root`, by hashing `address` into a leaf and folding `proof` up to the
/// root, comparing the result against `root`.
pub fn verify_proof(env: &Env, proof: &Vec<BytesN<32>>, root: &BytesN<32>, address: &Address) -> bool {
    let mut computed = leaf_hash(env, address);
    for sibling in proof.iter() {
        computed = hash_pair(env, &computed, &sibling);
    }
    computed == *root
}
