//! Chain registry — tracks supported chains and their bridgeable tokens.

use crate::types::{BridgeToken, ExternalChain};
use std::collections::HashMap;

/// Registry of supported chains and tokens.
pub struct ChainRegistry {
    tokens: HashMap<ExternalChain, Vec<BridgeToken>>,
}

impl ChainRegistry {
    /// Create a new registry with default supported tokens.
    pub fn new() -> Self {
        let mut tokens = HashMap::new();

        // Bitcoin
        tokens.insert(
            ExternalChain::Bitcoin,
            vec![BridgeToken {
                symbol: "BTC".into(),
                name: "Bitcoin".into(),
                decimals: 8,
                origin_chain: ExternalChain::Bitcoin,
                external_address: String::new(), // native token
                enabled: true,
                min_amount: 10_000,       // 0.0001 BTC
                max_amount: 100_000_000_000, // 1000 BTC
                fee_bps: 10,
            }],
        );

        // Ethereum
        tokens.insert(
            ExternalChain::Ethereum,
            vec![
                BridgeToken {
                    symbol: "ETH".into(),
                    name: "Ether".into(),
                    decimals: 18,
                    origin_chain: ExternalChain::Ethereum,
                    external_address: String::new(),
                    enabled: true,
                    min_amount: 1_000_000_000_000_000,    // 0.001 ETH
                    max_amount: 1_000_000_000_000_000_000_000, // 1000 ETH
                    fee_bps: 10,
                },
                BridgeToken {
                    symbol: "USDT".into(),
                    name: "Tether USD".into(),
                    decimals: 6,
                    origin_chain: ExternalChain::Ethereum,
                    external_address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".into(),
                    enabled: true,
                    min_amount: 1_000_000,    // 1 USDT
                    max_amount: 1_000_000_000_000, // 1M USDT
                    fee_bps: 10,
                },
                BridgeToken {
                    symbol: "USDC".into(),
                    name: "USD Coin".into(),
                    decimals: 6,
                    origin_chain: ExternalChain::Ethereum,
                    external_address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into(),
                    enabled: true,
                    min_amount: 1_000_000,
                    max_amount: 1_000_000_000_000,
                    fee_bps: 10,
                },
            ],
        );

        // BNB Chain
        tokens.insert(
            ExternalChain::BnbChain,
            vec![
                BridgeToken {
                    symbol: "BNB".into(),
                    name: "BNB".into(),
                    decimals: 18,
                    origin_chain: ExternalChain::BnbChain,
                    external_address: String::new(),
                    enabled: true,
                    min_amount: 1_000_000_000_000_000,
                    max_amount: 100_000_000_000_000_000_000_000,
                    fee_bps: 10,
                },
                BridgeToken {
                    symbol: "BUSD".into(),
                    name: "Binance USD".into(),
                    decimals: 18,
                    origin_chain: ExternalChain::BnbChain,
                    external_address: "0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56".into(),
                    enabled: true,
                    min_amount: 1_000_000_000_000_000_000,
                    max_amount: 1_000_000_000_000_000_000_000_000,
                    fee_bps: 10,
                },
            ],
        );

        // Polygon
        tokens.insert(
            ExternalChain::Polygon,
            vec![
                BridgeToken {
                    symbol: "MATIC".into(),
                    name: "Polygon".into(),
                    decimals: 18,
                    origin_chain: ExternalChain::Polygon,
                    external_address: String::new(),
                    enabled: true,
                    min_amount: 1_000_000_000_000_000_000,
                    max_amount: 10_000_000_000_000_000_000_000_000,
                    fee_bps: 10,
                },
            ],
        );

        // Solana
        tokens.insert(
            ExternalChain::Solana,
            vec![
                BridgeToken {
                    symbol: "SOL".into(),
                    name: "Solana".into(),
                    decimals: 9,
                    origin_chain: ExternalChain::Solana,
                    external_address: String::new(),
                    enabled: true,
                    min_amount: 1_000_000,       // 0.001 SOL
                    max_amount: 1_000_000_000_000, // 1000 SOL
                    fee_bps: 10,
                },
            ],
        );

        Self { tokens }
    }

    /// Get all supported chains.
    pub fn supported_chains(&self) -> Vec<ExternalChain> {
        self.tokens.keys().copied().collect()
    }

    /// Get supported tokens for a specific chain.
    pub fn tokens_for_chain(&self, chain: &ExternalChain) -> Vec<BridgeToken> {
        self.tokens.get(chain).cloned().unwrap_or_default()
    }

    /// Get all tokens across all chains.
    pub fn all_tokens(&self) -> Vec<BridgeToken> {
        self.tokens.values().flatten().cloned().collect()
    }

    /// Check if a chain+token combination is supported and enabled.
    pub fn is_supported(&self, chain: &ExternalChain, symbol: &str) -> bool {
        self.tokens
            .get(chain)
            .map(|tokens| tokens.iter().any(|t| t.symbol == symbol && t.enabled))
            .unwrap_or(false)
    }

    /// Get token info for a specific chain+symbol.
    pub fn get_token(&self, chain: &ExternalChain, symbol: &str) -> Option<BridgeToken> {
        self.tokens
            .get(chain)
            .and_then(|tokens| tokens.iter().find(|t| t.symbol == symbol).cloned())
    }
}

impl Default for ChainRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_chains() {
        let registry = ChainRegistry::new();
        let chains = registry.supported_chains();
        assert_eq!(chains.len(), 5);
    }

    #[test]
    fn test_tokens_for_chain() {
        let registry = ChainRegistry::new();
        let eth_tokens = registry.tokens_for_chain(&ExternalChain::Ethereum);
        assert_eq!(eth_tokens.len(), 3); // ETH, USDT, USDC
        assert!(eth_tokens.iter().any(|t| t.symbol == "ETH"));
        assert!(eth_tokens.iter().any(|t| t.symbol == "USDT"));
    }

    #[test]
    fn test_is_supported() {
        let registry = ChainRegistry::new();
        assert!(registry.is_supported(&ExternalChain::Bitcoin, "BTC"));
        assert!(registry.is_supported(&ExternalChain::Ethereum, "ETH"));
        assert!(registry.is_supported(&ExternalChain::Ethereum, "USDT"));
        assert!(!registry.is_supported(&ExternalChain::Bitcoin, "ETH"));
        assert!(!registry.is_supported(&ExternalChain::Solana, "DOGE"));
    }

    #[test]
    fn test_get_token() {
        let registry = ChainRegistry::new();
        let btc = registry.get_token(&ExternalChain::Bitcoin, "BTC").unwrap();
        assert_eq!(btc.decimals, 8);
        assert_eq!(btc.name, "Bitcoin");
    }
}
