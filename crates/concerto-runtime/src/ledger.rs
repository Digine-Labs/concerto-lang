use std::collections::{HashMap, HashSet};

use crate::value::Value;

// ============================================================================
// LedgerEntry
// ============================================================================

/// A single entry in a Ledger.
#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub identifier: String,
    pub keys: Vec<String>,
    pub value: String,
}

impl LedgerEntry {
    /// Convert to a runtime Value (Struct { type_name: "LedgerEntry", fields }).
    pub fn to_value(&self) -> Value {
        let mut fields = HashMap::new();
        fields.insert(
            "identifier".to_string(),
            Value::String(self.identifier.clone()),
        );
        fields.insert(
            "keys".to_string(),
            Value::Array(self.keys.iter().map(|k| Value::String(k.clone())).collect()),
        );
        fields.insert("value".to_string(), Value::String(self.value.clone()));
        Value::Struct {
            type_name: "LedgerEntry".to_string(),
            fields,
        }
    }
}

// ============================================================================
// LedgerStore
// ============================================================================

/// In-memory storage for all ledgers.
///
/// Keys are ledger names (or "name::scope" for scoped views).
pub struct LedgerStore {
    entries: HashMap<String, Vec<LedgerEntry>>,
}

impl LedgerStore {
    pub fn new() -> Self {
        LedgerStore {
            entries: HashMap::new(),
        }
    }

    /// Initialize an empty ledger with the given name.
    pub fn init_ledger(&mut self, name: &str) {
        self.entries.entry(name.to_string()).or_default();
    }

    // ========================================================================
    // Mutations
    // ========================================================================

    /// Insert or upsert an entry. If an entry with the same identifier exists
    /// (exact, case-sensitive), it is replaced.
    pub fn insert(&mut self, ledger: &str, identifier: String, keys: Vec<String>, value: String) {
        let entries = self.entries.entry(ledger.to_string()).or_default();
        if let Some(existing) = entries.iter_mut().find(|e| e.identifier == identifier) {
            existing.keys = keys;
            existing.value = value;
        } else {
            entries.push(LedgerEntry {
                identifier,
                keys,
                value,
            });
        }
    }

    /// Delete an entry by exact identifier match. Returns true if found.
    pub fn delete(&mut self, ledger: &str, identifier: &str) -> bool {
        if let Some(entries) = self.entries.get_mut(ledger) {
            let len_before = entries.len();
            entries.retain(|e| e.identifier != identifier);
            entries.len() < len_before
        } else {
            false
        }
    }

    /// Update the value of an entry by exact identifier. Returns true if found.
    pub fn update(&mut self, ledger: &str, identifier: &str, new_value: String) -> bool {
        if let Some(entries) = self.entries.get_mut(ledger) {
            if let Some(entry) = entries.iter_mut().find(|e| e.identifier == identifier) {
                entry.value = new_value;
                return true;
            }
        }
        false
    }

    /// Replace the keys of an entry by exact identifier. Returns true if found.
    pub fn update_keys(&mut self, ledger: &str, identifier: &str, new_keys: Vec<String>) -> bool {
        if let Some(entries) = self.entries.get_mut(ledger) {
            if let Some(entry) = entries.iter_mut().find(|e| e.identifier == identifier) {
                entry.keys = new_keys;
                return true;
            }
        }
        false
    }

    // ========================================================================
    // Queries
    // ========================================================================

    /// Word-level containment search on identifiers.
    /// An entry matches if EVERY word in `text` appears in the entry's identifier
    /// (case-insensitive, word-tokenized).
    pub fn query_from_identifier(&self, ledger: &str, text: &str) -> Vec<&LedgerEntry> {
        let query_words = tokenize_words(text);
        if query_words.is_empty() {
            return Vec::new();
        }
        self.entries
            .get(ledger)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| {
                        let id_words = tokenize_words(&e.identifier);
                        query_words.iter().all(|qw| id_words.contains(qw))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Exact case-insensitive key match (single key).
    /// An entry matches if any of its keys exactly equals `key` (case-insensitive).
    pub fn query_from_key(&self, ledger: &str, key: &str) -> Vec<&LedgerEntry> {
        let key_lower = key.to_lowercase();
        self.entries
            .get(ledger)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| e.keys.iter().any(|k| k.to_lowercase() == key_lower))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// OR semantics: entry matches if it has at least one key matching any query key
    /// (case-insensitive).
    pub fn query_from_any_keys(&self, ledger: &str, keys: &[String]) -> Vec<&LedgerEntry> {
        let query_keys: HashSet<String> = keys.iter().map(|k| k.to_lowercase()).collect();
        self.entries
            .get(ledger)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| {
                        e.keys
                            .iter()
                            .any(|k| query_keys.contains(&k.to_lowercase()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// AND semantics: entry matches if it has ALL of the query keys (case-insensitive).
    pub fn query_from_exact_keys(&self, ledger: &str, keys: &[String]) -> Vec<&LedgerEntry> {
        let query_keys: HashSet<String> = keys.iter().map(|k| k.to_lowercase()).collect();
        self.entries
            .get(ledger)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| {
                        let entry_keys: HashSet<String> =
                            e.keys.iter().map(|k| k.to_lowercase()).collect();
                        query_keys.iter().all(|qk| entry_keys.contains(qk))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    // ========================================================================
    // Utilities
    // ========================================================================

    /// Number of entries in a ledger.
    pub fn len(&self, ledger: &str) -> usize {
        self.entries.get(ledger).map(|e| e.len()).unwrap_or(0)
    }

    /// Check if a ledger is empty.
    pub fn is_empty(&self, ledger: &str) -> bool {
        self.len(ledger) == 0
    }

    /// Remove all entries from a ledger.
    pub fn clear(&mut self, ledger: &str) {
        if let Some(entries) = self.entries.get_mut(ledger) {
            entries.clear();
        }
    }

    /// Get all entries in a ledger.
    pub fn entries(&self, ledger: &str) -> &[LedgerEntry] {
        self.entries.get(ledger).map(|e| e.as_slice()).unwrap_or(&[])
    }

    /// Get all identifiers in a ledger.
    pub fn identifiers(&self, ledger: &str) -> Vec<&str> {
        self.entries
            .get(ledger)
            .map(|entries| entries.iter().map(|e| e.identifier.as_str()).collect())
            .unwrap_or_default()
    }
}

impl Default for LedgerStore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper: word tokenization
// ============================================================================

/// Tokenize text into lowercase words, splitting on whitespace and stripping
/// punctuation characters.
fn tokenize_words(text: &str) -> HashSet<String> {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_store() -> LedgerStore {
        let mut store = LedgerStore::new();
        store.init_ledger("knowledge");
        store.insert(
            "knowledge",
            "Uniswap contract addresses on Ethereum mainnet.".to_string(),
            vec![
                "Uniswap".to_string(),
                "Contract Address".to_string(),
                "Ethereum".to_string(),
                "Dex".to_string(),
                "Dex contracts".to_string(),
            ],
            "Pool: 0x123, Router: 0x456, Factory: 0x789".to_string(),
        );
        store.insert(
            "knowledge",
            "AAVE contract addresses on Ethereum mainnet.".to_string(),
            vec![
                "AAVE".to_string(),
                "Contract Address".to_string(),
                "Ethereum".to_string(),
                "Lending".to_string(),
                "Money market contracts".to_string(),
            ],
            "Pool: 0x756, MoneyMarket: 0x789, FlashLoan: 0xabc".to_string(),
        );
        store
    }

    #[test]
    fn insert_and_query_identifier() {
        let store = setup_store();
        let results = store.query_from_identifier("knowledge", "Uniswap");
        assert_eq!(results.len(), 1);
        assert!(results[0].identifier.contains("Uniswap"));
    }

    #[test]
    fn upsert_replaces_existing() {
        let mut store = setup_store();
        store.insert(
            "knowledge",
            "Uniswap contract addresses on Ethereum mainnet.".to_string(),
            vec!["Uniswap".to_string()],
            "UPDATED VALUE".to_string(),
        );
        assert_eq!(store.len("knowledge"), 2); // still 2 entries
        let results = store.query_from_identifier("knowledge", "Uniswap");
        assert_eq!(results[0].value, "UPDATED VALUE");
        assert_eq!(results[0].keys.len(), 1); // keys also replaced
    }

    #[test]
    fn query_identifier_word_containment() {
        let store = setup_store();
        // Both entries contain "contract" AND "addresses"
        let results = store.query_from_identifier("knowledge", "contract addresses");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_identifier_case_insensitive() {
        let store = setup_store();
        let results = store.query_from_identifier("knowledge", "aave ethereum");
        assert_eq!(results.len(), 1);
        assert!(results[0].identifier.contains("AAVE"));
    }

    #[test]
    fn query_identifier_no_match() {
        let store = setup_store();
        let results = store.query_from_identifier("knowledge", "Solana");
        assert!(results.is_empty());
    }

    #[test]
    fn query_from_key_exact() {
        let store = setup_store();
        // Both have "Ethereum" key
        let results = store.query_from_key("knowledge", "Ethereum");
        assert_eq!(results.len(), 2);

        // Case-insensitive
        let results = store.query_from_key("knowledge", "dex");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn query_from_key_partial_no_match() {
        let store = setup_store();
        // "Contract" alone doesn't match "Contract Address"
        let results = store.query_from_key("knowledge", "Contract");
        assert!(results.is_empty());
    }

    #[test]
    fn query_from_any_keys_or() {
        let store = setup_store();
        let results = store.query_from_any_keys(
            "knowledge",
            &["Dex".to_string(), "Lending".to_string()],
        );
        assert_eq!(results.len(), 2); // first has Dex, second has Lending

        let results = store.query_from_any_keys(
            "knowledge",
            &["Solana".to_string(), "Polygon".to_string()],
        );
        assert!(results.is_empty());
    }

    #[test]
    fn query_from_exact_keys_and() {
        let store = setup_store();
        // Both entries have both "Ethereum" and "Contract Address"
        let results = store.query_from_exact_keys(
            "knowledge",
            &["Ethereum".to_string(), "Contract Address".to_string()],
        );
        assert_eq!(results.len(), 2);

        // Only second has both "AAVE" and "Lending"
        let results = store.query_from_exact_keys(
            "knowledge",
            &["AAVE".to_string(), "Lending".to_string()],
        );
        assert_eq!(results.len(), 1);
        assert!(results[0].identifier.contains("AAVE"));

        // No entry has both "Dex" AND "Lending"
        let results = store.query_from_exact_keys(
            "knowledge",
            &["Dex".to_string(), "Lending".to_string()],
        );
        assert!(results.is_empty());
    }

    #[test]
    fn delete_returns_bool() {
        let mut store = setup_store();
        assert!(store.delete(
            "knowledge",
            "Uniswap contract addresses on Ethereum mainnet."
        ));
        assert_eq!(store.len("knowledge"), 1);

        assert!(!store.delete("knowledge", "Nonexistent"));
        assert_eq!(store.len("knowledge"), 1);
    }

    #[test]
    fn update_value() {
        let mut store = setup_store();
        assert!(store.update(
            "knowledge",
            "Uniswap contract addresses on Ethereum mainnet.",
            "NEW ADDRESSES".to_string(),
        ));
        let results = store.query_from_identifier("knowledge", "Uniswap");
        assert_eq!(results[0].value, "NEW ADDRESSES");
        // Keys unchanged
        assert_eq!(results[0].keys.len(), 5);

        assert!(!store.update("knowledge", "Nonexistent", "val".to_string()));
    }

    #[test]
    fn update_keys() {
        let mut store = setup_store();
        assert!(store.update_keys(
            "knowledge",
            "Uniswap contract addresses on Ethereum mainnet.",
            vec!["NewKey".to_string()],
        ));
        let results = store.query_from_identifier("knowledge", "Uniswap");
        assert_eq!(results[0].keys, vec!["NewKey"]);

        assert!(!store.update_keys("knowledge", "Nonexistent", vec![]));
    }

    #[test]
    fn len_is_empty_clear() {
        let mut store = setup_store();
        assert_eq!(store.len("knowledge"), 2);
        assert!(!store.is_empty("knowledge"));

        store.clear("knowledge");
        assert_eq!(store.len("knowledge"), 0);
        assert!(store.is_empty("knowledge"));

        // Non-existent ledger
        assert!(store.is_empty("nonexistent"));
        assert_eq!(store.len("nonexistent"), 0);
    }

    #[test]
    fn entries_and_identifiers() {
        let store = setup_store();
        let entries = store.entries("knowledge");
        assert_eq!(entries.len(), 2);

        let ids = store.identifiers("knowledge");
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"Uniswap contract addresses on Ethereum mainnet."));
        assert!(ids.contains(&"AAVE contract addresses on Ethereum mainnet."));
    }

    #[test]
    fn scoped_isolation() {
        let mut store = LedgerStore::new();
        store.init_ledger("shared");
        store.init_ledger("shared::defi");
        store.init_ledger("shared::nft");

        store.insert(
            "shared::defi",
            "Uniswap pools".to_string(),
            vec!["Uniswap".to_string(), "DEX".to_string()],
            "Pool: 0x123...".to_string(),
        );
        store.insert(
            "shared::nft",
            "OpenSea collections".to_string(),
            vec!["OpenSea".to_string(), "NFT".to_string()],
            "Top collections: ...".to_string(),
        );

        // defi scope sees Uniswap but not OpenSea
        let results = store.query_from_key("shared::defi", "Uniswap");
        assert_eq!(results.len(), 1);
        let results = store.query_from_key("shared::defi", "OpenSea");
        assert!(results.is_empty());

        // nft scope sees OpenSea but not Uniswap
        let results = store.query_from_key("shared::nft", "OpenSea");
        assert_eq!(results.len(), 1);
        let results = store.query_from_key("shared::nft", "Uniswap");
        assert!(results.is_empty());

        // parent scope sees nothing (scopes are isolated)
        assert!(store.is_empty("shared"));
    }

    #[test]
    fn entry_to_value() {
        let entry = LedgerEntry {
            identifier: "test id".to_string(),
            keys: vec!["k1".to_string(), "k2".to_string()],
            value: "test value".to_string(),
        };
        let val = entry.to_value();
        match &val {
            Value::Struct { type_name, fields } => {
                assert_eq!(type_name, "LedgerEntry");
                assert_eq!(fields.get("identifier"), Some(&Value::String("test id".to_string())));
                assert_eq!(fields.get("value"), Some(&Value::String("test value".to_string())));
                if let Some(Value::Array(keys)) = fields.get("keys") {
                    assert_eq!(keys.len(), 2);
                } else {
                    panic!("expected Array for keys");
                }
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn tokenize_words_basic() {
        let words = tokenize_words("Hello, World! This is a test.");
        assert!(words.contains("hello"));
        assert!(words.contains("world"));
        assert!(words.contains("test"));
        assert_eq!(words.len(), 6); // hello, world, this, is, a, test
    }

    #[test]
    fn empty_query_returns_empty() {
        let store = setup_store();
        let results = store.query_from_identifier("knowledge", "");
        assert!(results.is_empty());

        let results = store.query_from_identifier("knowledge", "   ");
        assert!(results.is_empty());
    }
}
