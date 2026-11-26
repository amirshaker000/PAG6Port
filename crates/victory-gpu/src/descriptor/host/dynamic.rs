//! Reusable contiguous ranges in the dynamic part of a view heap
//! (avatarmanz #47, gomeaw). A BTreeMap of free ranges, neighbours
//! coalesced on release: a run across the search's start is seen (the old
//! ring walk missed it), and no walk of a fragmented heap - a refusal
//! records the size in `too_long` until something is released.

use std::collections::BTreeMap;

pub(super) struct DynamicSlots {
    limit: u32,
    cursor: u32,
    available: u32,
    /// Smallest failed request since the free ranges last grew.
    too_long: u32,
    /// Free ranges by inclusive start, with exclusive ends. Neighbors coalesce.
    ranges: BTreeMap<u32, u32>,
}

impl DynamicSlots {
    pub(super) fn new(limit: u32) -> Self {
        let mut ranges = BTreeMap::new();
        if limit != 0 {
            ranges.insert(0, limit);
        }
        Self {
            limit,
            cursor: 0,
            available: limit,
            too_long: u32::MAX,
            ranges,
        }
    }

    pub(super) fn take(&mut self, count: u32) -> Option<u32> {
        if count == 0 || count > self.available || count >= self.too_long {
            return None;
        }
        let cursor = self.cursor;
        let from_cursor = self
            .ranges
            .range(..=cursor)
            .next_back()
            .and_then(|(&start, &end)| {
                (end.saturating_sub(cursor) >= count).then_some((start, end, cursor))
            });
        let fits = |&(&start, &end): &(&u32, &u32)| end - start >= count;
        let found = from_cursor.or_else(|| {
            let (&start, &end) = self
                .ranges
                .range(cursor..)
                .find(fits)
                .or_else(|| self.ranges.range(..cursor).find(fits))?;
            Some((start, end, start))
        });
        let Some((start, end, first)) = found else {
            self.too_long = count;
            return None;
        };
        self.ranges.remove(&start);
        if start < first {
            self.ranges.insert(start, first);
        }
        let next = first + count;
        if next < end {
            self.ranges.insert(next, end);
        }
        self.available -= count;
        self.cursor = next % self.limit;
        Some(first)
    }

    /// What the state is, for tests.
    #[cfg(test)]
    pub(super) fn debug_state(&self) -> String {
        format!("cursor {} available {} too_long {} ranges {:?}", self.cursor, self.available, self.too_long, self.ranges)
    }

    #[cfg(feature = "diagnostics")]
    pub(super) fn is_taken(&self, slot: u32) -> Option<bool> {
        (slot < self.limit).then(|| {
            self.ranges
                .range(..=slot)
                .next_back()
                .is_none_or(|(_, &end)| end <= slot)
        })
    }

    pub(super) fn release(&mut self, first: u32, count: u32) {
        let end = first.saturating_add(count).min(self.limit);
        if first >= end {
            return;
        }
        let start = self
            .ranges
            .range(..=first)
            .next_back()
            .filter(|&(_, &end)| end >= first)
            .map_or(first, |(&start, _)| start);
        let mut merged_end = end;
        let mut already_free = 0;
        while let Some((&start, &end)) = self.ranges.range(start..=merged_end).next() {
            self.ranges.remove(&start);
            merged_end = merged_end.max(end);
            already_free += end - start;
        }
        let given = merged_end - start - already_free;
        self.ranges.insert(start, merged_end);
        self.available += given;
        // A release that only joins free ranges the slot had already given
        // back (given == 0) still makes a bigger run than either was: a
        // refusal recorded before it (`too_long`) would refuse a take the
        // merged run fits until something really is taken again.
        if given != 0 || merged_end - start != already_free {
            self.too_long = u32::MAX;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DynamicSlots;

    /// The ranges say what a ring of taken flags says, taken and given back
    /// in every order (avatarmanz #47's fuzz, condensed).
    #[test]
    fn free_ranges_match_slot_scanning_in_a_fragmented_heap() {
        const LIMIT: u32 = 511;
        let mut ranges = DynamicSlots::new(LIMIT);
        let mut taken = [false; LIMIT as usize];
        let mut cursor = 0u32;
        let mut seed = 17u32;
        let mut live: Vec<(u32, u32)> = Vec::new();
        let mut next = |seed: &mut u32| {
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *seed
        };
        for _ in 0..20_000 {
            let s = next(&mut seed);
            if s >> 30 == 0 && !live.is_empty() {
                let at = (s >> 8) as usize % live.len();
                let (first, count) = live.swap_remove(at);
                ranges.release(first, count);
                taken[first as usize..(first + count) as usize].fill(false);
            } else {
                let count = (s >> 18) % 31 + 1;
                // The ranges must take a run wherever one exists and refuse
                // only when none does; where exactly is their own choice. A
                // run never wraps the heap's end: a free stretch across it
                // is two runs, and one take cannot serve it.
                let any_free = (0..=LIMIT.saturating_sub(count))
                    .any(|first| (first..first + count).all(|f| !taken[f as usize]));
                let got = ranges.take(count);

                assert_eq!(got.is_some(), any_free, "count {count}: got {got:?}");
                if let Some(g) = got {
                    // The taken run is really free, stays recorded, and the
                    // chosen run is inside the limit.
                    assert!(g + count <= LIMIT, "run {g}+{count} past {LIMIT}");
                    live.push((g, count));
                    taken[g as usize..(g + count) as usize].fill(true);
                    cursor = (g + count) % LIMIT;
                }
            }
        }
    }
}
