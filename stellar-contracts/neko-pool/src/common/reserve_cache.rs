//! In-transaction cache for [`ReserveData`](crate::common::types::ReserveData).
//! Reduces repeated persistent reads/writes for the same asset within one contract invocation.

use soroban_sdk::{Env, Map, Symbol, Vec};

use crate::common::storage::Storage;
use crate::common::types::ReserveData;

/// Load/save reserve state — implemented by [`ReserveCache`] (batched) or [`StorageReserveSink`] (direct).
pub trait ReserveSink {
    fn load_reserve(&mut self, env: &Env, asset: &Symbol) -> ReserveData;
    fn save_reserve(&mut self, env: &Env, asset: &Symbol, data: &ReserveData);
}

/// Read/write `ReserveData` straight to persistent storage (default for admin / standalone accrue).
pub struct StorageReserveSink;

impl ReserveSink for StorageReserveSink {
    fn load_reserve(&mut self, env: &Env, asset: &Symbol) -> ReserveData {
        Storage::get_reserve_data(env, asset)
    }

    fn save_reserve(&mut self, env: &Env, asset: &Symbol, data: &ReserveData) {
        Storage::set_reserve_data(env, asset, data);
    }
}

/// Per-invocation cache: load once, write dirty entries on [`flush`](Self::flush).
pub struct ReserveCache {
    reserves: Map<Symbol, ReserveData>,
    dirty: Vec<Symbol>,
}

impl ReserveCache {
    pub fn new(env: &Env) -> Self {
        Self {
            reserves: Map::new(env),
            dirty: Vec::new(env),
        }
    }

    fn mark_dirty(&mut self, asset: &Symbol) {
        let sym = asset.clone();
        let len = self.dirty.len();
        let mut i = 0u32;
        while i < len {
            if self.dirty.get(i).unwrap() == sym {
                return;
            }
            i += 1;
        }
        self.dirty.push_back(sym);
    }

    /// Read reserve: from cache if present, else persistent storage (then populate cache).
    pub fn get_reserve(&mut self, env: &Env, asset: &Symbol) -> ReserveData {
        if let Some(r) = self.reserves.get(asset.clone()) {
            return r;
        }
        let r = Storage::get_reserve_data(env, asset);
        self.reserves.set(asset.clone(), r.clone());
        r
    }

    /// Update cached reserve and mark for flush to persistent storage.
    pub fn set_reserve(&mut self, _env: &Env, asset: &Symbol, data: &ReserveData) {
        self.reserves.set(asset.clone(), data.clone());
        self.mark_dirty(asset);
    }

    /// Persist all dirty reserves (call once at end of the operation).
    pub fn flush(&mut self, env: &Env) {
        let len = self.dirty.len();
        let mut i = 0u32;
        while i < len {
            let sym = self.dirty.get(i).unwrap();
            if let Some(data) = self.reserves.get(sym.clone()) {
                Storage::set_reserve_data(env, &sym, &data);
            }
            i += 1;
        }
    }
}

impl ReserveSink for ReserveCache {
    fn load_reserve(&mut self, env: &Env, asset: &Symbol) -> ReserveData {
        self.get_reserve(env, asset)
    }

    fn save_reserve(&mut self, env: &Env, asset: &Symbol, data: &ReserveData) {
        self.set_reserve(env, asset, data);
    }
}
