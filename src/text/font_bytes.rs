//! Immutable font payloads shared by text objects, formatting spans and history.

use super::MAX_FONT_BYTES;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::{
    hash_map::{Entry, RandomState},
    HashMap,
};
use std::fmt;
use std::hash::{BuildHasher, Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, Mutex, OnceLock, Weak};

struct Allocation {
    bytes: Box<[u8]>,
    fingerprint: u64,
}

/// Exact font file contents, independent of family names and collection indices.
/// Cloning a handle does not copy the font. Construction reuses an existing
/// allocation with equal contents, and the registry retains only weak handles.
#[derive(Clone)]
pub struct FontBytes(Arc<Allocation>);

#[derive(Default)]
struct Registry {
    fonts: HashMap<u64, Vec<Weak<Allocation>>>,
    insertions: usize,
}

fn hash_state() -> &'static RandomState {
    static STATE: OnceLock<RandomState> = OnceLock::new();
    STATE.get_or_init(RandomState::new)
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}

impl FontBytes {
    fn intern(bytes: Cow<'_, [u8]>) -> Self {
        let fingerprint = hash_state().hash_one(bytes.as_ref());
        let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
        let bucket = registry.fonts.entry(fingerprint).or_default();
        bucket.retain(|font| font.strong_count() > 0);
        for existing in bucket.iter().filter_map(Weak::upgrade) {
            if existing.bytes.as_ref() == bytes.as_ref() {
                return Self(existing);
            }
        }
        let allocation = Arc::new(Allocation {
            bytes: bytes.into_owned().into_boxed_slice(),
            fingerprint,
        });
        bucket.push(Arc::downgrade(&allocation));
        registry.insertions += 1;
        if registry.insertions.is_multiple_of(64) {
            // Dead fonts release their payload immediately. This occasional
            // sweep also bounds stale weak handles and hash buckets.
            registry.fonts.retain(|_, fonts| {
                fonts.retain(|font| font.strong_count() > 0);
                !fonts.is_empty()
            });
        }
        Self(allocation)
    }

    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0.bytes
    }
}

impl Default for FontBytes {
    fn default() -> Self {
        Self::from(&[][..])
    }
}

impl From<&[u8]> for FontBytes {
    fn from(bytes: &[u8]) -> Self {
        Self::intern(Cow::Borrowed(bytes))
    }
}

impl<const N: usize> From<&[u8; N]> for FontBytes {
    fn from(bytes: &[u8; N]) -> Self {
        Self::from(bytes.as_slice())
    }
}

impl From<Vec<u8>> for FontBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::intern(Cow::Owned(bytes))
    }
}

impl Deref for FontBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[u8]> for FontBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl PartialEq for FontBytes {
    fn eq(&self, other: &Self) -> bool {
        self.shares_storage_with(other)
            || (self.0.fingerprint == other.0.fingerprint && self.as_slice() == other.as_slice())
    }
}

impl Eq for FontBytes {}

impl Hash for FontBytes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // HashMap table lookups do not repeatedly hash megabytes of shared data.
        self.0.fingerprint.hash(state);
    }
}

impl fmt::Debug for FontBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontBytes")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

impl Serialize for FontBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_slice().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FontBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = FontBytes;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a font byte array of at most 32 MiB")
            }

            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                if sequence
                    .size_hint()
                    .is_some_and(|length| length > MAX_FONT_BYTES)
                {
                    return Err(de::Error::custom(
                        "The embedded font exceeds the 32 MiB limit.",
                    ));
                }
                let mut bytes =
                    Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(64 * 1024));
                while let Some(byte) = sequence.next_element::<u8>()? {
                    if bytes.len() == MAX_FONT_BYTES {
                        return Err(de::Error::custom(
                            "The embedded font exceeds the 32 MiB limit.",
                        ));
                    }
                    bytes.push(byte);
                }
                Ok(FontBytes::from(bytes))
            }
        }
        deserializer.deserialize_seq(Visitor)
    }
}

/// Count a shared font payload once across any group of objects or snapshots.
/// The accounting scope owns no strong references and never prolongs font life.
#[derive(Default)]
pub struct FontMemory {
    allocations: HashMap<usize, Weak<Allocation>>,
}

impl FontMemory {
    pub fn include(&mut self, font: &FontBytes) -> usize {
        if let Entry::Vacant(entry) = self.allocations.entry(Arc::as_ptr(&font.0) as usize) {
            // The weak handle prevents allocation-address reuse within this
            // accounting scope without retaining a dead font's byte payload.
            entry.insert(Arc::downgrade(&font.0));
            font.len()
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::value::{Error, SeqDeserializer};

    #[test]
    fn clones_and_independent_imports_share_exact_font_contents() {
        let original = FontBytes::from(&b"shared font payload"[..]);
        let cloned = original.clone();
        let imported = FontBytes::from(b"shared font payload".to_vec());
        let different = FontBytes::from(&b"different font payload"[..]);
        assert!(original.shares_storage_with(&cloned));
        assert!(original.shares_storage_with(&imported));
        assert_eq!(original.as_ref(), b"shared font payload");
        assert_ne!(original, different);
        assert_eq!(
            std::collections::HashSet::from([original, cloned, imported, different]).len(),
            2
        );
    }

    #[test]
    fn legacy_byte_sequence_serde_preserves_contents_and_storage() {
        let original = FontBytes::from(vec![0, 1, 127, 128, 255]);
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "[0,1,127,128,255]");
        let decoded: FontBytes = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
        assert!(decoded.shares_storage_with(&original));
        assert!(serde_json::from_str::<FontBytes>("[256]").is_err());
        assert!(serde_json::from_str::<FontBytes>("[-1]").is_err());
    }

    #[test]
    fn deserialization_rejects_an_oversized_length_before_reading_bytes() {
        let unread = (0..=MAX_FONT_BYTES).map(|_| -> u8 {
            panic!("The size hint should reject this sequence before reading a payload");
        });
        let result = FontBytes::deserialize(SeqDeserializer::<_, Error>::new(unread));
        assert!(result.unwrap_err().to_string().contains("32 MiB"));
    }

    #[test]
    fn weak_registry_and_accounting_do_not_retain_dead_font_payloads() {
        let font = FontBytes::from(&b"this font is only owned by the lifetime test"[..]);
        let weak = Arc::downgrade(&font.0);
        let mut memory = FontMemory::default();
        assert_eq!(memory.include(&font), font.len());
        assert_eq!(memory.include(&font.clone()), 0);
        drop(font);
        assert!(weak.upgrade().is_none());
        let replacement = FontBytes::from(&b"this font is only owned by the lifetime test"[..]);
        assert_eq!(memory.include(&replacement), replacement.len());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn simultaneous_font_imports_reuse_one_allocation() {
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    FontBytes::from(&b"concurrent imported font"[..])
                })
            })
            .collect();
        let fonts: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert!(fonts.iter().all(|font| font.shares_storage_with(&fonts[0])));
    }
}
