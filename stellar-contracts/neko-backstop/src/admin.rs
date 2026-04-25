use soroban_sdk::{Address, Env, panic_with_error};

use crate::error::Error;
use crate::events::Events;
use crate::storage::Storage;

/// Two-step admin transfer (same pattern as `neko-pool`).
pub struct Admin;

impl Admin {
    pub fn propose_admin(env: &Env, proposed: &Address) {
        Storage::get_admin(env).require_auth();
        Storage::set_proposed_admin(env, proposed);
        Events::admin_proposed(env, proposed);
    }

    pub fn accept_admin(env: &Env) {
        let proposed = Storage::get_proposed_admin(env)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotAuthorized));
        proposed.require_auth();
        Storage::del_proposed_admin(env);
        Storage::replace_admin(env, &proposed);
        Events::admin_changed(env, &proposed);
    }
}
