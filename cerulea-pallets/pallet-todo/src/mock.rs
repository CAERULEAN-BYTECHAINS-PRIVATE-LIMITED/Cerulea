// mock.rs – minimal runtime mock for pallet-todo unit tests

use crate as pallet_todo;
use frame_support::{
    construct_runtime, derive_impl,
    parameter_types,
};
use sp_runtime::BuildStorage;

// ── Type aliases ────────────────────────────────────────────
type Block = frame_system::mocking::MockBlock<Test>;

// ── Runtime construction ─────────────────────────────────────
construct_runtime!(
    pub enum Test {
        System: frame_system,
        TodoPallet: pallet_todo,
    }
);

// ── frame_system config ──────────────────────────────────────
// TestDefaultConfig supplies all defaults; only Block needs overriding here.
#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

// ── pallet_todo config ───────────────────────────────────────
parameter_types! {
    pub const MaxTitleLength: u32 = 128;
    pub const MaxDescriptionLength: u32 = 512;
    pub const MaxTodosPerAccount: u32 = 100;
}

impl pallet_todo::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxTitleLength = MaxTitleLength;
    type MaxDescriptionLength = MaxDescriptionLength;
    type MaxTodosPerAccount = MaxTodosPerAccount;
}

// ── Test externalities builder ────────────────────────────────
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into();

    // Advance to block 1 so that events are properly registered.
    ext.execute_with(|| {
        System::set_block_number(1);
    });

    ext
}
