//! A small bounded memo for the transcript's per-frame parses.
//!
//! Every frame the transcript re-derives the same things from the same
//! strings: markdown blocks from a turn's source, syntax tokens from a code
//! line, ANSI spans from a shell line. None of them depend on the theme or
//! the layout, so they are pure functions of a string and can be memoised
//! across frames.
//!
//! The rules this type exists to enforce, from the `library-hotpaths` audit:
//!
//! * the key is the string itself, hashed once and **verified** on a hit, so
//!   a hash collision cannot hand back another entry's value;
//! * eviction is **per entry** (least recently used), never `clear()` — a
//!   streaming turn inserting a new prefix every chunk must not evict the
//!   settled turns above it, which are re-read every frame and so are always
//!   the most recently used;
//! * the bound is a named constant at the call site, so the memory a cache
//!   can hold is visible where it is used.
//!
//! Values come back as `Arc<T>`: the caller reads through the `Arc` instead
//! of cloning what the memo holds.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};

/// One cached value plus the key it was built from and its last use.
struct Entry<T> {
    /// The full key, `scope` and `key` joined by a NUL (which neither a
    /// markdown source, a code line nor a language name contains).
    full_key: String,
    value: Arc<T>,
    used: u64,
}

/// Interior state behind the lock.
struct Inner<T> {
    entries: HashMap<u64, Entry<T>>,
    /// Monotonic use counter; the smallest one is evicted first.
    clock: u64,
}

/// Eviction drops this fraction of the entries at a time. Per-entry — the
/// least recently used ones go and everything else stays — but in one scan
/// per batch rather than one scan per insert, so a full cache scrolling
/// through a long file does not pay an O(cap) search for every new line.
const EVICT_DIVISOR: usize = 8;

impl<T> Inner<T> {
    /// Drops the least recently used `cap / EVICT_DIVISOR` entries (at least
    /// one), keeping everything that was read more recently — in particular
    /// everything the current frame has already touched.
    fn evict_oldest(&mut self, cap: usize) {
        let drop_count = (cap / EVICT_DIVISOR).max(1).min(self.entries.len());
        let mut by_use: Vec<(u64, u64)> = self
            .entries
            .iter()
            .map(|(hash, entry)| (entry.used, *hash))
            .collect();
        by_use.sort_unstable();
        for (_, hash) in by_use.into_iter().take(drop_count) {
            self.entries.remove(&hash);
        }
    }
}

/// A bounded, least-recently-used memo from a string key to a shared value.
pub(super) struct Memo<T> {
    cap: usize,
    inner: Mutex<Inner<T>>,
}

/// Separates the scope from the key inside [`Entry::full_key`].
const SEP: char = '\0';

impl<T> Memo<T> {
    /// A memo holding at most `cap` entries. Wrap it in a
    /// [`LazyLock`](std::sync::LazyLock) to hold one in a `static`.
    pub(super) fn new(cap: usize) -> Self {
        Self {
            cap,
            inner: Mutex::new(Inner {
                entries: HashMap::new(),
                clock: 0,
            }),
        }
    }

    /// The value for `key`, building it with `build` on a miss. `scope`
    /// separates keyspaces that share a string — the language a code line is
    /// highlighted as, say — without allocating a joined key on the hot
    /// (hit) path.
    pub(super) fn get_or_insert(
        &self,
        scope: &str,
        key: &str,
        build: impl FnOnce(&str) -> T,
    ) -> Arc<T> {
        let hash = hash_key(scope, key);
        if let Ok(mut inner) = self.inner.lock() {
            inner.clock += 1;
            let clock = inner.clock;
            if let Some(entry) = inner.entries.get_mut(&hash) {
                if matches_key(&entry.full_key, scope, key) {
                    entry.used = clock;
                    return entry.value.clone();
                }
            }
        }
        let value = Arc::new(build(key));
        if let Ok(mut inner) = self.inner.lock() {
            inner.clock += 1;
            let clock = inner.clock;
            if inner.entries.len() >= self.cap {
                inner.evict_oldest(self.cap);
            }
            let mut full_key = String::with_capacity(scope.len() + 1 + key.len());
            full_key.push_str(scope);
            full_key.push(SEP);
            full_key.push_str(key);
            inner.entries.insert(
                hash,
                Entry {
                    full_key,
                    value: value.clone(),
                    used: clock,
                },
            );
        }
        value
    }

    /// How many entries the memo holds. Tests only.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.inner.lock().map(|inner| inner.entries.len()).unwrap_or(0)
    }
}

fn hash_key(scope: &str, key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    scope.hash(&mut hasher);
    key.hash(&mut hasher);
    hasher.finish()
}

/// Whether `full_key` is exactly `scope` + NUL + `key`, without joining them.
fn matches_key(full_key: &str, scope: &str, key: &str) -> bool {
    full_key.len() == scope.len() + SEP.len_utf8() + key.len()
        && full_key.as_bytes()[..scope.len()] == *scope.as_bytes()
        && full_key.as_bytes()[scope.len()] == 0
        && full_key.as_bytes()[scope.len() + 1..] == *key.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hit_returns_the_same_allocation() {
        let memo: Memo<String> = Memo::new(4);
        let first = memo.get_or_insert("", "a", |k| k.to_string());
        let second = memo.get_or_insert("", "a", |_| panic!("should not rebuild"));
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn scopes_do_not_collide() {
        let memo: Memo<String> = Memo::new(4);
        assert_eq!(*memo.get_or_insert("rust", "x", |_| "r".to_string()), "r");
        assert_eq!(*memo.get_or_insert("ts", "x", |_| "t".to_string()), "t");
    }

    #[test]
    fn eviction_drops_one_entry_and_keeps_the_recently_used() {
        let memo: Memo<String> = Memo::new(2);
        let _a = memo.get_or_insert("", "a", |k| k.to_string());
        let _b = memo.get_or_insert("", "b", |k| k.to_string());
        // Touch `a` so `b` is the least recently used, then overflow.
        let _a_again = memo.get_or_insert("", "a", |_| panic!("a is cached"));
        let _c = memo.get_or_insert("", "c", |k| k.to_string());
        assert!(memo.len() <= 2);
        let _a_still = memo.get_or_insert("", "a", |_| panic!("a survived eviction"));
    }

    #[test]
    fn a_key_that_differs_only_across_the_separator_does_not_match() {
        assert!(matches_key("ab\0c", "ab", "c"));
        assert!(!matches_key("ab\0c", "a", "bc"));
        assert!(!matches_key("ab\0c", "ab", "cd"));
    }
}
