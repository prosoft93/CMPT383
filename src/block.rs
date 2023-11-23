use crate::queue::{Task, WorkQueue};
use digest::consts::U32;
use sha2::digest::generic_array::GenericArray;
use sha2::{Digest, Sha256};
use std::fmt::Write;
use std::sync;

pub type Hash = GenericArray<u8, U32>;

#[derive(Debug, Clone)]
pub struct Block {
    pub prev_hash: Hash,
    pub generation: u64,
    pub difficulty: u8,
    pub data: String,
    pub proof: Option<u64>,
}

impl Block {
    pub fn initial(difficulty: u8) -> Block {
        // TODO: create and return a new initial block
        Block {
            prev_hash: Hash::default(),
            generation: 0,
            difficulty,
            data: String::from(""),
            proof: None,
        }
    }

    pub fn next(previous: &Block, data: String) -> Block {
        // TODO: create and return a block that could follow `previous` in the chain
        Block {
            prev_hash: previous.hash(),
            generation: previous.generation + 1,
            difficulty: previous.difficulty,
            data,
            proof: None,
        }
    }

    pub fn hash_string_for_proof(&self, proof: u64) -> String {
        // TODO: return the hash string this block would have if we set the proof to `proof`.
        let prev_hash_str: String = self.prev_hash.iter().map(|byte| format!("{:02x}", byte)).collect();

        // Format the input string
        let mut input = String::new();
        write!(input, "{}:{}:{}:{}:{}", prev_hash_str, self.generation, self.difficulty, self.data, proof).expect("Failed to write to string");

        // Hash the input string
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());

        input
    }

    pub fn hash_string(&self) -> String {
        // self.proof.unwrap() panics if block not mined
        let p = self.proof.unwrap();
        self.hash_string_for_proof(p)
    }

    pub fn hash_for_proof(&self, proof: u64) -> Hash {
        // Format the input string
        let input = format!(
            "{}:{}:{}:{}:{}",
            self.prev_hash.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
            self.generation,
            self.difficulty,
            self.data,
            proof
        );

        // Hash the input string
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());

        // Convert the hashed result to GenericArray<u8, U32>
        let mut hash_array = Hash::default();
        hash_array.copy_from_slice(&hasher.finalize());

        hash_array
    }

    pub fn hash(&self) -> Hash {
        // self.proof.unwrap() panics if block not mined
        let p = self.proof.unwrap();
        self.hash_for_proof(p)
    }

    pub fn set_proof(self: &mut Block, proof: u64) {
        self.proof = Some(proof);
    }

    pub fn hash_satisfies_difficulty(difficulty:u8,hash:Hash) -> bool {
        // TODO: does the hash `hash` have `difficulty` trailing 0s
        let n_bytes = (difficulty / 8) as usize;
        let n_bits = (difficulty % 8) as usize;

        // Check that each of the last n_bytes are 0u8
        if hash.as_slice().iter().rev().take(n_bytes).any(|&byte| byte != 0) {
            return false;
        }

        // Check that the byte one before the last n_bytes is divisible by 1<<n_bits
        if n_bits > 0 && (hash.as_slice()[n_bytes] >> (8 - n_bits)) != 0 {
            return false;
        }

        true
    }

    pub fn is_valid_for_proof(&self, proof: u64) -> bool {
        Self::hash_satisfies_difficulty(self.difficulty,self.hash_for_proof(proof))
    }

    pub fn is_valid(&self) -> bool {
        if self.proof.is_none() {
            return false;
        }
        self.is_valid_for_proof(self.proof.unwrap())
    }

    // Mine in a very simple way: check sequentially until a valid hash is found.
    // This doesn't *need* to be used in any way, but could be used to do some mining
    // before your .mine is complete. Results should be the same as .mine (but slower).
    pub fn mine_serial(self: &mut Block) {
        let mut p = 0u64;
        while !self.is_valid_for_proof(p) {
            p += 1;
        }
        self.proof = Some(p);
    }

    pub fn mine_range(self: &Block, workers: usize, start: u64, end: u64, chunks: u64) -> u64 {
        // TODO: with `workers` threads, check proof values in the given range, breaking up
	// into `chunks` tasks in a work queue. Return the first valid proof found.
        // HINTS:
        // - Create and use a queue::WorkQueue.
        // - Use sync::Arc to wrap a clone of self for sharing.
        let arc_block = sync::Arc::new(self.clone());
        let mut queue = WorkQueue::new(workers);

        // Number of chunks should be at least 1
        let chunks = chunks.max(1);

        // Calculate the number of proofs each chunk should process
        let proofs_per_chunk = (end - start) / chunks;

        for chunk_index in 0..chunks {
            let chunk_start = start + chunk_index * proofs_per_chunk;
            let chunk_end = if chunk_index == chunks - 1 {
                end // Last chunk may have extra proofs
            } else {
                start + (chunk_index + 1) * proofs_per_chunk
            };

            let task = MiningTask {
                block: arc_block.clone(),
                start: chunk_start,
                end: chunk_end,
            };

            let _ = queue.enqueue(task);
        }

        // Accumulate valid proofs
        let mut valid_proof = None;
        for _ in 0..chunks {
            match queue.try_recv() {
                Ok(result) => {
                    valid_proof = Some(result);
                    break;
                }
                Err(_) => {
                    // Channel is empty, workers still processing
                    // Retry after a short delay
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
            }
        }

        // Wait for all workers to finish
        queue.shutdown();
    
        // Return the first valid proof found or end if none
        valid_proof.unwrap_or(end)
    }

    pub fn mine_for_proof(self: &Block, workers: usize) -> u64 {
        let range_start: u64 = 0;
        let range_end: u64 = 8 * (1 << self.difficulty); // 8 * 2^(bits that must be zero)
        let chunks: u64 = 2345;
        self.mine_range(workers, range_start, range_end, chunks)
    }

    pub fn mine(self: &mut Block, workers: usize) {
        self.proof = Some(self.mine_for_proof(workers));
    }
}

struct MiningTask {
    block: sync::Arc<Block>,
    // TODO: more fields as needed
    start: u64,
    end: u64,
}

impl Task for MiningTask {
    type Output = u64;

    fn run(&self) -> Option<u64> {
        // TODO: what does it mean to .run?
        for proof in self.start..self.end {
            if self.block.is_valid_for_proof(proof) {
                return Some(proof);
            }
        }
        None
    }
}