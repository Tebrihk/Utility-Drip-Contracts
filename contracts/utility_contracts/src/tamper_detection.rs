use soroban_sdk::{Env, Address, Symbol};

#[derive(Clone)]
pub struct DeviceState {
    pub tamper_flag: bool,
    pub balance: i128,
    pub authorized: bool,
}

pub fn handle_tamper_signal(env: &Env, device: Address) {
    let mut state: DeviceState = env.storage().get(&format!("device:{}", device))
        .unwrap_or_else(|| panic!("Device not registered"));

    // 1. Set tamper flag
    state.tamper_flag = true;

    // 2. Freeze funds
    let frozen_balance = state.balance;
    state.balance = 0;

    // 3. Route to escrow vault
    let escrow_key = format!("escrow:{}", device);
    let mut escrow_balance: i128 = env.storage().get(&escrow_key).unwrap_or(0);
    escrow_balance += frozen_balance;
    env.storage().set(&escrow_key, &escrow_balance);

    // 4. Revoke authorization
    state.authorized = false;

    // Save updated state
    env.storage().set(&format!("device:{}", device), &state);

    // 5. Emit high-priority event
    env.events().publish(
        (Symbol::short("HardwareTamperingDetected"),),
        (device, frozen_balance),
    );
}

pub fn is_blacklisted(env: &Env, device: Address) -> bool {
    let state: DeviceState = env.storage().get(&format!("device:{}", device))
        .unwrap_or_else(|| panic!("Device not registered"));
    state.tamper_flag
}
