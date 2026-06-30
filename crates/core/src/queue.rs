//! `PlayQueue`: ordered list of track ids with repeat and deterministic shuffle.
//!
//! Pure logic — no I/O, no allocation beyond `Vec`, no time-based RNG.

use crate::{next_index, RepeatMode};

// ---------------------------------------------------------------------------
// Tiny seeded PRNG — SplitMix64 (public-domain, fully deterministic)
// ---------------------------------------------------------------------------

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// Fisher–Yates shuffle driven by the SplitMix64 PRNG.
fn seeded_shuffle(indices: &mut Vec<usize>, seed: u64) {
    let mut rng = SplitMix64(seed);
    let n = indices.len();
    for i in (1..n).rev() {
        let j = (rng.next() as usize) % (i + 1);
        indices.swap(i, j);
    }
}

// ---------------------------------------------------------------------------
// PlayQueue
// ---------------------------------------------------------------------------

/// A play queue over track database ids (`i64`).
///
/// - Linear mode: items are walked in insertion order using `next_index`.
/// - Shuffle mode: an explicit `order` permutation is built once (deterministically,
///   from a seed) and walked; the permutation always starts at position 0 in the
///   shuffled order, not necessarily the item at logical index 0.
pub struct PlayQueue {
    /// The ordered list of track ids.
    items: Vec<i64>,
    /// Current logical position in the *order* slice (shuffled or linear).
    pos: Option<usize>,
    repeat: RepeatMode,
    shuffle: bool,
    /// Permutation of `0..items.len()` used when `shuffle == true`.
    order: Vec<usize>,
    /// Seed used to build `order`.
    shuffle_seed: u64,
}

impl PlayQueue {
    /// Fixed default seed so `set_shuffle(true)` is reproducible in tests.
    const DEFAULT_SEED: u64 = 0xdead_beef_cafe_1234;

    pub fn new() -> Self {
        PlayQueue {
            items: Vec::new(),
            pos: None,
            repeat: RepeatMode::Off,
            shuffle: false,
            order: Vec::new(),
            shuffle_seed: Self::DEFAULT_SEED,
        }
    }

    // ------------------------------------------------------------------
    // Mutation helpers
    // ------------------------------------------------------------------

    fn rebuild_order(&mut self) {
        self.order = (0..self.items.len()).collect();
        if self.shuffle {
            seeded_shuffle(&mut self.order, self.shuffle_seed);
        }
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Replace the queue with a new set of items; position resets to 0
    /// (first item becomes current).
    pub fn set_items(&mut self, ids: Vec<i64>) {
        self.items = ids;
        self.rebuild_order();
        self.pos = if self.items.is_empty() { None } else { Some(0) };
    }

    /// Return the id of the track at the current position, or `None`.
    pub fn current(&self) -> Option<i64> {
        let pos = self.pos?;
        let idx = self.order.get(pos).copied()?;
        self.items.get(idx).copied()
    }

    /// Ids of the tracks that will play after the current one, in actual play
    /// order (shuffle-aware), up to `max`.  Does not wrap.  Used to render the
    /// "Up Next" list so it matches what will really play.
    pub fn upcoming(&self, max: usize) -> Vec<i64> {
        let Some(pos) = self.pos else {
            return Vec::new();
        };
        self.order
            .iter()
            .skip(pos + 1)
            .take(max)
            .filter_map(|&i| self.items.get(i).copied())
            .collect()
    }

    /// Advance to the next track and return its id, or `None` when the
    /// queue is exhausted (respects `repeat`).
    pub fn next(&mut self) -> Option<i64> {
        let pos = self.pos?;
        let len = self.items.len();
        let new_pos = next_index(pos, len, self.repeat)?;
        self.pos = Some(new_pos);
        self.current()
    }

    /// Step back to the previous track and return its id.
    ///
    /// In `RepeatMode::Off` / `RepeatMode::One` clamps at 0.
    /// In `RepeatMode::All` wraps from 0 to the last item.
    pub fn prev(&mut self) -> Option<i64> {
        let pos = self.pos?;
        let len = self.items.len();
        if len == 0 {
            return None;
        }
        let new_pos = if pos == 0 {
            match self.repeat {
                RepeatMode::All => len - 1,
                _ => 0,
            }
        } else {
            pos - 1
        };
        self.pos = Some(new_pos);
        self.current()
    }

    /// Jump to an explicit (linear) index in `items`.
    ///
    /// Finds the position of `index` in the current `order` permutation
    /// and sets `pos` accordingly, so `current()` always returns
    /// `items[index]`.
    pub fn jump_to(&mut self, index: usize) -> Option<i64> {
        if index >= self.items.len() {
            return None;
        }
        // Find where `index` sits in the order permutation.
        let order_pos = self.order.iter().position(|&i| i == index)?;
        self.pos = Some(order_pos);
        self.current()
    }

    /// Set the repeat mode.
    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    /// Enable or disable shuffle using the default fixed seed.
    pub fn set_shuffle(&mut self, on: bool) {
        self.set_shuffle_seeded(on, Self::DEFAULT_SEED);
    }

    /// Enable or disable shuffle with an explicit seed (for tests or
    /// caller-supplied reproducibility).  Rebuilds the order permutation
    /// immediately, preserving the *current track* identity if possible.
    pub fn set_shuffle_seeded(&mut self, on: bool, seed: u64) {
        // Remember which track we are on so we can re-anchor pos.
        let current_idx = self.pos.and_then(|p| self.order.get(p).copied());

        self.shuffle = on;
        self.shuffle_seed = seed;
        self.rebuild_order();

        // Re-anchor pos to the same logical item.
        self.pos = current_idx.and_then(|idx| {
            if idx < self.items.len() {
                self.order.iter().position(|&i| i == idx)
            } else {
                None
            }
        });
        // If we had a non-empty queue but lost pos somehow, reset to 0.
        if self.pos.is_none() && !self.items.is_empty() {
            self.pos = Some(0);
        }
    }

    /// Return the current repeat mode.
    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Return whether shuffle is currently enabled.
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Append a single track id to the end of the queue.
    ///
    /// The order permutation is extended to include the new item.
    /// In both linear and shuffle modes the new index is appended to the tail.
    pub fn append(&mut self, id: i64) {
        let new_idx = self.items.len();
        self.items.push(id);
        self.order.push(new_idx);
        // If the queue was empty, set pos to 0.
        if self.pos.is_none() {
            self.pos = Some(0);
        }
    }

    /// Clear the queue.
    pub fn clear(&mut self) {
        self.items.clear();
        self.order.clear();
        self.pos = None;
    }
}

impl Default for PlayQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests (TDD — written before implementation, verifying spec)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // -----------------------------------------------------------------------
    // 1. Empty queue
    // -----------------------------------------------------------------------

    #[test]
    fn empty_current_is_none() {
        let q = PlayQueue::new();
        assert_eq!(q.current(), None);
    }

    #[test]
    fn upcoming_follows_play_order() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10i64, 20, 30, 40, 50]);
        q.jump_to(0);
        // Linear: upcoming is the next items in insertion order.
        assert_eq!(q.upcoming(2), vec![20, 30]);
        // Shuffled: upcoming follows the shuffle permutation, and must match
        // what next() actually returns.
        q.set_shuffle(true);
        let up = q.upcoming(3);
        let mut walk = Vec::new();
        for _ in 0..3 {
            if let Some(id) = q.next() {
                walk.push(id);
            }
        }
        assert_eq!(up, walk, "upcoming() must match the order next() plays");
    }

    #[test]
    fn shuffle_off_then_next_is_sequential() {
        let items = vec![10i64, 20, 30, 40, 50];
        let mut q = PlayQueue::new();
        q.set_items(items.clone());
        q.set_shuffle(true);
        q.jump_to(0);
        q.next();
        q.next();
        let cur = q.current().unwrap();

        // Turn shuffle OFF — current track must be preserved...
        q.set_shuffle(false);
        assert_eq!(q.current(), Some(cur), "current track changed on shuffle-off");

        // ...and next() must now be the item AFTER `cur` in insertion order.
        let cur_idx = items.iter().position(|&x| x == cur).unwrap();
        let expected = items.get(cur_idx + 1).copied();
        assert_eq!(q.next(), expected, "next after shuffle-off is not sequential");
    }

    #[test]
    fn empty_next_is_none() {
        let mut q = PlayQueue::new();
        assert_eq!(q.next(), None);
    }

    // -----------------------------------------------------------------------
    // 2. set_items → current is the first item
    // -----------------------------------------------------------------------

    #[test]
    fn set_items_current_is_first() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        assert_eq!(q.current(), Some(10));
    }

    // -----------------------------------------------------------------------
    // 3. next() walks the queue in order
    // -----------------------------------------------------------------------

    #[test]
    fn next_walks_linear() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        assert_eq!(q.current(), Some(10));
        assert_eq!(q.next(), Some(20));
        assert_eq!(q.next(), Some(30));
    }

    // -----------------------------------------------------------------------
    // 4. RepeatMode::Off past the end → None
    // -----------------------------------------------------------------------

    #[test]
    fn repeat_off_past_end_is_none() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        q.set_repeat(RepeatMode::Off);
        q.next(); // 20
        q.next(); // 30
        assert_eq!(q.next(), None);
    }

    // -----------------------------------------------------------------------
    // 5. RepeatMode::All wraps around
    // -----------------------------------------------------------------------

    #[test]
    fn repeat_all_wraps_to_first() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        q.set_repeat(RepeatMode::All);
        q.next(); // 20
        q.next(); // 30
        assert_eq!(q.next(), Some(10));
    }

    // -----------------------------------------------------------------------
    // 6. prev()
    // -----------------------------------------------------------------------

    #[test]
    fn prev_goes_back() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        q.next(); // 20
        assert_eq!(q.prev(), Some(10));
    }

    #[test]
    fn prev_at_start_stays_at_first_when_off() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        q.set_repeat(RepeatMode::Off);
        // already at first item
        assert_eq!(q.prev(), Some(10));
    }

    // -----------------------------------------------------------------------
    // 7. jump_to(2) → current is items[2]
    // -----------------------------------------------------------------------

    #[test]
    fn jump_to_positions_correctly() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        q.jump_to(2);
        assert_eq!(q.current(), Some(30));
    }

    // -----------------------------------------------------------------------
    // 8. append(40) is reachable via next()
    // -----------------------------------------------------------------------

    #[test]
    fn append_is_reachable() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        q.append(40);
        // Walk to the end.
        q.next(); // 20
        q.next(); // 30
        assert_eq!(q.next(), Some(40));
    }

    // -----------------------------------------------------------------------
    // 9. Shuffle: visiting all items exactly once, deterministically
    // -----------------------------------------------------------------------

    #[test]
    fn shuffle_visits_all_items_no_repeats() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        // Use a fixed seed so the test is reproducible.
        q.set_shuffle_seeded(true, 42);

        let mut visited: Vec<i64> = Vec::new();
        // current() counts as visiting the first item in the shuffled order.
        if let Some(id) = q.current() {
            visited.push(id);
        }
        // Two more next() calls exhaust the 3-item queue.
        for _ in 0..2 {
            q.set_repeat(RepeatMode::Off);
            if let Some(id) = q.next() {
                visited.push(id);
            }
        }

        // The multiset of visited ids must equal {10, 20, 30}.
        assert_eq!(visited.len(), 3, "should visit exactly 3 items");
        let set: HashSet<i64> = visited.iter().copied().collect();
        assert!(set.contains(&10));
        assert!(set.contains(&20));
        assert!(set.contains(&30));
        // No duplicates.
        assert_eq!(set.len(), 3, "each item visited exactly once");
    }

    // -----------------------------------------------------------------------
    // 10. clear() resets everything
    // -----------------------------------------------------------------------

    #[test]
    fn clear_resets_queue() {
        let mut q = PlayQueue::new();
        q.set_items(vec![10, 20, 30]);
        q.clear();
        assert_eq!(q.current(), None);
        assert_eq!(q.next(), None);
    }
}
