//! Running the same work over many files at once.
//!
//! Every slow thing this program does is one external process per track, so
//! throughput comes from running many of them side by side rather than from
//! making any one faster. Encoding and probing want exactly the same shape, and
//! having it in one place is what stops the two from drifting.

use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// How much to run at once by default: one per available core.
///
/// Each ffmpeg and ffprobe is pinned to a single thread, because audio codecs
/// barely parallelise.
pub fn default_concurrency() -> usize {
    thread::available_parallelism().map_or(1, NonZeroUsize::get)
}

/// Run `work` over every item, at most `concurrency` at a time.
///
/// Results come back in the order the items were given, whatever order they
/// finished in. Out of order they would be useless: a failure has to name the
/// file that caused it.
///
/// `on_finished` is called from a worker thread as each item lands, with its
/// index. A folder of a hundred tracks would otherwise sit silent until the
/// last one finished.
///
/// # Panics
///
/// Panics if a worker thread panics, which poisons the shared results and
/// leaves the run with no answer for at least one item.
pub fn map<T, R>(
    items: &[T],
    concurrency: usize,
    work: &(dyn Fn(usize, &T) -> R + Sync),
    on_finished: &(dyn Fn(usize, &R) + Sync),
) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());

    // Workers beyond the item count would start only to find nothing left.
    let workers = concurrency.clamp(1, items.len().max(1));

    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else { break };

                    let outcome = work(index, item);
                    on_finished(index, &outcome);

                    // Held just long enough to drop the result into its slot,
                    // never across the work itself.
                    results.lock().expect("results lock")[index] = Some(outcome);
                }
            });
        }
    });

    results
        .into_inner()
        .expect("results lock")
        .into_iter()
        .map(|slot| slot.expect("every index was filled by a worker"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of collecting into indexed slots: a result that came back
    /// third still has to be reported against the item that produced it.
    #[test]
    fn results_keep_the_order_of_the_items() {
        let items: Vec<usize> = (0..64).collect();

        let doubled = map(&items, 8, &|_, item| item * 2, &|_, _| {});

        assert_eq!(doubled, items.iter().map(|n| n * 2).collect::<Vec<_>>());
    }

    /// Progress has to arrive as work lands, not in a batch at the end.
    #[test]
    fn every_item_is_reported_once() {
        let items: Vec<usize> = (0..64).collect();
        let seen = AtomicUsize::new(0);

        map(&items, 8, &|_, item| *item, &|_, _| {
            seen.fetch_add(1, Ordering::Relaxed);
        });

        assert_eq!(seen.load(Ordering::Relaxed), 64);
    }

    /// Spawning workers for an empty list would have them start only to find
    /// nothing to do, and `clamp` would be handed an inverted range.
    #[test]
    fn nothing_to_do_is_not_a_panic() {
        let empty: Vec<usize> = Vec::new();

        assert!(map(&empty, 8, &|_, item: &usize| *item, &|_, _| {}).is_empty());
    }
}
