use crate::error::VmError;

/// Gas meter for tracking computation costs.
#[derive(Debug, Clone)]
pub struct GasMeter {
    limit: u64,
    consumed: u64,
}

impl GasMeter {
    pub fn new(limit: u64) -> Self {
        Self { limit, consumed: 0 }
    }

    /// Consume gas, returning an error if the limit is exceeded.
    pub fn consume(&mut self, amount: u64) -> Result<(), VmError> {
        let new_consumed = self.consumed.saturating_add(amount);
        if new_consumed > self.limit {
            self.consumed = self.limit;
            return Err(VmError::OutOfGas {
                limit: self.limit,
                used: new_consumed,
            });
        }
        self.consumed = new_consumed;
        Ok(())
    }

    pub fn consumed(&self) -> u64 {
        self.consumed
    }

    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn is_exhausted(&self) -> bool {
        self.consumed >= self.limit
    }
}

/// Cost table for host operations.
#[derive(Debug, Clone)]
pub struct GasCosts {
    pub storage_read: u64,
    pub storage_write: u64,
    pub storage_delete: u64,
    pub balance_check: u64,
    pub transfer: u64,
    pub hash_blake3: u64,
    pub hash_per_byte: u64,
    pub event_base: u64,
    pub event_per_topic: u64,
    pub event_per_byte: u64,
    pub call_base: u64,
    pub memory_page: u64,
    pub wasm_instruction: u64,
}

impl Default for GasCosts {
    fn default() -> Self {
        Self {
            storage_read: 200,
            storage_write: 5000,
            storage_delete: 5000,
            balance_check: 100,
            transfer: 2300,
            hash_blake3: 30,
            hash_per_byte: 6,
            event_base: 375,
            event_per_topic: 375,
            event_per_byte: 8,
            call_base: 700,
            memory_page: 3,
            wasm_instruction: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_meter_consume() {
        let mut meter = GasMeter::new(1000);
        assert!(meter.consume(500).is_ok());
        assert_eq!(meter.consumed(), 500);
        assert_eq!(meter.remaining(), 500);
    }

    #[test]
    fn test_gas_meter_overflow() {
        let mut meter = GasMeter::new(100);
        assert!(meter.consume(101).is_err());
        assert!(meter.is_exhausted());
    }

    #[test]
    fn test_gas_meter_exact() {
        let mut meter = GasMeter::new(100);
        assert!(meter.consume(100).is_ok());
        assert!(meter.consume(1).is_err());
    }
}
